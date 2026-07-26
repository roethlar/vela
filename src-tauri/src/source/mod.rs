//! Media-source abstraction. Each backend (Plex, Jellyfin/Emby) implements
//! [`MediaSource`] and is registered in the [`SourceRegistry`]. Commands talk
//! to the registry, not to any one backend, so the UI can present a unified
//! library while still being able to scope to a single source.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

pub mod jellyfin;
pub mod plex;

/// A browsable library/section, tagged with the source it came from.
#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SectionDto {
    /// Source-namespaced key (`"<source_id>:<raw>"`); opaque to the frontend.
    pub key: String,
    pub title: String,
    pub section_type: String,
    pub source_id: String,
    pub source_name: String,
    /// The library's persisted sort preference, when one was saved and is
    /// still a valid sort key. Sources always construct this as `None`; the
    /// command layer stamps it from config in `get_sections` — sources know
    /// nothing of sort persistence.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sort: Option<String>,
    /// WHICH SERVER ISSUED THIS KEY, when the source can say. Opaque to the
    /// frontend, which only has to hand it back with any action taken on this
    /// section (see [`MediaSource::scan_library`]).
    ///
    /// A Plex section key is a server-LOCAL number, so "section 2" means a
    /// different library on every server; a source that has repointed at
    /// another server on the same account (rediscovery) would happily act on
    /// a key the user is still looking at but which it no longer issued. The
    /// source cannot detect that from a global "who served the last list"
    /// note, because the key held by an open menu — or by a listing a failed
    /// refresh left on screen — is exactly the one such a note no longer
    /// describes (codex r10, r11). So provenance travels WITH the key.
    ///
    /// `None` = the source cannot vouch for this key's origin; actions that
    /// need provenance must fail closed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provenance: Option<String>,
    /// WHICH BINDING of the source issued this key. Two sections are the same
    /// library only if their key AND their binding match: a source that rebinds
    /// to a server it cannot prove is the same one (Plex rediscovery on a server
    /// whose identity was never established) reissues the SAME section numbers
    /// for DIFFERENT libraries, so a matching key alone proves nothing and the
    /// caller must treat its old root as gone (codex r12).
    ///
    /// This cannot be folded into `provenance`, which is `None` exactly when the
    /// machine is unknown — exactly when a rebind is possible. A caller watching
    /// provenance would see `None -> Some(A)` and be unable to tell a source that
    /// REBOUND from one whose identity probe merely recovered on the same server.
    ///
    /// Sources that cannot rebind (Jellyfin/Emby: one fixed address for life)
    /// always issue `0`.
    pub binding: u64,
}

/// A playable/browsable item (movie, show, season, episode), source-tagged.
/// `Deserialize` exists for the recents persistence round-trip.
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ItemDto {
    /// Source-namespaced key (`"<source_id>:<raw>"`); opaque to the frontend.
    pub rating_key: String,
    pub title: String,
    pub year: Option<u32>,
    pub summary: Option<String>,
    pub duration_ms: Option<u64>,
    pub media_type: Option<String>,
    pub poster: Option<String>,
    /// The series (grandparent) poster for episodic items, when the backend
    /// exposes one — lets catalog rows render portrait art for episodes.
    pub series_poster: Option<String>,
    /// Landscape backdrop/fanart, when the backend exposes one — used by the
    /// resume-row/hero rendering for movies and shows.
    pub backdrop: Option<String>,
    pub view_offset_ms: Option<u64>,
    /// Whether the item is marked watched. `None` when the source doesn't report
    /// it (e.g. local files), so the UI can distinguish "unwatched" from "unknown".
    pub played: Option<bool>,
    /// Unix ms of the last watch activity, when known (Plex `lastViewedAt`;
    /// Vela recents stamp their `ended_at_ms`). Drives the Continue Watching
    /// flow's interleave-by-recency ordering. Jellyfin/Emby: not populated
    /// yet (needs ISO-8601 parsing; recorded follow-up).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_watched_at_ms: Option<u64>,
    /// Unix ms when the item was added to the library, when known (Plex
    /// `addedAt`; local: the file mtime via `Vfs::modified_ms`). Drives the
    /// "date added" sort. Jellyfin/Emby: not populated yet (needs `DateCreated`
    /// in the `Fields=` query + ISO-8601 parsing; recorded follow-up).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub added_at_ms: Option<u64>,
    pub index: Option<u32>,
    pub parent_index: Option<u32>,
    pub grandparent_title: Option<String>,
    pub parent_title: Option<String>,
    /// Source-namespaced key of the parent container (an episode's season, a
    /// season's show), when the backend exposes one — the info surface's
    /// season/show navigation targets.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_rating_key: Option<String>,
    /// Source-namespaced key of the grandparent (an episode's show).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub grandparent_rating_key: Option<String>,
    pub source_id: String,
    /// Cross-source identity hints, normalized as `"scheme:value"`
    /// (e.g. `"imdb:tt0133093"`). Used by the merged All view's dedup.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub provider_ids: Vec<String>,
    /// Present only on merged (deduped) listing entries: every source
    /// backing this title, play target first (override, else kind rank).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub backing: Option<Vec<BackingRef>>,
    /// Stable identity of a merged title (first provider id, else
    /// title+year) — the key the per-title source override persists under.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub canonical_id: Option<String>,
    /// Where watched-state actions should route when the play identity
    /// cannot take them (merged card fronted by a local file while a server
    /// backing owns the watch state). Absent when the play key works.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub watch_key: Option<String>,
    /// Where the detail / "more info" surface (and a merged show's children
    /// drill) should route: the metadata-richest backing of a merged card,
    /// which is usually not the play identity (playback prefers direct files,
    /// detail prefers servers). Absent when the play key is already the
    /// richest — callers fall back to `rating_key`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail_key: Option<String>,
}

/// One source's copy of a merged title.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct BackingRef {
    pub source_id: String,
    pub rating_key: String,
    /// This copy's parent container, when the provider supplied one. Merged
    /// hierarchy navigation uses the per-source path instead of accidentally
    /// drilling through only the display face.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_rating_key: Option<String>,
    /// This copy's grandparent container (an episode's show), when known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub grandparent_rating_key: Option<String>,
}

pub(crate) fn backing_ref_of(item: &ItemDto) -> BackingRef {
    BackingRef {
        source_id: item.source_id.clone(),
        rating_key: item.rating_key.clone(),
        parent_rating_key: item.parent_rating_key.clone(),
        grandparent_rating_key: item.grandparent_rating_key.clone(),
    }
}

fn rekey_namespaced(value: &mut String, old_source_id: &str, new_source_id: &str) {
    let Some((source_id, raw)) = value.split_once(':') else {
        return;
    };
    if source_id == old_source_id {
        *value = namespace_key(new_source_id, raw);
    }
}

/// Re-key every routing identity carried by a persisted item snapshot.
///
/// Only recents and Vela playlists deserialize `ItemDto`; live DTOs are built
/// fresh by their source. Keeping this traversal beside the type prevents a
/// config migration from updating the front-facing key while silently leaving
/// a parent, watch/detail target, or merged backing routed to the retired id.
pub(crate) fn rekey_item_source(
    item: &mut ItemDto,
    old_source_id: &str,
    new_source_id: &str,
) {
    rekey_namespaced(&mut item.rating_key, old_source_id, new_source_id);
    if item.source_id == old_source_id {
        item.source_id = new_source_id.to_string();
    }
    for key in [
        &mut item.parent_rating_key,
        &mut item.grandparent_rating_key,
        &mut item.watch_key,
        &mut item.detail_key,
    ]
    .into_iter()
    .flatten()
    {
        rekey_namespaced(key, old_source_id, new_source_id);
    }
    if let Some(backings) = &mut item.backing {
        for backing in backings {
            rekey_namespaced(&mut backing.rating_key, old_source_id, new_source_id);
            for key in [
                &mut backing.parent_rating_key,
                &mut backing.grandparent_rating_key,
            ]
            .into_iter()
            .flatten()
            {
                rekey_namespaced(key, old_source_id, new_source_id);
            }
            if backing.source_id == old_source_id {
                backing.source_id = new_source_id.to_string();
            }
        }
    }
}

/// Full metadata for a single item — the detail / "more info" surface. A superset
/// of the listing [`ItemDto`], fetched on demand so the grid path stays lean. Every
/// rich field is optional / possibly-empty so a sparse backend (a local file with
/// no `.nfo`) degrades to a clean minimal page rather than an error.
#[derive(Debug, Serialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct DetailDto {
    /// Source-namespaced key (`"<source_id>:<raw>"`).
    pub rating_key: String,
    pub title: String,
    pub year: Option<u32>,
    pub summary: Option<String>,
    pub tagline: Option<String>,
    /// Runtime.
    pub duration_ms: Option<u64>,
    pub media_type: Option<String>,
    pub poster: Option<String>,
    pub backdrop: Option<String>,
    /// Certification (e.g. "PG-13").
    pub content_rating: Option<String>,
    /// Critic/user rating (0–10).
    pub rating: Option<f32>,
    /// Audience rating (0–10), when the backend distinguishes it.
    pub audience_rating: Option<f32>,
    pub studio: Option<String>,
    /// Air/release date as the backend reports it (ISO `YYYY-MM-DD` for Plex).
    pub originally_available_at: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub genres: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub directors: Vec<PersonRef>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub writers: Vec<PersonRef>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub countries: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub cast: Vec<CastMember>,
    /// Episode positioning (populated for episodes; used by the shared episode page).
    pub index: Option<u32>,
    pub parent_index: Option<u32>,
    pub grandparent_title: Option<String>,
    pub parent_title: Option<String>,
    /// Episode parent keys (source-namespaced) when the backend reports them —
    /// they let an episode opened without season context (e.g. a stale hero
    /// recents snapshot) upgrade to its shared season page.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_rating_key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub grandparent_rating_key: Option<String>,
    /// Watch state, when the source reports it — lets the info page show progress
    /// and choose Resume vs Play. `None` = unknown (e.g. a local file).
    pub played: Option<bool>,
    pub view_offset_ms: Option<u64>,
    /// Technical media specs (one entry per available version/file).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub media: Vec<MediaVersionDto>,
    pub source_id: String,
}

/// A cast member for the detail view's cast strip.
#[derive(Debug, Serialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct CastMember {
    pub name: String,
    /// The character played, when known.
    pub role: Option<String>,
    /// Headshot image URL (same accepted poster-exposure class as posters — it may
    /// carry the backend's image token; never logged).
    pub thumb: Option<String>,
    /// Source-namespaced person key (`"<source_id>:<tag_id>"`) when the
    /// backend can identify the person — the person-browse query target.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub person_key: Option<String>,
}

/// A person credit (director/writer) with an optional identity the person
/// browse can query by. Absent `person_key` renders as plain text.
#[derive(Debug, Serialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct PersonRef {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub person_key: Option<String>,
}

/// One media version/file's technical specs.
#[derive(Debug, Serialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct MediaVersionDto {
    /// Human resolution label (e.g. "1080", "4k") when the backend gives one.
    pub video_resolution: Option<String>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub video_codec: Option<String>,
    pub audio_codec: Option<String>,
    pub container: Option<String>,
    /// True when the backend reports an HDR/Dolby-Vision/HLG dynamic range.
    pub hdr: bool,
    /// Per-stream detail (audio channels/codec/language, subtitle languages).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub streams: Vec<MediaStreamDto>,
}

/// One audio/subtitle/video stream within a media version.
#[derive(Debug, Serialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct MediaStreamDto {
    /// 1 = video, 2 = audio, 3 = subtitle (Plex `streamType`).
    pub stream_type: Option<u8>,
    pub codec: Option<String>,
    pub language: Option<String>,
    pub channels: Option<u32>,
    pub display_title: Option<String>,
}

/// A home-screen rail of items, source-tagged.
#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct HubDto {
    pub title: String,
    pub hub_identifier: String,
    pub hub_type: String,
    pub items: Vec<ItemDto>,
    pub source_id: String,
    pub source_name: String,
}

/// One read-only playlist owned by a media server. The playlist key is
/// source-namespaced exactly like item and section keys, so it can be routed
/// back to the source without exposing backend-specific identifiers to the UI.
#[derive(Debug, Serialize, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PlaylistDto {
    pub key: String,
    pub title: String,
    /// Some servers omit a count. The detail fetch remains authoritative.
    pub item_count: Option<usize>,
    pub source_id: String,
    pub source_name: String,
}

/// Minimal hierarchy identity needed to walk from one completed episode to
/// the next. Keys stay source-namespaced so the command layer can route every
/// container through the same registry boundary as ordinary browsing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EpisodeContext {
    pub item_key: String,
    pub season_key: Option<String>,
    pub show_key: Option<String>,
    pub episode_index: Option<u32>,
    pub season_index: Option<u32>,
}

fn episode_context_from_detail(detail: DetailDto) -> Option<EpisodeContext> {
    (detail.media_type.as_deref() == Some("episode")).then_some(EpisodeContext {
        item_key: detail.rating_key,
        season_key: detail.parent_rating_key,
        show_key: detail.grandparent_rating_key,
        episode_index: detail.index,
        season_index: detail.parent_index,
    })
}

/// One rung of the playback quality ladder. Vela mirrors Plex's own ladder so
/// the choices read exactly as they do in Plex's clients
/// (`.agents/decisions.md`, 2026-07-25).
///
/// `width`/`height` are a bounding BOX, not the output size: the server refits
/// to the source's aspect ratio and may go smaller still when the bitrate is the
/// binding constraint. Probed against a live Plex server 2026-07-25 — a 2.35:1
/// source asked for `1920x1080` came back `1920x1038`, and `1280x720` at
/// 2000 kbps came back `720x388`.
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QualityTier {
    /// Stable identifier for config and the command boundary. It carries the
    /// bitrate because two tiers share a label and would otherwise collide.
    pub id: &'static str,
    /// Shown to the user exactly as Plex words it.
    pub label: &'static str,
    pub bitrate_kbps: u32,
    pub width: u32,
    pub height: u32,
}

/// Plex's ladder, read off a live client 2026-07-25. Order is highest quality
/// first, matching the order Plex presents.
///
/// Two entries are labelled "Convert to 1080p HD" and differ only by bitrate,
/// so any UI showing these MUST show the bitrate too or they are
/// indistinguishable.
pub const QUALITY_TIERS: &[QualityTier] = &[
    QualityTier {
        id: "1080p-20000",
        label: "Convert to 1080p HD (High)",
        bitrate_kbps: 20_000,
        width: 1920,
        height: 1080,
    },
    QualityTier {
        id: "1080p-12000",
        label: "Convert to 1080p HD (Medium)",
        bitrate_kbps: 12_000,
        width: 1920,
        height: 1080,
    },
    QualityTier {
        id: "1080p-10000",
        label: "Convert to 1080p HD",
        bitrate_kbps: 10_000,
        width: 1920,
        height: 1080,
    },
    QualityTier {
        id: "1080p-8000",
        label: "Convert to 1080p HD",
        bitrate_kbps: 8_000,
        width: 1920,
        height: 1080,
    },
    QualityTier {
        id: "720p-4000",
        label: "Convert to 720p HD (High)",
        bitrate_kbps: 4_000,
        width: 1280,
        height: 720,
    },
    QualityTier {
        id: "720p-3000",
        label: "Convert to 720p HD (Medium)",
        bitrate_kbps: 3_000,
        width: 1280,
        height: 720,
    },
    QualityTier {
        id: "720p-2000",
        label: "Convert to 720p HD",
        bitrate_kbps: 2_000,
        width: 1280,
        height: 720,
    },
    QualityTier {
        id: "480p-1500",
        label: "Convert to 480p",
        bitrate_kbps: 1_500,
        width: 848,
        height: 480,
    },
    QualityTier {
        id: "328p-700",
        label: "Convert to 328p",
        bitrate_kbps: 700,
        width: 584,
        height: 328,
    },
];

/// Which tiers may be offered for a source of this height.
///
/// Plex filters its ladder by RESOLUTION ONLY and never by bitrate — confirmed
/// against three live samples (`.agents/decisions.md`). A 10 Mbps 1080p source
/// still offers the 20 Mbps and 12 Mbps tiers, and a 1.5 Mbps 384p source drops
/// the 480p tier even though that tier's bitrate matches the source exactly.
/// Do not add a bitrate filter here.
pub fn tiers_for_source(source_height: u32) -> Vec<QualityTier> {
    QUALITY_TIERS
        .iter()
        .copied()
        .filter(|tier| tier.height <= source_height)
        .collect()
}

/// What a given server can actually do with one exact copy, and therefore what
/// Vela is allowed to offer for it. Never offer an option not represented here.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlaybackOptions {
    /// Whether the original file can be played untouched. When false, there is
    /// no Original entry.
    pub can_direct_play: bool,
    /// Whether this server will transcode this copy at all. When false, the
    /// tier list is empty regardless of resolution.
    pub can_transcode: bool,
    pub source_width: u32,
    pub source_height: u32,
    pub source_bitrate_kbps: u32,
    /// Already filtered for this source; empty when transcoding is unavailable.
    pub tiers: Vec<QualityTier>,
}

impl PlaybackOptions {
    pub fn new(
        can_direct_play: bool,
        can_transcode: bool,
        source_width: u32,
        source_height: u32,
        source_bitrate_kbps: u32,
    ) -> Self {
        Self {
            can_direct_play,
            can_transcode,
            source_width,
            source_height,
            source_bitrate_kbps,
            tiers: if can_transcode {
                tiers_for_source(source_height)
            } else {
                Vec::new()
            },
        }
    }

    /// True when the user has a real choice. A single-option item shows no
    /// quality menu at all.
#[allow(dead_code)] // consumed by slice 4's per-title quality menu
#[allow(dead_code)] // consumed by slice 4's per-title quality menu
    pub fn has_choice(&self) -> bool {
        usize::from(self.can_direct_play) + self.tiers.len() > 1
    }

    /// What a stored quality setting means for THIS copy.
    ///
    /// The setting is a ceiling chosen for the user's situation, not a promise
    /// that every file offers that exact rung: a 1080p choice cannot be honoured
    /// literally by a 480p source. The rule is therefore "at most what you
    /// asked for" — take the requested tier when it is offered, else the best
    /// offered tier below it, else fall back to playing the file untouched.
    ///
    /// Falling back UP to a higher tier would hand the user more bitrate than
    /// they asked for, which is exactly wrong on the constrained link that made
    /// them choose.
    pub fn resolve(&self, setting: &str) -> Delivery {
        // UNCONDITIONAL. Original means the untouched file, full stop — never a
        // conversion chosen on the user's behalf because a capability flag was
        // absent or false. Conversion costs HDR and container chapters, so it
        // may only ever happen because the user asked for it. An earlier
        // revision fell back to "the gentlest available conversion" when a
        // server reported no direct-play support; on Jellyfin, whose capability
        // fields are optional, that silently transcoded for people still on the
        // default (review finding tr-3).
        if setting == crate::config::PLAYBACK_QUALITY_ORIGINAL
            || setting == crate::config::PLAYBACK_QUALITY_AUTOMATIC
        {
            return Delivery::Original;
        }
        if self.tiers.is_empty() {
            return Delivery::Original;
        }
        let Some(requested) = QUALITY_TIERS.iter().find(|tier| tier.id == setting) else {
            // An unrecognised setting must not silently convert; config
            // validation should have rejected it long before here.
            return Delivery::Original;
        };
        let chosen = self
            .tiers
            .iter()
            .find(|tier| tier.bitrate_kbps <= requested.bitrate_kbps)
            .copied();
        match chosen {
            Some(tier) => Delivery::Transcode(tier),
            None => Delivery::Original,
        }
    }
}

/// How one play will actually be delivered.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Delivery {
    /// The original file, untouched. The only delivery that preserves HDR.
    Original,
    Transcode(QualityTier),
}

impl Delivery {
    pub fn tier(self) -> Option<QualityTier> {
        match self {
            Delivery::Original => None,
            Delivery::Transcode(tier) => Some(tier),
        }
    }
}

/// A kind of skippable range a media server publishes for one item. Server
/// types Vela does not model (Preview, Recap) are dropped at parse time rather
/// than folded into one of these — a recap is not an advert.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MarkerKind {
    Intro,
    Credits,
    Commercial,
}

/// One skippable range on the exact item being played, in provider-neutral
/// form. Providers publish these in their own units; each backend converts
/// before construction.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MediaMarker {
    pub kind: MarkerKind,
    /// Inclusive start of the skippable range, milliseconds from media start.
    pub start_ms: u64,
    /// Seek target / range end, milliseconds. Always greater than `start_ms`
    /// once [`normalize_markers`] has run.
    pub end_ms: u64,
}

/// Longest range Vela will accept as a real marker. A half-hour skip is far
/// outside any real intro or credit sequence, so anything longer is garbage
/// data rather than something to seek across.
pub const MAX_MARKER_MS: u64 = 30 * 60 * 1000;

/// Shared post-parse cleanup every provider runs before markers reach the
/// playback layer: drop inverted, empty and implausibly long ranges, drop exact
/// duplicate `(kind, start, end)` triples, and order by start.
///
/// Overlapping ranges of the same kind are deliberately kept — the runtime
/// picks the first range containing the current position, which is only
/// well-defined if the list stays sorted by start.
pub fn normalize_markers(mut markers: Vec<MediaMarker>) -> Vec<MediaMarker> {
    let mut seen = std::collections::HashSet::new();
    markers.retain(|marker| {
        marker.end_ms > marker.start_ms
            && marker.end_ms - marker.start_ms <= MAX_MARKER_MS
            && seen.insert((marker.kind, marker.start_ms, marker.end_ms))
    });
    markers.sort_by_key(|marker| marker.start_ms);
    markers
}

/// What `resolve_stream` hands back to the playback layer: the media URL, where
/// to resume from, and how (if at all) to report progress.
pub struct StreamResolution {
    pub url: String,
    pub resume_ms: u64,
    pub progress: crate::playback::ProgressTarget,
    /// HTTP headers mpv must send when fetching `url` — e.g. `X-Plex-Token`,
    /// which travels as a header so the URL stays clean of credentials
    /// (mpv renders `${path}` in its title, stats overlay, and playlist).
    /// Empty only when the stream needs no authentication.
    pub http_headers: Vec<(String, String)>,
    /// How this play is being delivered. `Original` means the untouched file,
    /// which is the only delivery that preserves HDR.
    pub delivery: Delivery,
    /// The handle that must later stop a server-side transcode. `None` for a
    /// direct play. Whatever starts a transcode is obliged to stop it: neither
    /// server has a keep-alive, and how an abandoned session expires is unknown.
    pub transcode_session: Option<String>,
    /// Best-effort intro/credits/commercial ranges for this exact selected
    /// item, already normalized. A provider marker failure is normalized to
    /// empty before construction: markers never fail a play.
    pub markers: Vec<MediaMarker>,
}

/// What one transcode-teardown attempt told us.
///
/// Finding `tr-6`: both providers used to inspect only the transport result, so
/// a 401, 429 or 5xx answer counted as success — nothing logged, nothing tried
/// again, and the encoder still running on the user's server.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum TeardownOutcome {
    /// Nothing left to stop: the server confirmed the teardown, or says it has
    /// no such session.
    Settled,
    /// The server was busy or briefly broken; another attempt may land.
    Retryable(String),
    /// Retrying cannot help — wrong credentials, or the server refuses.
    Refused(String),
}

/// Short, deliberately fixed backoff: this runs on the shutdown path, where the
/// whole teardown is bounded by `TRANSCODE_TEARDOWN_TIMEOUT` in `commands.rs`.
const TEARDOWN_RETRY_BACKOFF: [std::time::Duration; 2] = [
    std::time::Duration::from_millis(200),
    std::time::Duration::from_millis(600),
];

/// Read the server's answer to a teardown request.
pub(crate) fn classify_teardown_status(status: reqwest::StatusCode) -> TeardownOutcome {
    if status == reqwest::StatusCode::NOT_FOUND {
        // The one 4xx that means the job is already done: there is no such
        // session, so there is no encoder to stop.
        return TeardownOutcome::Settled;
    }
    if status.is_success() {
        return TeardownOutcome::Settled;
    }
    let reason = format!("HTTP {}", status.as_u16());
    if status == reqwest::StatusCode::TOO_MANY_REQUESTS || status.is_server_error() {
        TeardownOutcome::Retryable(reason)
    } else {
        TeardownOutcome::Refused(reason)
    }
}

/// Describe a transport failure WITHOUT touching the error's own `Display`.
/// reqwest renders the full request URL there, and a teardown URL carries the
/// Plex token or the Jellyfin `api_key` plus the session handle.
pub(crate) fn describe_transport_failure(error: &reqwest::Error) -> &'static str {
    if error.is_timeout() {
        "timed out"
    } else if error.is_connect() {
        "could not connect"
    } else if error.is_request() {
        "the request could not be sent"
    } else {
        "the request failed"
    }
}

/// Issue a transcode teardown until the server confirms it, refuses it, or the
/// attempts run out — logging every outcome that is not success. `build` is
/// called once per attempt because a `RequestBuilder` cannot be reused.
///
/// Returns how it ended. Nothing in production reads that yet; it is what makes
/// "settled quietly" distinguishable from "reported and abandoned", which a
/// request count alone cannot tell apart.
pub(crate) async fn stop_transcode_request<F>(provider: &str, mut build: F) -> TeardownOutcome
where
    F: FnMut() -> reqwest::RequestBuilder,
{
    let mut last = TeardownOutcome::Retryable("never attempted".to_string());
    for attempt in 0..=TEARDOWN_RETRY_BACKOFF.len() {
        last = match build().send().await {
            Ok(response) => classify_teardown_status(response.status()),
            // No answer at all — the request may not have reached the server,
            // which is exactly what retrying is for.
            Err(error) => TeardownOutcome::Retryable(describe_transport_failure(&error).to_string()),
        };
        match &last {
            TeardownOutcome::Settled => return last,
            TeardownOutcome::Refused(reason) => {
                eprintln!("{provider}: the server refused to stop a transcode session ({reason})");
                return last;
            }
            TeardownOutcome::Retryable(reason) => match TEARDOWN_RETRY_BACKOFF.get(attempt) {
                Some(delay) => {
                    eprintln!(
                        "{provider}: could not stop a transcode session ({reason}); retrying"
                    );
                    tokio::time::sleep(*delay).await;
                }
                None => eprintln!(
                    "{provider}: gave up stopping a transcode session after {} attempts ({reason})",
                    TEARDOWN_RETRY_BACKOFF.len() + 1
                ),
            },
        }
    }
    last
}

/// Credential-free facts about one exact provider media version. This stays
/// entirely behind the Rust command boundary: stream URLs, auth headers, and
/// provider playback-session ids are resolved only after selection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlaybackVersion {
    pub source_id: String,
    pub source_name: String,
    /// Source-namespaced item key, so the shared selector can route the exact
    /// backing without retaining a registry guard.
    pub item_key: String,
    /// Opaque provider media id. It is handed back only to the same source.
    pub version_id: String,
    pub width: u32,
    pub height: u32,
    pub hdr: bool,
    pub bitrate: u64,
    /// Lower is more directly playable: direct play, direct stream, fallback.
    pub direct_play_rank: u8,
    /// Server origin only; never a token-bearing stream URL.
    pub endpoint: url::Url,
    pub provider_verified_local: bool,
}

/// A configured media backend. Methods that *receive* a key get the raw
/// (un-namespaced) key — the registry strips the `"<source_id>:"` prefix before
/// dispatching. Methods that *emit* keys must namespace them via [`namespace_key`].
#[async_trait]
pub trait MediaSource: Send + Sync {
    /// Stable, unique id used to namespace keys and route requests.
    fn id(&self) -> String;
    /// Human-friendly name for the UI (e.g. the server or folder name).
    fn name(&self) -> String;
    /// Backend kind: `"plex"`, `"jellyfin"`, `"emby"`, or a local-family kind
    /// (`"local"` for plain folders, `"smb"`/`"ssh"` for per-mount sources).
    fn kind(&self) -> &'static str;

    async fn sections(&self) -> Result<Vec<SectionDto>, String>;
    async fn hubs(&self) -> Result<Vec<HubDto>, String>;
    async fn items(
        &self,
        section_key: &str,
        section_type: &str,
        sort: Option<&str>,
        start: usize,
        size: usize,
    ) -> Result<Vec<ItemDto>, String>;
    async fn search(&self, query: &str) -> Result<Vec<ItemDto>, String>;
    async fn children(
        &self,
        item_key: &str,
        start: usize,
        size: usize,
    ) -> Result<Vec<ItemDto>, String>;
    /// `include_markers` asks the backend to collect skip ranges while it does
    /// the resolve work it must do anyway. The play command passes `true` only
    /// when a marker policy is actually enabled, so servers see no extra
    /// request for a feature the user turned off.
    /// `quality` is the user's current playback-quality setting, resolved
    /// against what this copy can actually do. `"original"` must leave the
    /// direct-play path exactly as it was.
    async fn resolve_stream(
        &self,
        item_key: &str,
        duration_ms: Option<u64>,
        include_markers: bool,
        quality: &str,
    ) -> Result<StreamResolution, String>;

    /// What this source can deliver for one exact copy, for building the
    /// per-title quality menu. Default is direct-play-only: a source that has
    /// not implemented this must never have conversions offered on its behalf.
    async fn playback_options(
        &self,
        _item_key: &str,
        _version_id: Option<&str>,
    ) -> Result<PlaybackOptions, String> {
        Ok(PlaybackOptions::new(true, false, 0, 0, 0))
    }

    /// Stop a transcode this source started. No-op for sources that never
    /// start one.
    async fn stop_transcode(&self, _session: &str) {}

    /// Fetch provider artwork without exposing its credentials to the
    /// frontend. Only Plex currently uses the app-local artwork protocol.
    async fn fetch_artwork(
        &self,
        _request: crate::artwork::ArtworkRequest,
    ) -> Result<crate::artwork::ArtworkResponse, crate::artwork::ArtworkError> {
        Err(crate::artwork::ArtworkError::Unsupported)
    }

    /// Enumerate exact playable media versions at the play boundary. Sources
    /// that do not support enumeration retain the legacy `resolve_stream`
    /// fallback; every configured server source overrides this.
    async fn playback_versions(&self, _item_key: &str) -> Result<Vec<PlaybackVersion>, String> {
        Ok(Vec::new())
    }

    /// Resolve a previously enumerated provider version, revalidating it with
    /// a fresh provider response. The default preserves legacy sources.
    async fn resolve_stream_version(
        &self,
        item_key: &str,
        duration_ms: Option<u64>,
        _version_id: &str,
        include_markers: bool,
        quality: &str,
    ) -> Result<StreamResolution, String> {
        self.resolve_stream(item_key, duration_ms, include_markers, quality)
            .await
    }

    /// Mark an item watched (`played = true`) or unwatched on its source.
    /// Defaults to a no-op error; sources that support it override this.
    async fn mark_played(&self, _item_key: &str, _played: bool) -> Result<(), String> {
        Err("this source doesn't support marking watched state".to_string())
    }

    /// Ask the backend to drop the item from its server-side Continue
    /// Watching (Plex today; Jellyfin/Emby are a recorded follow-up).
    /// Callers treat failure as non-fatal — Vela's own tombstone already
    /// guarantees the UX. Default: unsupported, quiet no-op.
    async fn remove_from_continue(&self, _item_key: &str) -> Result<(), String> {
        Ok(())
    }

    /// Ask the server to rescan a library section for new/removed files (the
    /// dashboard "scan library files" action — no forced metadata or artwork
    /// refresh). Defaults to unsupported; server backends opt in. Local-family
    /// sources don't need it: their listings re-index on ordinary refresh.
    ///
    /// `provenance` is [`SectionDto::provenance`] as issued with this key,
    /// handed back unchanged by the caller. A scan is an authenticated ACTION
    /// on a server-local id, so a source whose keys are server-local MUST
    /// refuse when it cannot prove the key came from the server it is now
    /// talking to — the caller may be holding a key from a list this source no
    /// longer serves.
    async fn scan_library(&self, _section_key: &str, _provenance: Option<&str>) -> Result<(), String> {
        Err("this source doesn't support server-side library scans".to_string())
    }

    /// Full metadata for one item, for the detail / "more info" surface. Defaults
    /// to unsupported; backends that can enrich an item override this (Plex first,
    /// then Jellyfin/Emby, then local). Callers degrade gracefully on `Err`.
    async fn item_detail(&self, _item_key: &str) -> Result<DetailDto, String> {
        Err("this source doesn't provide item detail".to_string())
    }

    /// Resolve only the hierarchy fields Continue Playing needs. Rich-detail
    /// sources get a default implementation; fixed-address MediaBrowser
    /// sources override this with their existing single-item endpoint.
    async fn episode_context(&self, item_key: &str) -> Result<Option<EpisodeContext>, String> {
        self.item_detail(item_key)
            .await
            .map(episode_context_from_detail)
    }

    /// Everything in this source's libraries featuring a person (`kind` is
    /// "actor" | "director" | "writer"), newest first. Defaults to
    /// unsupported; sources opt in (Plex today — JF/Emby `PersonIds` is a
    /// recorded follow-up).
    async fn person_items(&self, _person_key: &str, _kind: &str) -> Result<Vec<ItemDto>, String> {
        Err("this source doesn't support person browsing".to_string())
    }

    /// Read-only server-owned playlists. Unsupported sources contribute no
    /// playlists rather than failing an aggregate view.
    async fn playlists(&self) -> Result<Vec<PlaylistDto>, String> {
        Ok(Vec::new())
    }

    /// Items in one server-owned playlist, in server order. Unsupported
    /// sources return an empty list, matching [`Self::playlists`].
    async fn playlist_items(&self, _playlist_key: &str) -> Result<Vec<ItemDto>, String> {
        Ok(Vec::new())
    }
}

/// Build a source-namespaced key. Raw Plex/Jellyfin keys never contain `:`,
/// so splitting on the first `:` recovers `(source_id, raw)`.
pub fn namespace_key(source_id: &str, raw: &str) -> String {
    format!("{source_id}:{raw}")
}

/// Split a namespaced key into `(source_id, raw_key)`.
fn split_key(key: &str) -> Option<(&str, &str)> {
    key.split_once(':')
}

/// Holds the configured sources and routes requests to them.
#[derive(Default)]
pub struct SourceRegistry {
    sources: Vec<std::sync::Arc<dyn MediaSource>>,
}

impl SourceRegistry {
    pub fn is_empty(&self) -> bool {
        self.sources.is_empty()
    }

    pub fn all(&self) -> &[std::sync::Arc<dyn MediaSource>] {
        &self.sources
    }

    /// Add a source, replacing any existing one with the same id.
    pub fn upsert(&mut self, source: std::sync::Arc<dyn MediaSource>) {
        let id = source.id();
        self.sources.retain(|s| s.id() != id);
        self.sources.push(source);
    }

    pub fn get(&self, id: &str) -> Option<std::sync::Arc<dyn MediaSource>> {
        self.sources.iter().find(|s| s.id() == id).cloned()
    }

    pub fn remove(&mut self, id: &str) {
        self.sources.retain(|s| s.id() != id);
    }

    /// Ids of every registered source — the "still exists" set for read-time
    /// filtering (e.g. recents entries whose source was removed).
    pub fn ids(&self) -> Vec<String> {
        self.sources.iter().map(|s| s.id().to_string()).collect()
    }

    /// Resolve a namespaced key to its source and the raw (un-prefixed) key.
    pub fn route(
        &self,
        namespaced_key: &str,
    ) -> Result<(std::sync::Arc<dyn MediaSource>, String), String> {
        crate::durable::ensure_commands_ready()?;
        let (id, raw) = split_key(namespaced_key).ok_or("malformed item key")?;
        let src = self.get(id).ok_or("unknown source for item")?;
        Ok((src, raw.to_string()))
    }

    /// Sources to use for a request: a specific one if `source_id` is given,
    /// else all of them (for the unified/aggregate view).
    pub fn selected(
        &self,
        source_id: Option<&str>,
    ) -> Result<Vec<std::sync::Arc<dyn MediaSource>>, String> {
        crate::durable::ensure_commands_ready()?;
        Ok(match source_id {
            Some(id) => self.get(id).into_iter().collect(),
            None => self.sources.clone(),
        })
    }
}

#[cfg(test)]
mod teardown_tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    /// A loopback server that answers each teardown request with the next
    /// canned status and counts what it was actually asked for. Every response
    /// closes its connection, so a request cannot hide inside a reused one.
    async fn canned_server(statuses: &'static [&'static str]) -> (String, Arc<AtomicUsize>) {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let seen = Arc::new(AtomicUsize::new(0));
        let counter = seen.clone();
        tokio::spawn(async move {
            loop {
                let Ok((mut socket, _)) = listener.accept().await else {
                    return;
                };
                let index = counter.fetch_add(1, Ordering::SeqCst);
                let status = statuses.get(index).copied().unwrap_or("500 Server Error");
                // Drain the request line and headers before answering.
                let mut request = Vec::new();
                let mut byte = [0u8; 1];
                while !request.ends_with(b"\r\n\r\n") {
                    use tokio::io::AsyncReadExt;
                    match socket.read(&mut byte).await {
                        Ok(0) | Err(_) => break,
                        Ok(_) => request.push(byte[0]),
                    }
                }
                use tokio::io::AsyncWriteExt;
                let _ = socket
                    .write_all(
                        format!(
                            "HTTP/1.1 {status}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
                        )
                        .as_bytes(),
                    )
                    .await;
                let _ = socket.shutdown().await;
            }
        });
        (base, seen)
    }

    /// The requests the server actually received, and how the teardown ended.
    /// The count alone cannot separate "settled" from "refused" — both stop
    /// after one attempt — so every case asserts on both.
    async fn run_teardown(statuses: &'static [&'static str]) -> (usize, TeardownOutcome) {
        let (base, seen) = canned_server(statuses).await;
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(5))
            .build()
            .unwrap();
        let outcome = stop_transcode_request("test", || client.delete(&base)).await;
        (seen.load(Ordering::SeqCst), outcome)
    }

    /// The tr-6 core: a busy server used to be read as success. It must be read
    /// as failure and tried again, or the encoder keeps running.
    #[tokio::test]
    async fn a_rate_limited_teardown_is_retried_until_it_lands() {
        let (requests, outcome) = run_teardown(&["429 Too Many Requests", "204 No Content"]).await;
        assert_eq!(
            requests, 2,
            "a 429 must be retried, not counted as a completed teardown"
        );
        assert_eq!(
            outcome,
            TeardownOutcome::Settled,
            "the retry must be what finally stops the encoder"
        );
    }

    /// Same class, the other retryable family, and it must exhaust its attempts
    /// rather than stopping after the first bad answer.
    #[tokio::test]
    async fn a_broken_server_is_retried_to_the_attempt_limit() {
        let (requests, outcome) = run_teardown(&["503 Service Unavailable"; 4]).await;
        assert_eq!(
            requests,
            TEARDOWN_RETRY_BACKOFF.len() + 1,
            "every 5xx must be retried, and only up to the attempt limit"
        );
        assert!(
            matches!(outcome, TeardownOutcome::Retryable(_)),
            "an exhausted teardown must end reported, never as success: {outcome:?}"
        );
    }

    /// Retrying a rejected credential cannot help; hammering the server is the
    /// wrong answer to an answer that will not change.
    #[tokio::test]
    async fn a_refused_teardown_is_not_retried() {
        let (requests, outcome) = run_teardown(&["401 Unauthorized"; 4]).await;
        assert_eq!(requests, 1, "a 401 must be reported once, not retried");
        assert!(
            matches!(outcome, TeardownOutcome::Refused(_)),
            "a rejected credential must be reported, not swallowed: {outcome:?}"
        );
    }

    /// Plex answers 404 for a session it no longer has. There is nothing left
    /// to stop, so this is success, not a failure to retry.
    #[tokio::test]
    async fn an_already_gone_session_settles_without_retrying() {
        let (requests, outcome) = run_teardown(&["404 Not Found"; 4]).await;
        assert_eq!(
            requests, 1,
            "a session the server does not have needs no further attempt"
        );
        // The count alone would also pass if 404 were classed as a refusal, so
        // this is the assertion that actually guards the 404 case.
        assert_eq!(
            outcome,
            TeardownOutcome::Settled,
            "an already-gone session is a finished teardown, not a failure"
        );
    }

    #[test]
    fn teardown_statuses_are_classified_by_whether_retrying_can_help() {
        use reqwest::StatusCode;
        for settled in [StatusCode::NO_CONTENT, StatusCode::OK, StatusCode::NOT_FOUND] {
            assert_eq!(classify_teardown_status(settled), TeardownOutcome::Settled);
        }
        for retryable in [
            StatusCode::TOO_MANY_REQUESTS,
            StatusCode::INTERNAL_SERVER_ERROR,
            StatusCode::SERVICE_UNAVAILABLE,
        ] {
            assert!(
                matches!(
                    classify_teardown_status(retryable),
                    TeardownOutcome::Retryable(_)
                ),
                "{retryable} must be retried"
            );
        }
        for refused in [StatusCode::UNAUTHORIZED, StatusCode::FORBIDDEN] {
            assert!(
                matches!(
                    classify_teardown_status(refused),
                    TeardownOutcome::Refused(_)
                ),
                "{refused} must be reported rather than retried"
            );
        }
    }

    /// reqwest renders the whole request URL in its own `Display`, and a
    /// teardown URL identifies the user's server and the session. Nothing
    /// derived from the error may reach a log.
    ///
    /// The token is not in that URL — every request Vela makes is
    /// header-authenticated (swept 2026-07-25, `.agents/review/findings/tr-3.md`)
    /// — but this test uses a token-bearing URL deliberately: it is the
    /// strongest available statement that NOTHING from the URL survives, and it
    /// still holds if a credential ever does end up in a query string.
    #[tokio::test]
    async fn a_transport_failure_is_described_without_its_url() {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_millis(200))
            .build()
            .unwrap();
        let error = client
            .delete("http://127.0.0.1:9/transcode/sessions/SESSIONHANDLE?X-Plex-Token=SECRETTOKEN")
            .send()
            .await
            .expect_err("port 9 must not answer");
        // The premise: the error itself does leak, which is why it is never used.
        assert!(error.to_string().contains("SECRETTOKEN"));

        let described = describe_transport_failure(&error);
        assert!(!described.contains("SECRETTOKEN"), "token leaked: {described}");
        assert!(!described.contains("SESSIONHANDLE"), "session leaked: {described}");
        assert!(!described.contains("127.0.0.1"), "server address leaked: {described}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Fake {
        id: &'static str,
        kind: &'static str,
    }

    #[async_trait]
    impl MediaSource for Fake {
        fn id(&self) -> String {
            self.id.to_string()
        }
        fn name(&self) -> String {
            self.id.to_string()
        }
        fn kind(&self) -> &'static str {
            self.kind
        }
        async fn sections(&self) -> Result<Vec<SectionDto>, String> {
            Ok(vec![])
        }
        async fn hubs(&self) -> Result<Vec<HubDto>, String> {
            Ok(vec![])
        }
        async fn items(
            &self,
            _: &str,
            _: &str,
            _: Option<&str>,
            _: usize,
            _: usize,
        ) -> Result<Vec<ItemDto>, String> {
            Ok(vec![])
        }
        async fn search(&self, _: &str) -> Result<Vec<ItemDto>, String> {
            Ok(vec![])
        }
        async fn children(&self, _: &str, _: usize, _: usize) -> Result<Vec<ItemDto>, String> {
            Ok(vec![])
        }
        async fn resolve_stream(
            &self,
            _: &str,
            _: Option<u64>,
            _: bool,
            _: &str,
        ) -> Result<StreamResolution, String> {
            Err("fake source".into())
        }
    }

    // The three live Plex samples this ladder was derived from
    // (`.agents/decisions.md`, 2026-07-25). They are the whole reason the filter
    // is resolution-only.
    #[test]
    fn tier_filtering_reproduces_every_observed_plex_sample() {
        assert_eq!(
            tiers_for_source(2160).len(),
            9,
            "a 4K source offers the whole ladder"
        );

        // Decisive: the source is 10 Mbps, and Plex still offers 20 and 12.
        // A bitrate filter would wrongly drop them.
        let hd = tiers_for_source(1080);
        assert_eq!(hd.len(), 9, "a 1080p source offers the whole ladder");
        assert!(
            hd.iter().any(|tier| tier.bitrate_kbps == 20_000)
                && hd.iter().any(|tier| tier.bitrate_kbps == 12_000),
            "tiers above the source bitrate are still offered"
        );

        // Decisive the other way: 480p is 1.5 Mbps, exactly this source's
        // bitrate, and Plex drops it because 480 > 384.
        let low = tiers_for_source(384);
        assert_eq!(
            low.iter().map(|tier| tier.id).collect::<Vec<_>>(),
            vec!["328p-700"],
            "only tiers at or below the source height survive"
        );
    }

    // Two tiers are labelled "Convert to 1080p HD"; ids must still be unique or
    // config and the command boundary cannot tell them apart.
    #[test]
    fn tier_ids_are_unique_even_where_labels_collide() {
        let mut ids: Vec<&str> = QUALITY_TIERS.iter().map(|tier| tier.id).collect();
        let total = ids.len();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), total, "tier ids must be unique");

        let duplicated_label = QUALITY_TIERS
            .iter()
            .filter(|tier| tier.label == "Convert to 1080p HD")
            .count();
        assert_eq!(
            duplicated_label, 2,
            "the two same-labelled tiers are expected; the UI must show bitrate"
        );
    }

    // A server that will not transcode offers nothing to transcode to, however
    // large the file is.
    #[test]
    fn options_offer_no_tiers_when_the_server_cannot_transcode() {
        let refused = PlaybackOptions::new(true, false, 3840, 2160, 76_000);
        assert!(refused.tiers.is_empty());
        assert!(!refused.has_choice(), "direct play alone is not a choice");

        let offered = PlaybackOptions::new(true, true, 3840, 2160, 76_000);
        assert_eq!(offered.tiers.len(), 9);
        assert!(offered.has_choice());

        // Transcode-only: no Original entry, but still a choice among tiers.
        let no_direct = PlaybackOptions::new(false, true, 1920, 1080, 10_000);
        assert!(no_direct.has_choice());
    }

    // The whole risk of wiring transcoding is breaking ordinary playback for
    // the people who never asked for it. Original must reach the untouched
    // file, and it must do so without any capability request at all.
    #[test]
    fn original_is_the_untouched_file_for_every_shape_of_copy() {
        for (direct, transcode, height) in [
            (true, true, 2160),
            (true, true, 384),
            (true, false, 1080),
            (false, true, 1080),
        ] {
            let options = PlaybackOptions::new(direct, transcode, 1920, height, 10_000);
            assert_eq!(
                options.resolve(crate::config::PLAYBACK_QUALITY_ORIGINAL),
                Delivery::Original,
                "direct={direct} transcode={transcode} height={height}"
            );
            assert!(
                options
                    .resolve(crate::config::PLAYBACK_QUALITY_ORIGINAL)
                    .tier()
                    .is_none(),
                "Original must never carry a tier, or the play path builds a transcode URL"
            );
        }
    }

    // The setting is a ceiling for the user's situation, not a promise every
    // file carries that rung. Resolving UPWARD would hand a constrained link
    // more bitrate than the user asked for.
    #[test]
    fn a_requested_tier_resolves_down_never_up() {
        let hd = PlaybackOptions::new(true, true, 1920, 1080, 10_000);
        assert_eq!(
            hd.resolve("720p-2000").tier().map(|tier| tier.id),
            Some("720p-2000"),
            "an offered tier is used as asked"
        );

        // 480p source: the only tier at or below it is 328p/700, which is also
        // below the requested 8000, so that is what a 1080p request becomes.
        let sd = PlaybackOptions::new(true, true, 854, 480, 2_000);
        assert_eq!(
            sd.resolve("1080p-8000").tier().map(|tier| tier.id),
            Some("480p-1500"),
            "an unavailable tier steps down to the best offered below it"
        );

        // 384p source offers only 328p/700. A 700-and-below request is fine.
        let tiny = PlaybackOptions::new(true, true, 640, 384, 1_500);
        assert_eq!(
            tiny.resolve("328p-700").tier().map(|tier| tier.id),
            Some("328p-700")
        );
    }

    #[test]
    fn original_and_automatic_never_convert_up_front() {
        let options = PlaybackOptions::new(true, true, 1920, 1080, 10_000);
        assert_eq!(options.resolve("original"), Delivery::Original);
        assert_eq!(
            options.resolve("automatic"),
            Delivery::Original,
            "Automatic starts at Original; stepping down needs evidence first"
        );
        assert_eq!(
            options.resolve("not-a-real-setting"),
            Delivery::Original,
            "an unrecognised setting must not silently convert"
        );
    }

    // Original is unconditional. A server that reports no direct-play support
    // must NOT cause a conversion for a user who never asked for one: on
    // Jellyfin those capability fields are optional, so an absent flag would
    // otherwise transcode silently and cost HDR (finding tr-3).
    #[test]
    fn original_never_converts_even_without_reported_direct_play() {
        let no_direct = PlaybackOptions::new(false, true, 1920, 1080, 10_000);
        assert_eq!(
            no_direct.resolve("original"),
            Delivery::Original,
            "a missing capability flag must not convert on the user's behalf"
        );
        assert_eq!(no_direct.resolve("automatic"), Delivery::Original);

        let nothing = PlaybackOptions::new(false, false, 1920, 1080, 10_000);
        assert_eq!(nothing.resolve("720p-2000"), Delivery::Original);
    }

    fn marker(kind: MarkerKind, start_ms: u64, end_ms: u64) -> MediaMarker {
        MediaMarker {
            kind,
            start_ms,
            end_ms,
        }
    }

    // A range that ends at or before it starts is not skippable, and a
    // half-hour "intro" is corrupt data — seeking across either would throw the
    // viewer somewhere they never asked to be.
    #[test]
    fn normalize_drops_empty_inverted_and_implausibly_long_ranges() {
        let kept = normalize_markers(vec![
            marker(MarkerKind::Intro, 5_000, 4_000),
            marker(MarkerKind::Intro, 5_000, 5_000),
            marker(MarkerKind::Credits, 0, MAX_MARKER_MS + 1),
            marker(MarkerKind::Commercial, 10_000, 10_000 + MAX_MARKER_MS),
        ]);
        assert_eq!(
            kept,
            vec![marker(
                MarkerKind::Commercial,
                10_000,
                10_000 + MAX_MARKER_MS
            )],
            "only the plausible range survives, and the boundary length is kept"
        );
    }

    // Duplicates must go by the whole triple: the same span published as two
    // different kinds is two real markers, not one repeated.
    #[test]
    fn normalize_drops_exact_duplicate_triples_but_keeps_distinct_kinds() {
        let kept = normalize_markers(vec![
            marker(MarkerKind::Intro, 1_000, 2_000),
            marker(MarkerKind::Credits, 1_000, 2_000),
            marker(MarkerKind::Intro, 1_000, 2_000),
        ]);
        assert_eq!(
            kept,
            vec![
                marker(MarkerKind::Intro, 1_000, 2_000),
                marker(MarkerKind::Credits, 1_000, 2_000),
            ],
            "the repeated Intro is dropped; the same-span Credits is not"
        );
    }

    // The runtime picks the first range containing the current position, which
    // is only correct if the list is ordered by start.
    #[test]
    fn normalize_sorts_by_start_and_keeps_same_kind_overlaps() {
        let kept = normalize_markers(vec![
            marker(MarkerKind::Credits, 90_000, 120_000),
            marker(MarkerKind::Intro, 30_000, 60_000),
            marker(MarkerKind::Intro, 10_000, 45_000),
        ]);
        assert_eq!(
            kept,
            vec![
                marker(MarkerKind::Intro, 10_000, 45_000),
                marker(MarkerKind::Intro, 30_000, 60_000),
                marker(MarkerKind::Credits, 90_000, 120_000),
            ],
            "overlapping same-kind ranges are both kept, in start order"
        );
    }

    // The frontend reads these camelCase names; a serde rename regression
    // would silently blank all card artwork.
    #[test]
    fn item_dto_serializes_artwork_fields_camel_case() {
        let dto = ItemDto {
            rating_key: "local:/x".into(),
            title: "T".into(),
            year: None,
            summary: None,
            duration_ms: None,
            media_type: Some("episode".into()),
            poster: Some("p".into()),
            series_poster: Some("sp".into()),
            backdrop: Some("bd".into()),
            view_offset_ms: None,
            played: None,
            last_watched_at_ms: None,
            added_at_ms: None,
            index: None,
            parent_index: None,
            grandparent_title: None,
            parent_title: None,
            parent_rating_key: Some("s1:150".into()),
            grandparent_rating_key: Some("s1:100".into()),
            provider_ids: vec![],
            backing: None,
            canonical_id: None,
            watch_key: None,
            detail_key: None,
            source_id: "local".into(),
        };
        let json = serde_json::to_string(&dto).expect("serialize");
        assert!(json.contains("\"seriesPoster\":\"sp\""));
        // The season/show navigation keys ride camelCase like every other field.
        assert!(json.contains("\"parentRatingKey\":\"s1:150\""));
        assert!(json.contains("\"grandparentRatingKey\":\"s1:100\""));
        assert!(json.contains("\"backdrop\":\"bd\""));
    }

    #[test]
    fn detail_episode_context_keeps_exact_hierarchy_identity() {
        let episode = DetailDto {
            rating_key: "plex:300".into(),
            media_type: Some("episode".into()),
            index: Some(4),
            parent_index: Some(2),
            parent_rating_key: Some("plex:200".into()),
            grandparent_rating_key: Some("plex:100".into()),
            ..DetailDto::default()
        };
        assert_eq!(
            episode_context_from_detail(episode),
            Some(EpisodeContext {
                item_key: "plex:300".into(),
                season_key: Some("plex:200".into()),
                show_key: Some("plex:100".into()),
                episode_index: Some(4),
                season_index: Some(2),
            })
        );

        assert_eq!(
            episode_context_from_detail(DetailDto {
                rating_key: "plex:movie".into(),
                media_type: Some("movie".into()),
                ..DetailDto::default()
            }),
            None,
            "non-episodes must stop only-tv continuation",
        );
    }

    // The "still exists" set for read-time filtering (dead recents entries).
    #[test]
    fn registry_ids_lists_every_registered_source() {
        let mut reg = SourceRegistry::default();
        reg.upsert(std::sync::Arc::new(Fake {
            id: "plex",
            kind: "plex",
        }));
        reg.upsert(std::sync::Arc::new(Fake {
            id: "jf",
            kind: "jellyfin",
        }));
        assert_eq!(reg.ids(), vec!["plex".to_string(), "jf".to_string()]);
    }

    #[tokio::test]
    async fn unsupported_sources_default_to_no_server_playlists() {
        let source = Fake {
            id: "plain",
            kind: "plain",
        };
        assert!(source.playlists().await.unwrap().is_empty());
        assert!(source.playlist_items("anything").await.unwrap().is_empty());
    }
}
