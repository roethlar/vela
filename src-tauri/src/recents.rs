//! Vela's own "recently played" record, feeding the Continue Watching hero.
//! Owner semantic (decision 2026-07-04): recently played and not finished =
//! Continue Watching — regardless of source (local/SMB plays count) and of
//! server-side resume thresholds (Plex ignores plays under ~a minute). The
//! the shared backend play path snapshots the item after mpv starts; the
//! playback end notifier stamps the final mpv position and drops finished
//! entries.

use serde::{Deserialize, Serialize};

use crate::config::AppConfig;
use crate::source::ItemDto;

/// Enough history to fan a cover-flow, small enough for the config file.
pub const MAX_RECENTS: usize = 20;
/// Percent of duration past which a play counts as finished (config
/// `watched_threshold_percent` overrides).
const DEFAULT_WATCHED_THRESHOLD: u8 = 95;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecentEntry {
    /// Snapshot of the item as played (artwork, titles, duration).
    pub item: ItemDto,
    /// Unique playback incarnation. Missing on pre-S3 config entries.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    /// Unix ms when this playback successfully started. Open sessions use it
    /// as their recency stamp so a newly-started item stays ahead of an older
    /// tracker that is still winding down.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub started_at_ms: u64,
    /// Unix ms when the play session ended; 0 while it is still playing.
    pub ended_at_ms: u64,
}

fn is_zero(value: &u64) -> bool {
    *value == 0
}

/// Bound on the Continue Watching tombstone list. Feeds aren't available
/// backend-side, so retired keys can't be pruned precisely; a FIFO cap at
/// hide time keeps the list small instead.
pub(crate) const MAX_HIDDEN: usize = 200;

/// True when `key` names this entry — by its play identity or its server
/// watch identity. Merged items carry both (`rating_key` = ranked play
/// target, often local; `watch_key` = the server backing that owns watch
/// state), and curation actions may arrive under either.
fn entry_matches(entry: &RecentEntry, key: &str) -> bool {
    entry.item.rating_key == key || entry.item.watch_key.as_deref() == Some(key)
}

fn entry_matches_completion(entry: &RecentEntry, play_key: &str, watch_key: Option<&str>) -> bool {
    entry_matches(entry, play_key) || watch_key.is_some_and(|key| entry_matches(entry, key))
}

fn add_tombstones(cfg: &mut AppConfig, keys: impl IntoIterator<Item = String>) {
    for key in keys {
        if !cfg.hidden_from_continue.iter().any(|held| held == &key) {
            cfg.hidden_from_continue.push(key);
        }
    }
    if cfg.hidden_from_continue.len() > MAX_HIDDEN {
        let excess = cfg.hidden_from_continue.len() - MAX_HIDDEN;
        cfg.hidden_from_continue.drain(..excess);
    }
}

/// Record a play starting: newest first, one entry per item, capped.
#[cfg(test)]
pub fn record(cfg: &mut AppConfig, item: ItemDto) {
    record_at(cfg, item, None, 0);
}

fn record_at(cfg: &mut AppConfig, item: ItemDto, session_id: Option<String>, started_at_ms: u64) {
    // Playing something again is the explicit opposite of "stop suggesting
    // it": clear the Continue Watching tombstones of BOTH its identities.
    cfg.hidden_from_continue
        .retain(|k| k != &item.rating_key && item.watch_key.as_deref() != Some(k.as_str()));
    cfg.recents.retain(|r| r.item.rating_key != item.rating_key);
    cfg.recents.insert(
        0,
        RecentEntry {
            item,
            session_id,
            started_at_ms,
            ended_at_ms: 0,
        },
    );
    cfg.recents.truncate(MAX_RECENTS);
}

/// Record the snapshot for a successfully-started playback session. Starting
/// from the beginning is a new zero-offset session even when the card carried
/// an older resume point; resume playback retains that point until the tracker
/// reports a newer one.
pub fn record_play_start(
    cfg: &mut AppConfig,
    mut item: ItemDto,
    start_from_beginning: bool,
    session_id: String,
    started_at_ms: u64,
) {
    if start_from_beginning {
        item.view_offset_ms = Some(0);
    }
    record_at(cfg, item, Some(session_id), started_at_ms);
}

/// Stamp only the exact playback incarnation that started this entry. A
/// replaced same-key tracker finds a different UUID and becomes a no-op. An
/// older different-key tracker may update its position in place, but never its
/// recency stamp once a newer play sits in front of it.
pub fn finish_session(
    cfg: &mut AppConfig,
    rating_key: &str,
    session_id: &str,
    position_ms: u64,
    now_ms: u64,
) {
    let Some(pos) = cfg.recents.iter().position(|entry| {
        entry.item.rating_key == rating_key && entry.session_id.as_deref() == Some(session_id)
    }) else {
        return;
    };
    let threshold = cfg
        .watched_threshold_percent
        .unwrap_or(DEFAULT_WATCHED_THRESHOLD) as u64;
    let finished = cfg.recents[pos].item.duration_ms.is_some_and(|duration| {
        duration > 0 && position_ms.saturating_mul(100) >= duration.saturating_mul(threshold)
    });
    if finished {
        cfg.recents.remove(pos);
        return;
    }
    let entry = &mut cfg.recents[pos];
    if position_ms > 0 {
        entry.item.view_offset_ms = Some(position_ms);
    }
    if pos == 0 {
        entry.ended_at_ms = now_ms;
    }
}

/// Admit a joined clean EOF for exactly the playback incarnation that produced
/// it. A replacement session sharing either the play identity or the owning
/// server's watch identity wins: stale completion work must not remove or hide
/// that replay. Older matching snapshots are curated with the completed one,
/// because they represent the same underlying watch identity and would
/// otherwise keep the carousel stale. The tracker may already have dropped the
/// exact entry at the watched threshold, so its recorded start stamp remains
/// part of the completion signal. Returns whether the completion was admitted
/// and may therefore be synchronized to the owning server.
pub fn complete_clean_session(
    cfg: &mut AppConfig,
    play_key: &str,
    watch_key: Option<&str>,
    session_id: &str,
    started_at_ms: u64,
) -> bool {
    let exact_position = cfg.recents.iter().position(|entry| {
        entry_matches_completion(entry, play_key, watch_key)
            && entry.session_id.as_deref() == Some(session_id)
    });
    let completed_start = exact_position
        .and_then(|position| {
            let recorded = cfg.recents[position].started_at_ms;
            (recorded > 0).then_some(recorded)
        })
        .unwrap_or(started_at_ms);
    let has_newer_match = cfg.recents.iter().enumerate().any(|(position, entry)| {
        if !entry_matches_completion(entry, play_key, watch_key)
            || entry.session_id.as_deref() == Some(session_id)
        {
            return false;
        }
        if completed_start == 0 || entry.started_at_ms > completed_start {
            return true;
        }
        entry.started_at_ms == completed_start
            && exact_position.is_none_or(|exact| position < exact)
    });
    if has_newer_match {
        return false;
    }

    let mut keys = vec![play_key.to_string()];
    if let Some(key) = watch_key {
        keys.push(key.to_string());
    }
    cfg.recents.retain(|entry| {
        let matching_snapshot = entry_matches_completion(entry, play_key, watch_key);
        if matching_snapshot {
            keys.push(entry.item.rating_key.clone());
            if let Some(key) = &entry.item.watch_key {
                keys.push(key.clone());
            }
        }
        !matching_snapshot
    });
    add_tombstones(cfg, keys);
    true
}

/// Stamp a session's final position onto its entry (and re-front it: it is
/// now the most recent thing that happened). An entry past the watched
/// threshold is finished and leaves the list — the hero shows only
/// "recently played and NOT finished".
#[cfg(test)]
pub fn finish(cfg: &mut AppConfig, rating_key: &str, position_ms: u64, now_ms: u64) {
    let Some(pos) = cfg
        .recents
        .iter()
        .position(|r| r.item.rating_key == rating_key)
    else {
        return;
    };
    let mut entry = cfg.recents.remove(pos);
    let threshold = cfg
        .watched_threshold_percent
        .unwrap_or(DEFAULT_WATCHED_THRESHOLD) as u64;
    let finished = entry
        .item
        .duration_ms
        .is_some_and(|d| d > 0 && position_ms.saturating_mul(100) >= d.saturating_mul(threshold));
    if finished {
        return; // watched to the end: no longer "continue watching"
    }
    if position_ms > 0 {
        entry.item.view_offset_ms = Some(position_ms);
    }
    entry.ended_at_ms = now_ms;
    cfg.recents.insert(0, entry);
}

/// Vela's own stamped resume position for a key, 0 when none. The local
/// family keeps no server-side progress, so this stamp is what lets a
/// Continue Watching click actually continue (2026-07-04 hero decision);
/// matches either identity of a merged item, like every other curation op.
pub fn resume_stamp_ms(cfg: &AppConfig, key: &str) -> u64 {
    cfg.recents
        .iter()
        .find(|r| entry_matches(r, key))
        .and_then(|r| r.item.view_offset_ms)
        .unwrap_or(0)
}

/// Drop an item from recents (mark-watched, explicit removal): watched or
/// dismissed = not "continue watching", the same semantic as `finish()`
/// past the threshold.
pub fn unrecord(cfg: &mut AppConfig, rating_key: &str) {
    cfg.recents.retain(|r| !entry_matches(r, rating_key));
}

/// Explicitly remove an item from Continue Watching: drop any recents entry
/// AND tombstone its full identity set (a merged item's server hub copy
/// shows under its watch key, not its play key), so a server hub that still
/// carries the item can't bring it back. The tombstone clears if the item
/// is played again. Returns the key server-side removal should target: the
/// entry's watch key when one exists, else the submitted key.
pub fn hide(cfg: &mut AppConfig, rating_key: &str) -> String {
    let mut keys = vec![rating_key.to_string()];
    let mut server_key = rating_key.to_string();
    if let Some(entry) = cfg.recents.iter().find(|r| entry_matches(r, rating_key)) {
        keys.push(entry.item.rating_key.clone());
        if let Some(watch) = entry.item.watch_key.clone() {
            server_key = watch.clone();
            keys.push(watch);
        }
    }
    unrecord(cfg, rating_key);
    add_tombstones(cfg, keys);
    server_key
}

/// Undo token for a watched-state curation: enough to restore the exact
/// pre-`hide` state when the server edit fails AFTER local curation.
/// Curating first closes the edit-vs-play race (a delayed curation could
/// drop a play recorded during the server round-trip — up to ~15s on the
/// HTTP clients — losing a sub-threshold resume position for good).
pub struct HideUndo {
    /// The dropped entry and its position, when one existed.
    entry: Option<(usize, RecentEntry)>,
    /// Exactly the tombstone keys `hide` added (pre-existing ones are not
    /// ours to remove on undo). Keys the FIFO cap evicted are not restored —
    /// accepted micro-loss on a 200-deep list in an error path.
    added_tombstones: Vec<String>,
}

/// `hide`, but returning an undo token for [`restore_hidden`].
pub fn hide_with_undo(cfg: &mut AppConfig, key: &str) -> HideUndo {
    let entry = cfg
        .recents
        .iter()
        .position(|r| entry_matches(r, key))
        .map(|i| (i, cfg.recents[i].clone()));
    let pre = cfg.hidden_from_continue.clone();
    hide(cfg, key);
    let added_tombstones = cfg
        .hidden_from_continue
        .iter()
        .filter(|k| !pre.contains(k))
        .cloned()
        .collect();
    HideUndo {
        entry,
        added_tombstones,
    }
}

/// Roll a [`hide_with_undo`] back after a failed server edit. Newer play
/// activity wins: if the item was re-recorded since the hide, its fresh
/// state (entry and cleared tombstones) is left untouched.
pub fn restore_hidden(cfg: &mut AppConfig, undo: HideUndo) {
    for k in &undo.added_tombstones {
        untombstone(cfg, k);
    }
    if let Some((idx, entry)) = undo.entry {
        if !cfg
            .recents
            .iter()
            .any(|r| r.item.rating_key == entry.item.rating_key)
        {
            let at = idx.min(cfg.recents.len());
            cfg.recents.insert(at, entry);
        }
    }
}

/// Clear a key's Continue Watching tombstone without recording a play
/// snapshot — the backend play path's counterpart to `record`, which only
/// the frontend's direct-play flow reaches. Exact-key match by design: a
/// merged card's sibling identity stays tombstoned until the next direct
/// play records the full identity set.
pub fn untombstone(cfg: &mut AppConfig, key: &str) {
    cfg.hidden_from_continue.retain(|k| k != key);
}

/// The hero feed: item snapshots, newest first. Each snapshot carries its
/// session stamp so the frontend can interleave recents with server hub items
/// by recency. A still-open session uses its successful start time; its final
/// tracker write replaces that with the end time.
pub fn list(cfg: &AppConfig) -> Vec<ItemDto> {
    cfg.recents
        .iter()
        .map(|r| {
            let mut item = r.item.clone();
            crate::artwork::sanitize_item_artwork(&mut item);
            item.last_watched_at_ms = if r.ended_at_ms > 0 {
                Some(r.ended_at_ms)
            } else {
                (r.started_at_ms > 0).then_some(r.started_at_ms)
            };
            item
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(key: &str, duration_ms: Option<u64>) -> ItemDto {
        ItemDto {
            rating_key: key.into(),
            title: key.into(),
            year: None,
            summary: None,
            duration_ms,
            media_type: Some("movie".into()),
            poster: None,
            series_poster: None,
            backdrop: None,
            view_offset_ms: None,
            played: None,
            last_watched_at_ms: None,
            added_at_ms: None,
            index: None,
            parent_index: None,
            grandparent_title: None,
            parent_title: None,
            parent_rating_key: None,
            grandparent_rating_key: None,
            source_id: "local".into(),
            provider_ids: vec![],
            backing: None,
            canonical_id: None,
            watch_key: None,
            detail_key: None,
        }
    }

    #[test]
    fn record_dedups_fronts_and_caps() {
        let mut cfg = AppConfig::default();
        for i in 0..(MAX_RECENTS + 5) {
            record(&mut cfg, item(&format!("k{i}"), None));
        }
        assert_eq!(cfg.recents.len(), MAX_RECENTS, "capped");
        // Re-playing an older item moves it to the front, no duplicate.
        record(&mut cfg, item("k10", None));
        assert_eq!(cfg.recents[0].item.rating_key, "k10");
        assert_eq!(
            cfg.recents
                .iter()
                .filter(|r| r.item.rating_key == "k10")
                .count(),
            1
        );
    }

    #[test]
    fn record_play_start_resets_only_for_an_explicit_beginning() {
        let mut cfg = AppConfig::default();
        let mut progressed = item("movie", Some(100_000));
        progressed.view_offset_ms = Some(30_000);

        record_play_start(&mut cfg, progressed.clone(), false, "resume".into(), 10);
        assert_eq!(
            cfg.recents[0].item.view_offset_ms,
            Some(30_000),
            "Resume keeps the known position until playback reports a newer one"
        );

        record_play_start(&mut cfg, progressed, true, "beginning".into(), 20);
        assert_eq!(
            cfg.recents[0].item.view_offset_ms,
            Some(0),
            "Play from Beginning must not advertise stale progress"
        );
        assert_eq!(cfg.recents[0].ended_at_ms, 0, "the session is still open");
    }

    #[test]
    fn exact_session_finish_updates_the_current_play() {
        let mut cfg = AppConfig::default();
        record_play_start(
            &mut cfg,
            item("movie", Some(100_000)),
            false,
            "session".into(),
            10,
        );
        finish_session(&mut cfg, "movie", "session", 30_000, 20);
        assert_eq!(cfg.recents[0].item.view_offset_ms, Some(30_000));
        assert_eq!(cfg.recents[0].ended_at_ms, 20);
    }

    #[test]
    fn older_different_key_finish_stays_behind_the_newer_play() {
        let mut cfg = AppConfig::default();
        record_play_start(
            &mut cfg,
            item("older", Some(100_000)),
            false,
            "older-session".into(),
            10,
        );
        record_play_start(
            &mut cfg,
            item("newer", Some(100_000)),
            false,
            "newer-session".into(),
            20,
        );
        finish_session(&mut cfg, "older", "older-session", 30_000, 30);

        assert_eq!(cfg.recents[0].item.rating_key, "newer");
        assert_eq!(cfg.recents[1].item.rating_key, "older");
        assert_eq!(cfg.recents[1].item.view_offset_ms, Some(30_000));
        assert_eq!(
            list(&cfg)[1].last_watched_at_ms,
            Some(10),
            "the delayed finish must not acquire newer recency"
        );
    }

    #[test]
    fn stale_same_key_finish_cannot_stamp_the_replacement_session() {
        let mut cfg = AppConfig::default();
        let mut replay = item("movie", Some(100_000));
        replay.view_offset_ms = Some(40_000);
        record_play_start(&mut cfg, replay.clone(), false, "old".into(), 10);
        record_play_start(&mut cfg, replay, true, "new".into(), 20);

        finish_session(&mut cfg, "movie", "old", 75_000, 30);
        assert_eq!(cfg.recents.len(), 1);
        assert_eq!(cfg.recents[0].session_id.as_deref(), Some("new"));
        assert_eq!(cfg.recents[0].item.view_offset_ms, Some(0));
        assert_eq!(cfg.recents[0].ended_at_ms, 0);
    }

    #[test]
    fn clean_completion_removes_the_exact_session_and_tombstones_every_identity() {
        let mut cfg = AppConfig::default();
        let mut merged = item("local:/shows/one.mkv", Some(100_000));
        merged.watch_key = Some("plex:42".into());
        record_play_start(&mut cfg, merged, false, "completed".into(), 10);

        assert!(complete_clean_session(
            &mut cfg,
            "local:/shows/one.mkv",
            Some("plex:42"),
            "completed",
            10,
        ));
        assert!(cfg.recents.is_empty());
        assert_eq!(
            cfg.hidden_from_continue,
            vec!["local:/shows/one.mkv".to_string(), "plex:42".to_string()]
        );
    }

    #[test]
    fn clean_completion_tombstones_supplied_identities_after_threshold_removal() {
        let mut cfg = AppConfig::default();
        let mut older = item("jf:older-face", Some(100_000));
        older.watch_key = Some("plex:42".into());
        record_play_start(&mut cfg, older, false, "older".into(), 10);
        let mut merged = item("local:/shows/one.mkv", Some(100_000));
        merged.watch_key = Some("plex:42".into());
        record_play_start(&mut cfg, merged, false, "completed".into(), 20);
        finish_session(&mut cfg, "local:/shows/one.mkv", "completed", 96_000, 20);
        assert_eq!(cfg.recents.len(), 1, "only the completed snapshot was removed");
        assert_eq!(cfg.recents[0].session_id.as_deref(), Some("older"));

        assert!(complete_clean_session(
            &mut cfg,
            "local:/shows/one.mkv",
            Some("plex:42"),
            "completed",
            20,
        ));
        assert!(cfg.recents.is_empty(), "older matching snapshots are curated too");
        assert_eq!(
            cfg.hidden_from_continue,
            vec![
                "local:/shows/one.mkv".to_string(),
                "plex:42".to_string(),
                "jf:older-face".to_string(),
            ]
        );
    }

    #[test]
    fn clean_completion_admits_a_newer_exact_session_over_an_older_watch_sibling() {
        let mut cfg = AppConfig::default();
        let mut older = item("jf:older-face", Some(100_000));
        older.watch_key = Some("plex:42".into());
        record_play_start(&mut cfg, older, false, "older".into(), 10);
        finish_session(&mut cfg, "jf:older-face", "older", 30_000, 15);

        let mut completed = item("local:/shows/one.mkv", Some(100_000));
        completed.watch_key = Some("plex:42".into());
        record_play_start(&mut cfg, completed, false, "completed".into(), 20);

        assert!(complete_clean_session(
            &mut cfg,
            "local:/shows/one.mkv",
            Some("plex:42"),
            "completed",
            20,
        ));
        assert!(cfg.recents.is_empty());
        assert_eq!(
            cfg.hidden_from_continue,
            vec![
                "local:/shows/one.mkv".to_string(),
                "plex:42".to_string(),
                "jf:older-face".to_string(),
            ]
        );
    }

    #[test]
    fn stale_clean_completion_cannot_curate_a_newer_same_key_session() {
        let mut cfg = AppConfig::default();
        record_play_start(
            &mut cfg,
            item("plex:42", Some(100_000)),
            false,
            "old".into(),
            10,
        );
        record_play_start(
            &mut cfg,
            item("plex:42", Some(100_000)),
            false,
            "new".into(),
            20,
        );

        assert!(!complete_clean_session(
            &mut cfg,
            "plex:42",
            None,
            "old",
            10,
        ));
        assert_eq!(cfg.recents.len(), 1);
        assert_eq!(cfg.recents[0].session_id.as_deref(), Some("new"));
        assert!(cfg.hidden_from_continue.is_empty());
    }

    #[test]
    fn stale_clean_completion_cannot_curate_a_newer_watch_key_session() {
        let mut cfg = AppConfig::default();
        let mut old = item("jf:old-face", Some(100_000));
        old.watch_key = Some("plex:42".into());
        record_play_start(&mut cfg, old, false, "old".into(), 10);
        let mut new = item("jf:new-face", Some(100_000));
        new.watch_key = Some("plex:42".into());
        record_play_start(&mut cfg, new, false, "new".into(), 20);

        assert!(!complete_clean_session(
            &mut cfg,
            "jf:old-face",
            Some("plex:42"),
            "old",
            10,
        ));
        assert_eq!(cfg.recents.len(), 2, "stale curation changes no snapshots");
        assert_eq!(cfg.recents[0].session_id.as_deref(), Some("new"));
        assert_eq!(cfg.recents[1].session_id.as_deref(), Some("old"));
        assert!(cfg.hidden_from_continue.is_empty());
    }

    #[test]
    fn threshold_removed_completion_rejects_a_newer_open_replay() {
        let mut cfg = AppConfig::default();
        let mut completed = item("local:/shows/one.mkv", Some(100_000));
        completed.watch_key = Some("plex:42".into());
        record_play_start(&mut cfg, completed, false, "completed".into(), 10);
        finish_session(
            &mut cfg,
            "local:/shows/one.mkv",
            "completed",
            96_000,
            15,
        );
        assert!(cfg.recents.is_empty());

        let mut replay = item("jf:new-face", Some(100_000));
        replay.watch_key = Some("plex:42".into());
        record_play_start(&mut cfg, replay, false, "replay".into(), 20);
        assert!(!complete_clean_session(
            &mut cfg,
            "local:/shows/one.mkv",
            Some("plex:42"),
            "completed",
            10,
        ));
        assert_eq!(cfg.recents.len(), 1);
        assert_eq!(cfg.recents[0].session_id.as_deref(), Some("replay"));
        assert!(cfg.hidden_from_continue.is_empty());
    }

    #[test]
    fn replay_clears_clean_completion_tombstones() {
        let mut cfg = AppConfig::default();
        let mut merged = item("local:/shows/one.mkv", Some(100_000));
        merged.watch_key = Some("plex:42".into());
        record_play_start(&mut cfg, merged.clone(), false, "completed".into(), 10);
        assert!(complete_clean_session(
            &mut cfg,
            "local:/shows/one.mkv",
            Some("plex:42"),
            "completed",
            10,
        ));

        record_play_start(&mut cfg, merged, false, "replay".into(), 20);
        assert!(cfg.hidden_from_continue.is_empty());
        assert_eq!(cfg.recents[0].session_id.as_deref(), Some("replay"));
    }

    #[test]
    fn sampled_finish_never_tombstones_without_a_joined_clean_eof() {
        let mut cfg = AppConfig::default();
        record_play_start(
            &mut cfg,
            item("plex:42", Some(100_000)),
            false,
            "quit".into(),
            10,
        );
        finish_session(&mut cfg, "plex:42", "quit", 30_000, 20);
        assert!(cfg.hidden_from_continue.is_empty());
        assert_eq!(cfg.recents[0].session_id.as_deref(), Some("quit"));

        finish_session(&mut cfg, "plex:42", "quit", 96_000, 30);
        assert!(
            cfg.recents.is_empty(),
            "sampled threshold behavior stays intact"
        );
        assert!(
            cfg.hidden_from_continue.is_empty(),
            "a tracker tail alone is not proof of clean EOF"
        );
    }

    #[test]
    fn finish_stamps_position_and_refronts_unfinished_plays() {
        let mut cfg = AppConfig::default();
        record(&mut cfg, item("movie", Some(100_000)));
        record(&mut cfg, item("other", None)); // now in front
        finish(&mut cfg, "movie", 30_000, 1111);
        assert_eq!(
            cfg.recents[0].item.rating_key, "movie",
            "just-ended session is the most recent"
        );
        assert_eq!(cfg.recents[0].item.view_offset_ms, Some(30_000));
        assert_eq!(cfg.recents[0].ended_at_ms, 1111);
    }

    #[test]
    fn resume_stamp_reads_back_the_finished_position() {
        let mut cfg = AppConfig::default();
        record(&mut cfg, item("movie", Some(100_000)));
        finish(&mut cfg, "movie", 30_000, 1111);
        assert_eq!(resume_stamp_ms(&cfg, "movie"), 30_000);
        assert_eq!(
            resume_stamp_ms(&cfg, "unknown"),
            0,
            "no entry ⇒ start from 0"
        );
        // An open session (no finish yet) has no stamp to resume from.
        record(&mut cfg, item("playing", None));
        assert_eq!(resume_stamp_ms(&cfg, "playing"), 0);
    }

    #[test]
    fn list_stamps_recency_only_on_ended_sessions() {
        let mut cfg = AppConfig::default();
        record(&mut cfg, item("movie", Some(100_000)));
        record(&mut cfg, item("playing", None)); // session still open
        finish(&mut cfg, "movie", 30_000, 1111);
        let listed = list(&cfg);
        assert_eq!(listed[0].rating_key, "movie");
        assert_eq!(
            listed[0].last_watched_at_ms,
            Some(1111),
            "ended session carries its stamp for recency interleaving"
        );
        assert_eq!(
            listed[1].last_watched_at_ms, None,
            "open session has no stamp yet"
        );
    }

    #[test]
    fn unrecord_drops_only_the_named_entry() {
        let mut cfg = AppConfig::default();
        record(&mut cfg, item("keep", None));
        record(&mut cfg, item("watched", None));
        unrecord(&mut cfg, "watched");
        assert_eq!(cfg.recents.len(), 1);
        assert_eq!(cfg.recents[0].item.rating_key, "keep");
        // Unknown key is a no-op, not an error.
        unrecord(&mut cfg, "absent");
        assert_eq!(cfg.recents.len(), 1);
    }

    #[test]
    fn unrecord_matches_watch_key_too() {
        // Merged card: plays under a local key, watch state lives on Plex.
        let mut cfg = AppConfig::default();
        let mut merged = item("local:/movies/Heat.mkv", None);
        merged.watch_key = Some("plex:42".into());
        record(&mut cfg, merged);
        unrecord(&mut cfg, "plex:42"); // mark-watched routes the server key
        assert!(
            cfg.recents.is_empty(),
            "a watch-key match must drop the local-keyed entry"
        );
    }

    #[test]
    fn hide_tombstones_every_key_of_a_merged_entry() {
        let mut cfg = AppConfig::default();
        let mut merged = item("local:/movies/Heat.mkv", None);
        merged.watch_key = Some("plex:42".into());
        record(&mut cfg, merged);
        let server = hide(&mut cfg, "local:/movies/Heat.mkv");
        assert_eq!(
            server, "plex:42",
            "server removal must target the watch key"
        );
        assert!(cfg.recents.is_empty());
        assert!(cfg
            .hidden_from_continue
            .contains(&"local:/movies/Heat.mkv".to_string()));
        assert!(
            cfg.hidden_from_continue.contains(&"plex:42".to_string()),
            "server hub copy shows under the watch key; it must be tombstoned too"
        );
        // Replaying the merged item clears BOTH tombstones.
        let mut again = item("local:/movies/Heat.mkv", None);
        again.watch_key = Some("plex:42".into());
        record(&mut cfg, again);
        assert!(cfg.hidden_from_continue.is_empty());
    }

    #[test]
    fn hide_tombstones_and_drops_the_entry() {
        let mut cfg = AppConfig::default();
        record(&mut cfg, item("gone", None));
        hide(&mut cfg, "gone");
        assert!(cfg.recents.is_empty(), "hidden entry leaves recents");
        assert_eq!(cfg.hidden_from_continue, vec!["gone".to_string()]);
        // Idempotent: hiding again doesn't duplicate the tombstone.
        hide(&mut cfg, "gone");
        assert_eq!(cfg.hidden_from_continue.len(), 1);
    }

    #[test]
    fn replaying_clears_the_tombstone() {
        let mut cfg = AppConfig::default();
        hide(&mut cfg, "back");
        record(&mut cfg, item("back", None));
        assert!(
            cfg.hidden_from_continue.is_empty(),
            "playing again is the explicit opposite of 'stop suggesting it'"
        );
        assert_eq!(cfg.recents[0].item.rating_key, "back");
    }

    #[test]
    fn hide_undo_restores_entry_position_and_only_added_tombstones() {
        let mut cfg = AppConfig::default();
        hide(&mut cfg, "pre-existing"); // an older explicit removal
        record(&mut cfg, item("front", None));
        record(&mut cfg, item("target", Some(100_000)));
        record(&mut cfg, item("newest", None)); // target sits at index 1
        finish(&mut cfg, "target", 30_000, 1111); // re-fronts: index 0
        let undo = hide_with_undo(&mut cfg, "target");
        assert!(!cfg.recents.iter().any(|r| r.item.rating_key == "target"));
        assert!(cfg.hidden_from_continue.contains(&"target".to_string()));
        restore_hidden(&mut cfg, undo);
        assert_eq!(
            cfg.recents[0].item.rating_key, "target",
            "entry returns at its original position with its stamp"
        );
        assert_eq!(cfg.recents[0].item.view_offset_ms, Some(30_000));
        assert!(
            !cfg.hidden_from_continue.contains(&"target".to_string()),
            "the tombstone the hide added is removed on undo"
        );
        assert!(
            cfg.hidden_from_continue
                .contains(&"pre-existing".to_string()),
            "tombstones the hide did NOT add survive the undo"
        );
    }

    #[test]
    fn restore_hidden_yields_to_newer_play_activity() {
        let mut cfg = AppConfig::default();
        record(&mut cfg, item("movie", Some(100_000)));
        finish(&mut cfg, "movie", 30_000, 1111);
        let undo = hide_with_undo(&mut cfg, "movie");
        // A replay lands between the hide and the failed server edit: it
        // clears the tombstone and records a fresh OPEN entry.
        record(&mut cfg, item("movie", Some(100_000)));
        restore_hidden(&mut cfg, undo);
        assert_eq!(cfg.recents.len(), 1, "no duplicate entry");
        assert_eq!(
            cfg.recents[0].ended_at_ms, 0,
            "the fresh open session wins over the restored snapshot"
        );
        assert!(cfg.hidden_from_continue.is_empty());
    }

    #[test]
    fn untombstone_clears_only_the_named_key() {
        let mut cfg = AppConfig::default();
        hide(&mut cfg, "played-from-queue");
        hide(&mut cfg, "other");
        untombstone(&mut cfg, "played-from-queue");
        assert_eq!(cfg.hidden_from_continue, vec!["other".to_string()]);
        // Unknown key is a no-op, not an error.
        untombstone(&mut cfg, "absent");
        assert_eq!(cfg.hidden_from_continue, vec!["other".to_string()]);
    }

    #[test]
    fn tombstone_list_is_bounded_fifo() {
        let mut cfg = AppConfig::default();
        for i in 0..(MAX_HIDDEN + 10) {
            hide(&mut cfg, &format!("k{i}"));
        }
        assert_eq!(cfg.hidden_from_continue.len(), MAX_HIDDEN);
        assert_eq!(cfg.hidden_from_continue[0], "k10", "oldest pruned first");
    }

    #[test]
    fn finish_drops_entries_past_the_watched_threshold() {
        let mut cfg = AppConfig::default();
        record(&mut cfg, item("movie", Some(100_000)));
        finish(&mut cfg, "movie", 96_000, 1111); // ≥ default 95%
        assert!(
            cfg.recents.is_empty(),
            "finished plays are not 'continue watching'"
        );

        // Unknown duration can never be judged finished: entry stays.
        record(&mut cfg, item("localfile", None));
        finish(&mut cfg, "localfile", 5_000, 2222);
        assert_eq!(cfg.recents.len(), 1);
    }
}
