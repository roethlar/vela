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

/// Record a play starting: newest first, one entry per item, capped.
pub fn record(cfg: &mut AppConfig, item: ItemDto) {
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

/// Drop an item from recents (mark-watched, explicit removal): watched or
/// dismissed = not "continue watching", the same semantic as `finish()`
/// past the threshold.
pub fn unrecord(cfg: &mut AppConfig, rating_key: &str) {
    cfg.recents.retain(|r| r.item.rating_key != rating_key);
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
            index: None,
            parent_index: None,
            grandparent_title: None,
            parent_title: None,
            source_id: "local".into(),
            provider_ids: vec![],
            backing: None,
            canonical_id: None,
            watch_key: None,
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
