//! Vela's own "recently played" record, feeding the Continue Watching hero.
//! Owner semantic (decision 2026-07-04): recently played and not finished =
//! Continue Watching — regardless of source (local/SMB plays count) and of
//! server-side resume thresholds (Plex ignores plays under ~a minute). The
//! frontend snapshots the item when playback starts; the playback end
//! notifier stamps the final mpv position and drops finished entries.

use serde::{Deserialize, Serialize};

use crate::config::AppConfig;
use crate::source::ItemDto;

/// Enough history to fan a cover-flow, small enough for the config file.
pub const MAX_RECENTS: usize = 20;
/// Percent of duration past which a play counts as finished (config
/// `watched_threshold_percent` overrides).
const DEFAULT_WATCHED_THRESHOLD: u8 = 95;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecentEntry {
    /// Snapshot of the item as played (artwork, titles, duration).
    pub item: ItemDto,
    /// Unix ms when the play session ended; 0 while it is still playing.
    pub ended_at_ms: u64,
}

/// Bound on the Continue Watching tombstone list. Feeds aren't available
/// backend-side, so retired keys can't be pruned precisely; a FIFO cap at
/// hide time keeps the list small instead.
const MAX_HIDDEN: usize = 200;

/// True when `key` names this entry — by its play identity or its server
/// watch identity. Merged items carry both (`rating_key` = ranked play
/// target, often local; `watch_key` = the server backing that owns watch
/// state), and curation actions may arrive under either.
fn entry_matches(entry: &RecentEntry, key: &str) -> bool {
    entry.item.rating_key == key || entry.item.watch_key.as_deref() == Some(key)
}

/// Record a play starting: newest first, one entry per item, capped.
pub fn record(cfg: &mut AppConfig, item: ItemDto) {
    // Playing something again is the explicit opposite of "stop suggesting
    // it": clear the Continue Watching tombstones of BOTH its identities.
    cfg.hidden_from_continue
        .retain(|k| k != &item.rating_key && item.watch_key.as_deref() != Some(k.as_str()));
    cfg.recents.retain(|r| r.item.rating_key != item.rating_key);
    cfg.recents.insert(
        0,
        RecentEntry {
            item,
            ended_at_ms: 0,
        },
    );
    cfg.recents.truncate(MAX_RECENTS);
}

/// Stamp a session's final position onto its entry (and re-front it: it is
/// now the most recent thing that happened). An entry past the watched
/// threshold is finished and leaves the list — the hero shows only
/// "recently played and NOT finished".
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
    for key in keys {
        if !cfg.hidden_from_continue.iter().any(|k| k == &key) {
            cfg.hidden_from_continue.push(key);
        }
    }
    if cfg.hidden_from_continue.len() > MAX_HIDDEN {
        let excess = cfg.hidden_from_continue.len() - MAX_HIDDEN;
        cfg.hidden_from_continue.drain(..excess);
    }
    server_key
}

/// The hero feed: item snapshots, newest first. Each snapshot carries its
/// session-end stamp so the frontend can interleave recents with server
/// hub items by recency. A still-open session (`ended_at_ms == 0`) has no
/// stamp yet; the stamp lands at mpv exit.
pub fn list(cfg: &AppConfig) -> Vec<ItemDto> {
    cfg.recents
        .iter()
        .map(|r| {
            let mut item = r.item.clone();
            item.last_watched_at_ms = (r.ended_at_ms > 0).then_some(r.ended_at_ms);
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
        assert_eq!(resume_stamp_ms(&cfg, "unknown"), 0, "no entry ⇒ start from 0");
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
        assert_eq!(server, "plex:42", "server removal must target the watch key");
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
