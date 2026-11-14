use serde::{Deserialize, Serialize};
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

const LEGACY_PLEX_SOURCE_ID: &str = "plex";

/// Serializes load-modify-save cycles so concurrent commands (and Plex
/// rediscovery) can't lose each other's updates via interleaved read/writes.
static CONFIG_LOCK: Mutex<()> = Mutex::new(());

#[derive(Clone, Serialize, Deserialize, Default)]
#[serde(default, deny_unknown_fields)]
pub struct AppConfig {
    /// Pre-multi-Plex singleton credentials. Read only by the one-shot
    /// migration; new code persists Plex credentials in `connections.json`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auth_token: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub client_identifier: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_server_host: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_server_port: Option<u16>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_server_scheme: Option<String>,
    pub last_section_key: Option<String>,
    pub last_page_start: Option<usize>,
    pub watched_threshold_percent: Option<u8>,
    pub auto_play_via_mpv: Option<bool>,
    /// Explicit path to the mpv executable, set by the user when auto-discovery
    /// can't find it (e.g. mpv installed somewhere unusual, or not on PATH).
    /// Takes precedence over every other discovery step.
    pub mpv_path: Option<String>,
    /// User-supplied advanced mpv options, one per line (blank lines and `#`
    /// comments ignored). Appended at launch so they override Vela's render
    /// defaults; the IPC socket / resume seek / media URL are re-asserted after and
    /// can't be overridden. Empowers power users (and weak-hardware users who want a
    /// lighter profile) without Vela having to guess.
    pub mpv_extra_args: Option<String>,
    /// When true, Vela drops `--no-config` so mpv loads the user's own
    /// `~/.config/mpv/mpv.conf`. It also leaves macOS audio-output selection to
    /// that config. Off by default for a reproducible launch.
    pub mpv_use_own_config: Option<bool>,
    /// Black-bar cropping via mpv's bundled `autocrop.lua`, three-state:
    /// `"off"` (default; nothing injected), `"manual"` (script loaded with
    /// `autocrop-auto=no` — crops only on an explicit in-player `Shift+C`), or
    /// `"auto"` (script's own crop-on-playback-start). Missing means off;
    /// unknown values invalidate the settings document.
    /// `"auto"` auto-fires mpv's live `video-crop`, which can hang mpv on some
    /// HDR/Wayland stacks — the Settings UI carries that warning.
    pub mpv_autocrop: Option<String>,
    /// Which player plays video: `"external"` (mpv in its own window) or
    /// `"embedded"` (mpv's library drawing inside Vela's window; experimental,
    /// Linux on Wayland only). Missing means external on every desktop OS
    /// (decision 2026-11-12); unknown values invalidate the document.
    ///
    /// Written only when set, so a config that never chose a player stays
    /// readable by releases that predate this field. A config that DOES carry
    /// it is rejected by those releases, like any setting they do not know.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub player: Option<String>,
    /// What to do after a clean EOF once a single item or named playlist has
    /// genuinely ended: `"off"`, `"on"`, or `"only-tv"`. Missing means the
    /// product default (`"only-tv"`); unknown values invalidate the document.
    pub continue_playing: Option<String>,
    /// How the first mpv window of a run of Vela opens: `"remember"` (as the
    /// last play of the previous run left it), `"fullscreen"`, or `"default"`
    /// (nothing from Vela). Later plays in a run always open as the play
    /// before them left the window. Missing means remember (owner ruling D2(a),
    /// `.agents/plans/mpv-window-geometry.md`); unknown values invalidate the
    /// document.
    ///
    /// Written only when set, like `player`: a release that predates the field
    /// rejects any key it does not know.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mpv_window_mode: Option<String>,
    /// How the last external play left its mpv window, kept for the first play
    /// of the next run in remember mode. Written when a play ends, and only
    /// then; removed when the mode leaves remember. Never a position.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mpv_window: Option<MpvWindow>,
    /// How duplicate copies are selected at the shared play boundary. Missing
    /// means `best`; unknown values invalidate the document.
    pub playback_source_policy: Option<String>,
    /// Optional compatibility-display resolution override. Missing means Auto;
    /// unknown values invalidate the document.
    pub playback_display_resolution: Option<String>,
    /// Optional compatibility-display HDR override (`enabled`/`disabled`).
    /// Missing means Auto; unknown values invalidate the document.
    pub playback_display_hdr: Option<String>,
    /// What Vela does with each kind of server-published marker range. Missing
    /// means the approved [`SkipPolicy::MISSING`] default; a present value
    /// outside the closed enum fails deserialization and invalidates the whole
    /// document. Missing and invalid are deliberately not the same thing.
    /// Current playback quality: `"original"`, `"automatic"`, or one of the
    /// quality-ladder tier ids. Missing means `"original"`, so every existing
    /// install keeps direct play and HDR passthrough untouched. Unknown values
    /// invalidate the document.
    ///
    /// This tracks the user's SITUATION, not a title and not the machine's
    /// fixed capability — the same laptop is on a cafe link one day and wired
    /// the next — so it is a plain setting the user flips, never derived and
    /// never remembered per file.
    pub playback_quality: Option<String>,
    pub skip_intros: Option<SkipPolicy>,
    pub skip_credits: Option<SkipPolicy>,
    pub skip_commercials: Option<SkipPolicy>,
    /// Pre-split media-server connections. New writes live in
    /// `connections.json`; this compatibility field is accepted only so the
    /// exact one-time split can move a fully valid legacy document.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) sources: Vec<SourceConfig>,
    /// INERT since 2026-07-08 (local/SMB/SSH sources removed — see
    /// `.agents/decisions.md` "Vela is a multi-server client"): these three
    /// fields are parsed, ignored, and preserved on save so an older build
    /// can still read its config after a rollback. Never strip them here.
    #[serde(default)]
    pub local_folders: Vec<LocalFolder>,
    #[serde(default)]
    pub smb_mounts: Vec<SmbMount>,
    #[serde(default)]
    pub ssh_mounts: Vec<SshMount>,
    /// Per-title playback-source overrides for the merged All view: canonical
    /// title identity → preferred source id. Set from the card's context
    /// menu; titles without an entry follow the default kind ranking.
    #[serde(default)]
    pub merged_overrides: std::collections::HashMap<String, String>,
    /// Vela's own "recently played" history feeding the Continue Watching
    /// hero (see `recents.rs`): item snapshots at play time, final position
    /// stamped at mpv exit, finished entries dropped.
    #[serde(default)]
    pub recents: Vec<crate::recents::RecentEntry>,
    /// Continue Watching tombstones: rating keys the user explicitly removed
    /// from the flow. The hero merge suppresses these even when a server hub
    /// still carries the item; replaying an item clears its tombstone.
    #[serde(default)]
    pub hidden_from_continue: Vec<String>,
    /// Per-library sort preference (owner ask 2026-07-10, "sort should stick
    /// per library"): source-namespaced section key → Vela sort key. Written
    /// on every sort change in a section view; stamped onto SectionDto when
    /// sections are listed. Values are re-validated against the sort
    /// whitelist on read; an unknown value invalidates settings. Entries for
    /// removed sections linger harmlessly (bounded by how many libraries
    /// existed).
    #[serde(default)]
    pub section_sorts: std::collections::BTreeMap<String, String>,
    /// Present only while the cross-file legacy Plex re-key is incomplete.
    /// Persisting the minted id before touching playlists makes the migration
    /// retry-safe across a crash between the two atomic JSON writes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) plex_source_migration: Option<PlexSourceMigration>,
    /// Private migration breadcrumb identifying the exact pre-split backup. It
    /// is present only while the connection split is incomplete and is
    /// removed with the legacy live credential fields.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) connections_split_backup: Option<ConnectionsSplitBackup>,
}

/// Play the file as it is — no conversion, and the only value that preserves
/// HDR passthrough. The default when the setting is absent.
pub const PLAYBACK_QUALITY_ORIGINAL: &str = "original";
/// Let mpv's own playback reporting decide, per the 2026-07-25 ruling. Opt-in,
/// never a default, because stepping down forfeits HDR.
pub const PLAYBACK_QUALITY_AUTOMATIC: &str = "automatic";

/// Resolve the stored playback quality, applying the documented default. The
/// only place that default is applied.
pub fn playback_quality(stored: Option<&str>) -> String {
    stored
        .filter(|value| !value.is_empty())
        .unwrap_or(PLAYBACK_QUALITY_ORIGINAL)
        .to_string()
}

/// Every value the quality setting may hold: the ladder plus the two non-tier
/// values. Built from `QUALITY_TIERS` so a tier that leaves the ladder cannot
/// survive anywhere as a value nothing can honour.
pub fn playback_quality_values() -> Vec<&'static str> {
    let mut values = vec![PLAYBACK_QUALITY_ORIGINAL, PLAYBACK_QUALITY_AUTOMATIC];
    values.extend(crate::source::QUALITY_TIERS.iter().map(|tier| tier.id));
    values
}

/// Whether a value is one Vela can actually play at. The one-off menu choice is
/// checked against exactly the set the stored setting is validated against, so
/// a frontend-invented value can never reach a source.
pub fn is_playback_quality(value: &str) -> bool {
    playback_quality_values().contains(&value)
}

/// What Vela does when playback enters a marker range of one kind: nothing,
/// offer the in-player skip button, or seek past it automatically.
///
/// This is a closed enum rather than the tolerant-string pattern the older
/// settings use, so an unrecognized stored value fails deserialization and
/// invalidates the settings document. The owner ruled on 2026-07-22 that Vela
/// must not guess what an unrecognized value meant.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SkipPolicy {
    Off,
    Button,
    Autoskip,
}

impl SkipPolicy {
    /// The owner-approved product default for every marker kind — intro and
    /// credits 2026-07-22, commercials 2026-07-23. A settings file that predates
    /// these fields gets the clickable prompt, never an automatic seek.
    pub const MISSING: SkipPolicy = SkipPolicy::Button;

    /// Resolve a stored value, applying the documented missing-field default.
    /// This is the only place that default is applied.
    pub fn resolve(stored: Option<SkipPolicy>) -> SkipPolicy {
        stored.unwrap_or(Self::MISSING)
    }

    /// The literal the bundled Lua script reads from mpv's script options. It
    /// matches the serde representation, and the closed set means no caller can
    /// put an arbitrary string into an mpv argument.
    pub fn as_option_value(self) -> &'static str {
        match self {
            SkipPolicy::Off => "off",
            SkipPolicy::Button => "button",
            SkipPolicy::Autoskip => "autoskip",
        }
    }

    pub fn is_off(self) -> bool {
        matches!(self, SkipPolicy::Off)
    }
}

/// The `mpv_window_mode` setting, resolved.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MpvWindowMode {
    Remember,
    Fullscreen,
    Default,
}

impl MpvWindowMode {
    /// Resolve a stored value. A missing value is remember (D2(a)); any other
    /// unknown value is an error, never a guess. The only place the default
    /// is applied.
    pub fn from_config(stored: Option<&str>) -> Result<Self, String> {
        match stored {
            None | Some("remember") => Ok(Self::Remember),
            Some("fullscreen") => Ok(Self::Fullscreen),
            Some("default") => Ok(Self::Default),
            Some(_) => Err("invalid saved player window setting".to_string()),
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Remember => "remember",
            Self::Fullscreen => "fullscreen",
            Self::Default => "default",
        }
    }
}

/// The longest screen name kept. mpv's names are short ("DP-3", or a model
/// and serial on macOS); anything longer is not one.
const MAX_SCREEN_NAME_LEN: usize = 256;

/// Whether a screen name may be stored and passed to mpv as `--screen-name`.
pub(crate) fn usable_screen_name(name: &str) -> bool {
    !name.trim().is_empty()
        && name.len() <= MAX_SCREEN_NAME_LEN
        && !name.chars().any(char::is_control)
}

/// How an mpv window was left: each part as last observed, `None` where it
/// never was. Width and height are the windowed size and come together.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct MpvWindow {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fullscreen: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub maximized: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub screen: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub width: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub height: Option<u32>,
}

impl MpvWindow {
    fn validate(&self) -> Result<(), String> {
        let size_ok = match (self.width, self.height) {
            (None, None) => true,
            (Some(width), Some(height)) => crate::playback::usable_windowed_size(width, height),
            _ => false,
        };
        if !size_ok
            || self
                .screen
                .as_deref()
                .is_some_and(|name| !usable_screen_name(name))
        {
            return Err("invalid saved player window".to_string());
        }
        Ok(())
    }

    /// The window state and screen the first play of a run opens with.
    pub(crate) fn opening(&self) -> (crate::playback::PlaybackWindowState, Option<String>) {
        (
            crate::playback::PlaybackWindowState {
                fullscreen: self.fullscreen,
                maximized: self.maximized,
                windowed_size: self.width.zip(self.height),
            },
            self.screen.clone(),
        )
    }

    /// This record updated with what a play that ended observed. A part the
    /// play did not observe keeps its stored value, so a play that reported
    /// nothing (one that failed at once, say) does not erase the record. An
    /// unusable screen name is not taken.
    pub(crate) fn updated_with(
        &self,
        ended: crate::playback::PlaybackWindowState,
        screen: Option<&str>,
    ) -> Self {
        let size = ended.windowed_size.or(self.width.zip(self.height));
        Self {
            fullscreen: ended.fullscreen.or(self.fullscreen),
            maximized: ended.maximized.or(self.maximized),
            screen: screen
                .filter(|name| usable_screen_name(name))
                .map(str::to_string)
                .or_else(|| self.screen.clone()),
            width: size.map(|(width, _)| width),
            height: size.map(|(_, height)| height),
        }
    }
}

/// A window state and the screen to open it on.
pub(crate) type OpeningWindow = (crate::playback::PlaybackWindowState, Option<String>);

/// How the first mpv window of a run of Vela opens, by the setting: as the
/// previous run left it, fullscreen and nothing else, or with nothing from
/// Vela (mpv's own default, or what the user's options say).
pub(crate) fn first_window_of_run(cfg: &AppConfig) -> Result<OpeningWindow, String> {
    Ok(
        match MpvWindowMode::from_config(cfg.mpv_window_mode.as_deref())? {
            MpvWindowMode::Remember => cfg
                .mpv_window
                .as_ref()
                .map(MpvWindow::opening)
                .unwrap_or_default(),
            MpvWindowMode::Fullscreen => (
                crate::playback::PlaybackWindowState {
                    fullscreen: Some(true),
                    ..Default::default()
                },
                None,
            ),
            MpvWindowMode::Default => OpeningWindow::default(),
        },
    )
}

/// What a play that ended in the external player leaves in the settings: its
/// window, in remember mode, and only when that changes what is stored. Any
/// other mode, and an embedded play (it has no mpv window), leave everything
/// as it is. Returns whether the record changed.
pub(crate) fn remember_mpv_window(
    cfg: &mut AppConfig,
    player: crate::player::PlayerKind,
    ended: crate::playback::PlaybackWindowState,
    screen: Option<&str>,
) -> bool {
    if player != crate::player::PlayerKind::External
        || MpvWindowMode::from_config(cfg.mpv_window_mode.as_deref()) != Ok(MpvWindowMode::Remember)
    {
        return false;
    }
    let updated = cfg
        .mpv_window
        .clone()
        .unwrap_or_default()
        .updated_with(ended, screen);
    // A play that observed nothing, with nothing stored, stores nothing. A
    // record the settings would reject is not stored either: it is written
    // with the play's recents stamp, which must not fail over it.
    let updated = (updated != MpvWindow::default()).then_some(updated);
    if updated == cfg.mpv_window
        || updated
            .as_ref()
            .is_some_and(|window| window.validate().is_err())
    {
        return false;
    }
    cfg.mpv_window = updated;
    true
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub(crate) struct PlexSourceMigration {
    pub(crate) from_id: String,
    pub(crate) to_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub(crate) struct ConnectionsSplitBackup {
    pub(crate) file_name: String,
    pub(crate) byte_length: u64,
    pub(crate) sha256: String,
}

/// A configured SMB share. INERT: kept only so old configs round-trip
/// (rollback-safe); no code mounts or browses these anymore. Every field —
/// including the legacy `kind`/`local_folder_id` pair — must survive
/// load→save (they were `skip_serializing` while a migrator handled them;
/// with the migrator gone, dropping them on save would lose rollback data).
#[derive(Clone, Serialize, Deserialize, Default)]
#[serde(default)] // missing fields fall back to Default rather than failing the parse
pub struct SmbMount {
    pub id: String,
    pub name: String,
    pub server: String,
    pub share: String,
    pub username: String,
    pub password: String,
    #[serde(default)]
    pub domain: String,
    pub mountpoint: String,
    #[serde(default)]
    pub folders: Vec<SmbFolder>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub kind: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub local_folder_id: String,
}

/// One selected folder inside an inert `SmbMount` record (see above).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct SmbFolder {
    pub id: String,
    pub name: String,
    pub path: String,
    #[serde(default)]
    pub kind: String,
}

/// A configured SSH/SFTP folder. INERT: kept only so old configs round-trip
/// (rollback-safe); no code mounts or browses these anymore. Vela stores no
/// SSH passwords.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)] // missing fields fall back to Default rather than failing the parse
pub struct SshMount {
    pub id: String,
    pub name: String,
    pub host: String,
    #[serde(default = "default_ssh_port")]
    pub port: u16,
    pub username: String,
    pub remote_path: String,
    #[serde(default)]
    pub identity_file: String,
    #[serde(default)]
    pub kind: String,
    /// Where sshfs mounts it (and the path the local folder points at).
    pub mountpoint: String,
    /// The `local_folders` entry this mount feeds, removed on unmount.
    pub local_folder_id: String,
}

fn default_ssh_port() -> u16 {
    22
}

/// A configured local folder. INERT: kept only so old configs round-trip
/// (rollback-safe); no code browses these anymore.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)] // missing fields fall back to Default rather than failing the parse
pub struct LocalFolder {
    pub id: String,
    pub name: String,
    pub path: String,
    #[serde(default)]
    pub kind: String,
}

/// Persisted connection for one configured source. Provider-neutral: `kind`
/// selects the backend, and the optional fields cover the union of what the
/// backends need (e.g. a user token vs. a pre-issued API key) without forcing
/// every backend into one exact shape.
#[derive(Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
#[serde(default, deny_unknown_fields)]
pub struct SourceConfig {
    pub id: String,
    pub kind: String,
    pub name: String,
    pub base_url: String,
    #[serde(default)]
    pub access_token: Option<String>,
    #[serde(default)]
    pub api_key: Option<String>,
    #[serde(default)]
    pub user_id: Option<String>,
    #[serde(default)]
    pub device_id: Option<String>,
    /// Plex's stable physical-server identity. Other providers leave this
    /// empty. A restored Plex endpoint carrying this value is pinned before
    /// any rediscovery can select among the account's other servers.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub machine_identifier: Option<String>,
}

impl std::fmt::Debug for SourceConfig {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("SourceConfig")
            .field("id", &self.id)
            .field("kind", &self.kind)
            .field("name", &self.name)
            .field("base_url", &"<redacted>")
            .field(
                "access_token",
                &self.access_token.as_ref().map(|_| "<redacted>"),
            )
            .field("api_key", &self.api_key.as_ref().map(|_| "<redacted>"))
            .field("user_id", &self.user_id.as_ref().map(|_| "<redacted>"))
            .field("device_id", &self.device_id.as_ref().map(|_| "<redacted>"))
            .field("machine_identifier", &self.machine_identifier)
            .finish()
    }
}

pub(crate) const ALLOWED_SECTION_SORTS: &[&str] = &[
    "titleSort:asc",
    "titleSort:desc",
    "year:asc",
    "year:desc",
    "addedAt:asc",
    "addedAt:desc",
    "episodeAddedAt:asc",
    "episodeAddedAt:desc",
    "originallyAvailableAt:asc",
    "originallyAvailableAt:desc",
    "rating:asc",
    "rating:desc",
    "lastViewedAt:asc",
    "lastViewedAt:desc",
];

impl AppConfig {
    pub(crate) fn validate(&self) -> Result<(), String> {
        if self.auth_token.is_some() != self.client_identifier.is_some() {
            return Err("incomplete legacy Plex credentials".to_string());
        }
        let legacy_endpoint_fields = [
            self.last_server_host.is_some(),
            self.last_server_port.is_some(),
            self.last_server_scheme.is_some(),
        ];
        if legacy_endpoint_fields.iter().any(|present| *present)
            && !legacy_endpoint_fields.iter().all(|present| *present)
        {
            return Err("incomplete legacy Plex endpoint".to_string());
        }
        if self
            .last_server_scheme
            .as_deref()
            .is_some_and(|scheme| !matches!(scheme, "http" | "https"))
        {
            return Err("invalid legacy Plex endpoint".to_string());
        }
        crate::connections::ConnectionsConfig {
            sources: self.sources.clone(),
        }
        .validate()?;
        if let Some(migration) = &self.plex_source_migration {
            if migration.from_id != LEGACY_PLEX_SOURCE_ID
                || migration.to_id.trim().is_empty()
                || migration.to_id.contains(':')
                || !self
                    .sources
                    .iter()
                    .any(|source| source.kind == "plex" && source.id == migration.to_id)
            {
                return Err("invalid partial Plex source migration".to_string());
            }
        }
        if self.connections_split_backup.is_some() && !has_legacy_connections(self) {
            return Err("invalid connection-split migration state".to_string());
        }
        if self
            .watched_threshold_percent
            .is_some_and(|value| !(1..=100).contains(&value))
        {
            return Err("invalid watched threshold".to_string());
        }
        validate_optional_closed(
            "mpv autocrop",
            self.mpv_autocrop.as_deref(),
            &["off", "manual", "auto"],
        )?;
        validate_optional_closed("player", self.player.as_deref(), &["external", "embedded"])?;
        validate_optional_closed(
            "Continue Playing",
            self.continue_playing.as_deref(),
            &["off", "on", "only-tv"],
        )?;
        validate_optional_closed(
            "player window setting",
            self.mpv_window_mode.as_deref(),
            &["remember", "fullscreen", "default"],
        )?;
        if let Some(window) = &self.mpv_window {
            window.validate()?;
        }
        validate_optional_closed(
            "playback source policy",
            self.playback_source_policy.as_deref(),
            &["best", "compatible", "fastest", "ask"],
        )?;
        // The allowed set is the ladder itself plus the two non-tier values, so
        // a tier that leaves the ladder can never linger as a valid setting.
        validate_optional_closed(
            "playback quality",
            self.playback_quality.as_deref(),
            &playback_quality_values(),
        )?;
        validate_optional_closed(
            "display resolution",
            self.playback_display_resolution.as_deref(),
            &["720p", "1080p", "1440p", "2160p", "4320p"],
        )?;
        validate_optional_closed(
            "display HDR",
            self.playback_display_hdr.as_deref(),
            &["enabled", "disabled"],
        )?;
        if self.recents.len() > crate::recents::MAX_RECENTS {
            return Err("too many recent items".to_string());
        }
        if self.hidden_from_continue.len() > crate::recents::MAX_HIDDEN {
            return Err("too many hidden Continue Watching items".to_string());
        }
        for (key, value) in &self.section_sorts {
            if key.trim().is_empty()
                || key.len() > 512
                || !ALLOWED_SECTION_SORTS.contains(&value.as_str())
            {
                return Err("invalid saved library sort".to_string());
            }
        }
        if let Some(backup) = &self.connections_split_backup {
            if !backup
                .file_name
                .starts_with("config.pre-connections-split-")
                || !backup.file_name.ends_with(".json")
                || backup.file_name.contains('/')
                || backup.file_name.contains('\\')
                || backup.sha256.len() != 64
                || !backup
                    .sha256
                    .bytes()
                    .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
            {
                return Err("invalid connection-split migration state".to_string());
            }
        }
        Ok(())
    }
}

fn validate_optional_closed(
    label: &str,
    value: Option<&str>,
    allowed: &[&str],
) -> Result<(), String> {
    if value.is_some_and(|value| !allowed.contains(&value)) {
        Err(format!("invalid {label}"))
    } else {
        Ok(())
    }
}

/// A file inside the app's config dir, creating the dir if needed. Surfaces a
/// real IO error rather than collapsing it into "no path".
pub fn config_dir_file(name: &str) -> io::Result<PathBuf> {
    crate::storage::config_dir_file(name)
}

pub(crate) fn config_path() -> io::Result<PathBuf> {
    config_dir_file("config.json")
}

/// Atomically load → modify → save the config under a process-wide lock. `f`
/// can validate and return an error (no save happens then) and may return a
/// value (e.g. a clone of the updated config). This is fully synchronous, so
/// callers must not hold its result-producing closure across an await.
pub fn update<T, F>(f: F) -> Result<T, String>
where
    F: FnOnce(&mut AppConfig) -> Result<T, String>,
{
    crate::durable::ensure_commands_ready()?;
    let result = update_internal(f);
    if result.is_err() {
        if let Ok(path) = config_path() {
            if let Err(error) = load_unmigrated_at(&path) {
                crate::durable::report_settings_fault(&error);
            }
        }
    }
    result
}

pub(crate) fn update_internal<T, F>(f: F) -> Result<T, String>
where
    F: FnOnce(&mut AppConfig) -> Result<T, String>,
{
    let path = config_path().map_err(|error| format!("config unavailable: {error}"))?;
    let lock_path = config_dir_file("config.lock")
        .map_err(|error| format!("config lock unavailable: {error}"))?;
    update_at(&path, &lock_path, f)
}

pub(crate) fn update_at<T, F>(path: &Path, lock_path: &Path, f: F) -> Result<T, String>
where
    F: FnOnce(&mut AppConfig) -> Result<T, String>,
{
    crate::storage::update_json_before_save(
        "config",
        &CONFIG_LOCK,
        path,
        lock_path,
        |cfg: &mut AppConfig| {
            cfg.validate()?;
            sanitize_legacy_artwork(cfg);
            let output = f(cfg)?;
            sanitize_legacy_artwork(cfg);
            cfg.validate()?;
            Ok(output)
        },
        |original| match original {
            Some(bytes) => crate::durable::preserve_valid_history(
                crate::durable::DurableFile::Settings,
                path,
                bytes,
            ),
            None => Ok(()),
        },
    )
}

pub(crate) fn sanitize_legacy_artwork(cfg: &mut AppConfig) -> bool {
    let mut changed = false;
    for recent in &mut cfg.recents {
        changed |= crate::artwork::sanitize_item_artwork(&mut recent.item);
    }
    changed
}

pub(crate) fn lock_process() -> std::sync::MutexGuard<'static, ()> {
    CONFIG_LOCK
        .lock()
        .unwrap_or_else(|error| error.into_inner())
}

fn rekey_namespaced(value: &mut String, old_source_id: &str, new_source_id: &str) {
    let Some((source_id, raw)) = value.split_once(':') else {
        return;
    };
    if source_id == old_source_id {
        *value = crate::source::namespace_key(new_source_id, raw);
    }
}

fn rekey_config_references(cfg: &mut AppConfig, old_source_id: &str, new_source_id: &str) {
    if let Some(section) = &mut cfg.last_section_key {
        rekey_namespaced(section, old_source_id, new_source_id);
    }
    for source_id in cfg.merged_overrides.values_mut() {
        if source_id == old_source_id {
            *source_id = new_source_id.to_string();
        }
    }
    for recent in &mut cfg.recents {
        crate::source::rekey_item_source(&mut recent.item, old_source_id, new_source_id);
    }
    for key in &mut cfg.hidden_from_continue {
        rekey_namespaced(key, old_source_id, new_source_id);
    }

    let old_sort_keys = cfg
        .section_sorts
        .keys()
        .filter(|key| {
            key.split_once(':')
                .is_some_and(|(source_id, _)| source_id == old_source_id)
        })
        .cloned()
        .collect::<Vec<_>>();
    for old_key in old_sort_keys {
        if let Some(value) = cfg.section_sorts.remove(&old_key) {
            let mut new_key = old_key;
            rekey_namespaced(&mut new_key, old_source_id, new_source_id);
            cfg.section_sorts.entry(new_key).or_insert(value);
        }
    }
}

fn legacy_server_base(cfg: &AppConfig) -> Option<String> {
    let host = cfg.last_server_host.as_deref()?;
    let port = cfg.last_server_port?;
    let scheme = cfg.last_server_scheme.as_deref()?;
    if !matches!(scheme, "http" | "https") {
        return None;
    }
    let host = if host.contains(':') && !host.starts_with('[') {
        format!("[{host}]")
    } else {
        host.to_string()
    };
    Some(format!("{scheme}://{host}:{port}"))
}

pub(crate) fn migration_needed(cfg: &AppConfig) -> bool {
    cfg.plex_source_migration.is_some()
        || (cfg.auth_token.is_some() && cfg.client_identifier.is_some())
        || cfg
            .sources
            .iter()
            .any(|source| source.kind == "plex" && source.id == LEGACY_PLEX_SOURCE_ID)
}

pub(crate) fn has_legacy_connections(cfg: &AppConfig) -> bool {
    migration_needed(cfg) || !cfg.sources.is_empty()
}

pub(crate) fn load_unmigrated_at(path: &Path) -> io::Result<AppConfig> {
    let cfg: AppConfig = crate::storage::load_json(path)?;
    cfg.validate()
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "invalid settings document"))?;
    Ok(cfg)
}

pub(crate) fn prepare_legacy_plex_migration(
    cfg: &mut AppConfig,
    make_id: impl FnOnce() -> String,
) -> Result<Option<PlexSourceMigration>, String> {
    if let Some(migration) = cfg.plex_source_migration.clone() {
        if migration.from_id != LEGACY_PLEX_SOURCE_ID
            || !cfg
                .sources
                .iter()
                .any(|source| source.kind == "plex" && source.id == migration.to_id)
        {
            return Err("invalid partial Plex source migration in config".to_string());
        }
        rekey_config_references(cfg, &migration.from_id, &migration.to_id);
        return Ok(Some(migration));
    }

    let legacy_positions = cfg
        .sources
        .iter()
        .enumerate()
        .filter_map(|(index, source)| {
            (source.kind == "plex" && source.id == LEGACY_PLEX_SOURCE_ID).then_some(index)
        })
        .collect::<Vec<_>>();
    if legacy_positions.len() > 1 {
        return Err("multiple persisted Plex sources use the retired `plex` id".to_string());
    }
    if legacy_positions.is_empty() && !(cfg.auth_token.is_some() && cfg.client_identifier.is_some())
    {
        return Ok(None);
    }

    let new_id = make_id();
    if new_id.is_empty()
        || new_id.contains(':')
        || new_id == LEGACY_PLEX_SOURCE_ID
        || cfg.sources.iter().any(|source| source.id == new_id)
    {
        return Err("could not mint a unique Plex source id".to_string());
    }

    let legacy_base = legacy_server_base(cfg);
    let legacy_token = cfg.auth_token.clone();
    let legacy_client = cfg.client_identifier.clone();
    if let Some(index) = legacy_positions.into_iter().next() {
        let source = &mut cfg.sources[index];
        source.id = new_id.clone();
        if source.name.trim().is_empty() {
            source.name = "Plex".to_string();
        }
        if source.base_url.trim().is_empty() {
            source.base_url = legacy_base.unwrap_or_default();
        }
        if source.access_token.is_none() {
            source.access_token = legacy_token;
        }
        if source.device_id.is_none() {
            source.device_id = legacy_client;
        }
    } else {
        cfg.sources.push(SourceConfig {
            id: new_id.clone(),
            kind: "plex".to_string(),
            name: "Plex".to_string(),
            base_url: legacy_base.unwrap_or_default(),
            access_token: legacy_token,
            api_key: None,
            user_id: None,
            device_id: legacy_client,
            machine_identifier: None,
        });
    }

    rekey_config_references(cfg, LEGACY_PLEX_SOURCE_ID, &new_id);
    let migration = PlexSourceMigration {
        from_id: LEGACY_PLEX_SOURCE_ID.to_string(),
        to_id: new_id,
    };
    cfg.plex_source_migration = Some(migration.clone());
    Ok(Some(migration))
}

pub(crate) fn finish_legacy_plex_migration(
    cfg: &mut AppConfig,
    migration: &PlexSourceMigration,
) -> Result<(), String> {
    match cfg.plex_source_migration.as_ref() {
        Some(current) if current == migration => {}
        None if cfg
            .sources
            .iter()
            .any(|source| source.kind == "plex" && source.id == migration.to_id) =>
        {
            return Ok(());
        }
        _ => return Err("Plex source migration changed while it was running".to_string()),
    }
    cfg.auth_token = None;
    cfg.client_identifier = None;
    cfg.last_server_host = None;
    cfg.last_server_port = None;
    cfg.last_server_scheme = None;
    cfg.plex_source_migration = None;
    Ok(())
}

fn migrate_legacy_plex_with(
    path: &Path,
    lock_path: &Path,
    make_id: impl FnOnce() -> String,
    migrate_playlists: impl FnOnce(&str, &str) -> Result<(), String>,
) -> Result<(), String> {
    let migration = update_at(path, lock_path, |cfg| {
        prepare_legacy_plex_migration(cfg, make_id)
    })?;
    let Some(migration) = migration else {
        return Ok(());
    };
    migrate_playlists(&migration.from_id, &migration.to_id)?;
    update_at(path, lock_path, |cfg| {
        finish_legacy_plex_migration(cfg, &migration)
    })
}

pub(crate) fn load_config_with(
    path: &Path,
    lock_path: &Path,
    make_id: impl FnOnce() -> String,
    migrate_playlists: impl FnOnce(&str, &str) -> Result<(), String>,
) -> io::Result<AppConfig> {
    let cfg: AppConfig = crate::storage::load_json(path)?;
    cfg.validate()
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "invalid settings document"))?;
    if !migration_needed(&cfg) {
        return Ok(cfg);
    }
    migrate_legacy_plex_with(path, lock_path, make_id, migrate_playlists)
        .map_err(io::Error::other)?;
    let cfg: AppConfig = crate::storage::load_json(path)?;
    cfg.validate()
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "invalid settings document"))?;
    Ok(cfg)
}

pub(crate) fn load_config_internal() -> io::Result<AppConfig> {
    let path = config_path()?;
    let lock_path = config_dir_file("config.lock")?;
    load_config_with(
        &path,
        &lock_path,
        || format!("plex-{}", uuid::Uuid::new_v4()),
        crate::playlists::migrate_source_id,
    )
}

pub fn load_config() -> io::Result<AppConfig> {
    crate::durable::ensure_commands_ready().map_err(io::Error::other)?;
    let result = load_config_internal();
    if let Err(error) = &result {
        crate::durable::report_settings_fault(error);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::playlists::{Playlist, PlaylistEntry, PlaylistFile};
    use crate::recents::RecentEntry;
    use crate::source::{BackingRef, ItemDto};
    use std::fs;

    fn item(source_id: &str) -> ItemDto {
        ItemDto {
            rating_key: format!("{source_id}:1"),
            title: "Migrated movie".to_string(),
            year: Some(2026),
            summary: None,
            duration_ms: Some(60_000),
            media_type: Some("movie".to_string()),
            poster: None,
            series_poster: None,
            backdrop: None,
            view_offset_ms: Some(1_000),
            played: Some(false),
            last_watched_at_ms: None,
            added_at_ms: None,
            index: None,
            parent_index: None,
            grandparent_title: None,
            parent_title: None,
            parent_rating_key: Some(format!("{source_id}:2")),
            grandparent_rating_key: Some(format!("{source_id}:3")),
            source_id: source_id.to_string(),
            provider_ids: vec!["imdb:tt1".to_string()],
            backing: Some(vec![
                BackingRef {
                    source_id: source_id.to_string(),
                    rating_key: format!("{source_id}:1"),
                    parent_rating_key: Some(format!("{source_id}:2")),
                    grandparent_rating_key: Some(format!("{source_id}:3")),
                },
                BackingRef {
                    source_id: "jf".to_string(),
                    rating_key: "jf:other".to_string(),
                    parent_rating_key: None,
                    grandparent_rating_key: None,
                },
            ]),
            canonical_id: Some("imdb:tt1".to_string()),
            watch_key: Some(format!("{source_id}:4")),
            detail_key: Some(format!("{source_id}:5")),
        }
    }

    fn temp_paths(label: &str) -> (PathBuf, PathBuf, PathBuf, PathBuf, PathBuf) {
        let root =
            std::env::temp_dir().join(format!("vela-config-{label}-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        (
            root.join("config.json"),
            root.join("config.lock"),
            root.join("playlists.json"),
            root.join("playlists.lock"),
            root,
        )
    }

    fn legacy_config() -> AppConfig {
        let mut cfg = AppConfig {
            auth_token: Some("legacy-token".to_string()),
            client_identifier: Some("legacy-client".to_string()),
            last_server_host: Some("plex.example".to_string()),
            last_server_port: Some(443),
            last_server_scheme: Some("https".to_string()),
            last_section_key: Some("plex:7".to_string()),
            recents: vec![RecentEntry {
                item: item("plex"),
                session_id: Some("session".to_string()),
                started_at_ms: 1,
                ended_at_ms: 2,
            }],
            hidden_from_continue: vec!["plex:6".to_string(), "jf:keep".to_string()],
            ..Default::default()
        };
        cfg.merged_overrides
            .insert("imdb:tt1".to_string(), "plex".to_string());
        cfg.section_sorts
            .insert("plex:7".to_string(), "titleSort:asc".to_string());
        cfg
    }

    #[test]
    fn ordinary_settings_write_removes_legacy_plex_artwork_tokens() {
        let (config, config_lock, _, _, root) = temp_paths("artwork-sanitize");
        let token = "synthetic-legacy-artwork-token";
        let mut recent_item = item("plex-a");
        recent_item.poster = Some(format!(
            "https://plex.example/photo/:/transcode?width=300&height=450&\
             url=%2Flibrary%2Fmetadata%2F1%2Fthumb%2F2&X-Plex-Token={token}"
        ));
        crate::storage::save_json(
            &config,
            &AppConfig {
                recents: vec![RecentEntry {
                    item: recent_item,
                    session_id: None,
                    started_at_ms: 1,
                    ended_at_ms: 2,
                }],
                ..Default::default()
            },
        )
        .unwrap();

        update_at(&config, &config_lock, |_| Ok(())).unwrap();
        let saved = fs::read_to_string(&config).unwrap();
        assert!(!saved.contains(token));
        assert!(saved.contains(crate::artwork::ARTWORK_MARKER_PREFIX));
        fs::remove_dir_all(root).unwrap();
    }

    fn playlist_file() -> PlaylistFile {
        PlaylistFile {
            schema_version: 1,
            playlists: vec![Playlist {
                id: "playlist".to_string(),
                name: "Migration".to_string(),
                items: vec![PlaylistEntry {
                    id: "entry".to_string(),
                    item: item("plex"),
                    source_name: Some("Plex".to_string()),
                }],
                created_ms: 1,
                updated_ms: 1,
            }],
        }
    }

    #[test]
    fn legacy_plex_migration_rekeys_every_persisted_route_and_is_idempotent() {
        let (config, config_lock, playlists, playlists_lock, root) = temp_paths("plex-migration");
        crate::storage::save_json(&config, &legacy_config()).unwrap();
        crate::storage::save_json(&playlists, &playlist_file()).unwrap();

        let migrated = load_config_with(
            &config,
            &config_lock,
            || "plex-new".to_string(),
            |old, new| {
                crate::playlists::migrate_source_id_at(&playlists, &playlists_lock, old, new)
            },
        )
        .unwrap();

        assert_eq!(migrated.sources.len(), 1);
        let source = &migrated.sources[0];
        assert_eq!(source.id, "plex-new");
        assert_eq!(source.kind, "plex");
        assert_eq!(source.base_url, "https://plex.example:443");
        assert_eq!(source.access_token.as_deref(), Some("legacy-token"));
        assert_eq!(source.device_id.as_deref(), Some("legacy-client"));
        assert_eq!(source.machine_identifier, None);
        assert_eq!(migrated.auth_token, None);
        assert_eq!(migrated.client_identifier, None);
        assert_eq!(migrated.last_server_host, None);
        assert_eq!(migrated.plex_source_migration, None);
        assert_eq!(migrated.last_section_key.as_deref(), Some("plex-new:7"));
        assert_eq!(
            migrated
                .merged_overrides
                .get("imdb:tt1")
                .map(String::as_str),
            Some("plex-new")
        );
        assert_eq!(
            migrated.section_sorts.get("plex-new:7").map(String::as_str),
            Some("titleSort:asc")
        );
        assert_eq!(
            migrated.hidden_from_continue,
            vec!["plex-new:6".to_string(), "jf:keep".to_string()]
        );
        let recent = &migrated.recents[0].item;
        assert_eq!(recent.rating_key, "plex-new:1");
        assert_eq!(recent.source_id, "plex-new");
        assert_eq!(recent.parent_rating_key.as_deref(), Some("plex-new:2"));
        assert_eq!(recent.grandparent_rating_key.as_deref(), Some("plex-new:3"));
        assert_eq!(recent.watch_key.as_deref(), Some("plex-new:4"));
        assert_eq!(recent.detail_key.as_deref(), Some("plex-new:5"));
        assert_eq!(recent.backing.as_ref().unwrap()[0].source_id, "plex-new");
        assert_eq!(recent.backing.as_ref().unwrap()[0].rating_key, "plex-new:1");
        assert_eq!(
            recent.backing.as_ref().unwrap()[0]
                .parent_rating_key
                .as_deref(),
            Some("plex-new:2")
        );
        assert_eq!(
            recent.backing.as_ref().unwrap()[0]
                .grandparent_rating_key
                .as_deref(),
            Some("plex-new:3")
        );
        assert_eq!(recent.backing.as_ref().unwrap()[1].rating_key, "jf:other");

        let migrated_playlists: PlaylistFile = crate::storage::load_json(&playlists).unwrap();
        let playlist_item = &migrated_playlists.playlists[0].items[0].item;
        assert_eq!(playlist_item.source_id, "plex-new");
        assert_eq!(playlist_item.rating_key, "plex-new:1");
        assert_eq!(playlist_item.watch_key.as_deref(), Some("plex-new:4"));

        let config_bytes = fs::read(&config).unwrap();
        let playlist_bytes = fs::read(&playlists).unwrap();
        let loaded_again = load_config_with(
            &config,
            &config_lock,
            || panic!("a completed migration must never mint another id"),
            |old, new| {
                crate::playlists::migrate_source_id_at(&playlists, &playlists_lock, old, new)
            },
        )
        .unwrap();
        assert_eq!(loaded_again.sources[0].id, "plex-new");
        assert_eq!(fs::read(&config).unwrap(), config_bytes);
        assert_eq!(fs::read(&playlists).unwrap(), playlist_bytes);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn failed_playlist_rekey_keeps_a_retryable_config_marker_and_id() {
        let (config, config_lock, playlists, playlists_lock, root) =
            temp_paths("plex-migration-retry");
        crate::storage::save_json(&config, &legacy_config()).unwrap();
        fs::write(&playlists, b"{not json").unwrap();
        let bad_playlist = fs::read(&playlists).unwrap();

        let first = load_config_with(
            &config,
            &config_lock,
            || "plex-first".to_string(),
            |old, new| {
                crate::playlists::migrate_source_id_at(&playlists, &playlists_lock, old, new)
            },
        );
        assert!(first.is_err());
        assert_eq!(fs::read(&playlists).unwrap(), bad_playlist);
        let partial: AppConfig = crate::storage::load_json(&config).unwrap();
        assert_eq!(partial.sources[0].id, "plex-first");
        assert_eq!(partial.auth_token.as_deref(), Some("legacy-token"));
        assert_eq!(
            partial.plex_source_migration,
            Some(PlexSourceMigration {
                from_id: "plex".to_string(),
                to_id: "plex-first".to_string(),
            })
        );

        crate::storage::save_json(&playlists, &playlist_file()).unwrap();
        let complete = load_config_with(
            &config,
            &config_lock,
            || panic!("a retry must reuse the persisted target id"),
            |old, new| {
                crate::playlists::migrate_source_id_at(&playlists, &playlists_lock, old, new)
            },
        )
        .unwrap();
        assert_eq!(complete.sources[0].id, "plex-first");
        assert_eq!(complete.auth_token, None);
        assert_eq!(complete.plex_source_migration, None);
        let migrated_playlists: PlaylistFile = crate::storage::load_json(&playlists).unwrap();
        assert_eq!(
            migrated_playlists.playlists[0].items[0].item.rating_key,
            "plex-first:1"
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn migration_does_not_create_an_absent_playlist_store() {
        let (config, config_lock, playlists, playlists_lock, root) =
            temp_paths("plex-migration-no-playlists");
        crate::storage::save_json(&config, &legacy_config()).unwrap();
        load_config_with(
            &config,
            &config_lock,
            || "plex-new".to_string(),
            |old, new| {
                crate::playlists::migrate_source_id_at(&playlists, &playlists_lock, old, new)
            },
        )
        .unwrap();
        assert!(!playlists.exists());
        fs::remove_dir_all(root).unwrap();
    }

    // Per-library sort preferences: absent in old configs (defaults empty);
    // explicit field/direction tokens validate and round-trip unchanged.
    #[test]
    fn section_sorts_default_empty_and_round_trip() {
        let old: AppConfig = serde_json::from_str(r#"{"auth_token":"tok"}"#).expect("parses");
        assert!(old.section_sorts.is_empty(), "missing field defaults empty");

        let mut cfg = AppConfig::default();
        cfg.section_sorts
            .insert("plex-1:6".into(), "episodeAddedAt:asc".into());
        cfg.section_sorts
            .insert("jf-1:7".into(), "rating:asc".into());
        cfg.validate().expect("ascending sort tokens validate");
        let saved = serde_json::to_string(&cfg).expect("serializes");
        let back: AppConfig = serde_json::from_str(&saved).expect("round-trips");
        back.validate().expect("round-tripped sorts still validate");
        assert_eq!(
            back.section_sorts.get("plex-1:6").map(String::as_str),
            Some("episodeAddedAt:asc")
        );
        assert_eq!(
            back.section_sorts.get("jf-1:7").map(String::as_str),
            Some("rating:asc")
        );
    }

    // An install that predates transcoding keeps direct play, which is also the
    // only value that preserves HDR passthrough. Defaulting any other way would
    // silently convert video for people who never asked.
    #[test]
    fn missing_playback_quality_means_original() {
        let old: AppConfig = serde_json::from_str(r#"{"auth_token":"tok"}"#).expect("parses");
        assert_eq!(old.playback_quality, None, "absence stays absence");
        assert_eq!(playback_quality(old.playback_quality.as_deref()), "original");
        assert_eq!(playback_quality(None), "original");
        assert_eq!(playback_quality(Some("")), "original", "empty is not a tier");
        assert_eq!(playback_quality(Some("automatic")), "automatic");
    }

    // The setting's valid set is the ladder itself, so a tier that ever leaves
    // the ladder cannot linger in a config as a value nothing can honour.
    #[test]
    fn playback_quality_accepts_only_ladder_values() {
        for valid in ["original", "automatic", "1080p-8000", "328p-700"] {
            let cfg = AppConfig {
                playback_quality: Some(valid.to_string()),
                ..Default::default()
            };
            assert!(cfg.validate().is_ok(), "{valid} should be accepted");
        }
        for invalid in ["1080p", "8000", "Original", "best", "1080p-9999"] {
            let cfg = AppConfig {
                playback_quality: Some(invalid.to_string()),
                ..Default::default()
            };
            assert!(
                cfg.validate().is_err(),
                "{invalid} must be rejected, not normalized"
            );
        }
    }

    // A settings file written before marker skipping existed is valid, and
    // every kind resolves to the owner-approved Button prompt — never to an
    // automatic seek the user never asked for.
    #[test]
    fn missing_marker_policies_resolve_to_the_approved_button_default() {
        let old: AppConfig = serde_json::from_str(r#"{"auth_token":"tok"}"#).expect("parses");
        assert_eq!(old.skip_intros, None, "absence is preserved as absence");
        assert_eq!(old.skip_credits, None);
        assert_eq!(old.skip_commercials, None);

        for stored in [old.skip_intros, old.skip_credits, old.skip_commercials] {
            assert_eq!(
                SkipPolicy::resolve(stored),
                SkipPolicy::Button,
                "a missing marker policy must mean the prompt, not a seek"
            );
        }
    }

    // Missing and invalid are different things: an explicit value outside the
    // closed enum invalidates the whole document rather than being normalized.
    #[test]
    fn unknown_marker_policy_values_invalidate_the_document() {
        for invalid in [
            r#"{"skip_intros":"skip"}"#,
            r#"{"skip_credits":"Button"}"#,
            r#"{"skip_commercials":true}"#,
            r#"{"skip_intros":"auto_skip"}"#,
        ] {
            assert!(
                serde_json::from_str::<AppConfig>(invalid).is_err(),
                "{invalid} must fail the whole settings document"
            );
        }
        assert_eq!(
            serde_json::from_str::<AppConfig>(r#"{"skip_intros":"autoskip"}"#)
                .expect("the closed enum still accepts its own values")
                .skip_intros,
            Some(SkipPolicy::Autoskip)
        );
    }

    // The rollback rail and the new marker fields must coexist: setting a skip
    // policy must not disturb the inert local/SMB/SSH fields an older build
    // still expects to find, credentials included.
    #[test]
    fn marker_policies_leave_the_legacy_rollback_fields_untouched() {
        let legacy = r#"{
            "skip_intros": "autoskip",
            "playback_quality": "720p-2000",
            "local_folders": [
                {"id": "legacy-folder", "name": "Movies", "path": "/Volumes/media", "kind": "movie"}
            ],
            "smb_mounts": [
                {"id": "mount", "name": "Media", "server": "nas", "share": "media",
                 "username": "user", "password": "pass", "mountpoint": "/Volumes/media",
                 "kind": "movie", "local_folder_id": "legacy-folder"}
            ],
            "ssh_mounts": [
                {"id": "sshm", "name": "NAS ssh", "host": "nas", "port": 22,
                 "username": "user", "remote_path": "/srv/media",
                 "mountpoint": "/mnt/vela-ssh", "local_folder_id": "lf-ssh"}
            ]
        }"#;
        let cfg: AppConfig = serde_json::from_str(legacy).expect("parses");
        let saved = serde_json::to_string(&cfg).expect("serializes");
        let back: AppConfig = serde_json::from_str(&saved).expect("round-trips");

        assert_eq!(back.skip_intros, Some(SkipPolicy::Autoskip));
        assert_eq!(back.playback_quality.as_deref(), Some("720p-2000"));
        assert_eq!(back.local_folders[0].path, "/Volumes/media");
        assert_eq!(back.smb_mounts[0].local_folder_id, "legacy-folder");
        assert_eq!(
            back.smb_mounts[0].password, "pass",
            "rollback credentials survive alongside the new fields"
        );
        assert_eq!(back.ssh_mounts[0].mountpoint, "/mnt/vela-ssh");
    }

    #[test]
    fn marker_policies_round_trip_every_value() {
        let cfg = AppConfig {
            skip_intros: Some(SkipPolicy::Off),
            skip_credits: Some(SkipPolicy::Button),
            skip_commercials: Some(SkipPolicy::Autoskip),
            ..Default::default()
        };
        let saved = serde_json::to_string(&cfg).expect("serializes");
        let back: AppConfig = serde_json::from_str(&saved).expect("round-trips");
        assert_eq!(back.skip_intros, Some(SkipPolicy::Off));
        assert_eq!(back.skip_credits, Some(SkipPolicy::Button));
        assert_eq!(back.skip_commercials, Some(SkipPolicy::Autoskip));
    }

    #[test]
    fn playback_preferences_are_optional_and_round_trip_without_affecting_old_configs() {
        let old: AppConfig = serde_json::from_str(r#"{"auth_token":"tok"}"#).expect("parses");
        assert_eq!(old.playback_source_policy, None);
        assert_eq!(old.playback_display_resolution, None);
        assert_eq!(old.playback_display_hdr, None);

        let cfg = AppConfig {
            playback_source_policy: Some("ask".to_string()),
            playback_display_resolution: Some("2160p".to_string()),
            playback_display_hdr: Some("disabled".to_string()),
            ..Default::default()
        };
        let saved = serde_json::to_string(&cfg).expect("serializes");
        let back: AppConfig = serde_json::from_str(&saved).expect("round-trips");
        assert_eq!(back.playback_source_policy.as_deref(), Some("ask"));
        assert_eq!(back.playback_display_resolution.as_deref(), Some("2160p"));
        assert_eq!(back.playback_display_hdr.as_deref(), Some("disabled"));
    }

    // Rollback rail for the 2026-07-08 local-source removal: every inert
    // local-family field — including the legacy pre-migration SmbMount shape
    // (`kind`/`local_folder_id` set, no `folders`) — must survive a
    // serde round trip unchanged. This fails if the old `skip_serializing`
    // attrs return. (It exercises the serde layer directly — the same layer
    // load_config/save_config use — not the file I/O around it.)
    #[test]
    fn inert_local_family_config_round_trips_unchanged() {
        let legacy = r#"{
            "auth_token": "tok",
            "local_folders": [
                {"id": "legacy-folder", "name": "Movies", "path": "/Volumes/media", "kind": "movie"}
            ],
            "smb_mounts": [
                {"id": "mount", "name": "Media", "server": "nas", "share": "media",
                 "username": "user", "password": "pass", "mountpoint": "/Volumes/media",
                 "kind": "movie", "local_folder_id": "legacy-folder"}
            ],
            "ssh_mounts": [
                {"id": "sshm", "name": "NAS ssh", "host": "nas", "port": 22,
                 "username": "user", "remote_path": "/srv/media",
                 "mountpoint": "/mnt/vela-ssh", "local_folder_id": "lf-ssh"}
            ]
        }"#;
        let cfg: AppConfig = serde_json::from_str(legacy).expect("legacy config parses");

        // Loaded untouched: no migrator moves/strips legacy fields anymore.
        assert_eq!(
            cfg.local_folders.len(),
            1,
            "local_folders preserved on load"
        );
        assert_eq!(cfg.smb_mounts[0].kind, "movie");
        assert_eq!(cfg.smb_mounts[0].local_folder_id, "legacy-folder");
        assert!(
            cfg.smb_mounts[0].folders.is_empty(),
            "no synthesized folders"
        );

        // Saved with everything intact: a rollback build sees its data.
        let saved = serde_json::to_string(&cfg).expect("serializes");
        let back: AppConfig = serde_json::from_str(&saved).expect("round-trips");
        assert_eq!(back.local_folders.len(), 1);
        assert_eq!(back.local_folders[0].path, "/Volumes/media");
        assert_eq!(
            back.smb_mounts[0].kind, "movie",
            "legacy kind survives save"
        );
        assert_eq!(
            back.smb_mounts[0].local_folder_id, "legacy-folder",
            "legacy local_folder_id survives save"
        );
        assert_eq!(
            back.smb_mounts[0].username, "user",
            "legacy SMB username survives save (rollback credentials)"
        );
        assert_eq!(
            back.smb_mounts[0].password, "pass",
            "legacy SMB password survives save (rollback credentials)"
        );
        assert_eq!(back.ssh_mounts[0].local_folder_id, "lf-ssh");
        assert_eq!(back.ssh_mounts[0].mountpoint, "/mnt/vela-ssh");
    }

    #[test]
    fn the_player_setting_loads_only_its_two_values_and_is_written_only_when_set() {
        let root =
            std::env::temp_dir().join(format!("vela-config-player-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        let path = root.join("config.json");

        // A config from before the field existed: loads, and the player is unset.
        fs::write(&path, br#"{"continue_playing":"on"}"#).unwrap();
        assert_eq!(load_unmigrated_at(&path).unwrap().player, None);

        for value in ["external", "embedded"] {
            fs::write(&path, format!(r#"{{"player":"{value}"}}"#)).unwrap();
            assert_eq!(
                load_unmigrated_at(&path).unwrap().player.as_deref(),
                Some(value)
            );
        }

        // Unset: the key is not written at all, so a release that predates the
        // field (which rejects keys it does not know) still reads the file.
        let unset = serde_json::to_string(&AppConfig::default()).unwrap();
        assert!(
            !unset.contains("\"player\""),
            "an unset player must not be serialized: {unset}"
        );
        // Set: it round-trips.
        let chosen = AppConfig {
            player: Some("embedded".to_string()),
            ..Default::default()
        };
        let written = serde_json::to_string(&chosen).unwrap();
        assert!(written.contains(r#""player":"embedded""#), "{written}");
        assert_eq!(
            serde_json::from_str::<AppConfig>(&written)
                .unwrap()
                .player
                .as_deref(),
            Some("embedded")
        );
        chosen.validate().expect("a known player value is valid");

        let _ = fs::remove_dir_all(&root);
    }

    // Slice 3 of `.agents/plans/mpv-window-geometry.md`. A config from before
    // the two fields loads exactly as before, and a config that never set them
    // does not gain them: the previous release (v1.0.60) rejects keys it does
    // not know (`deny_unknown_fields`).
    #[test]
    fn the_player_window_fields_are_optional_and_written_only_when_set() {
        let (path, _, _, _, root) = temp_paths("mpv-window");

        fs::write(&path, br#"{"continue_playing":"on"}"#).unwrap();
        let old = load_unmigrated_at(&path).unwrap();
        assert_eq!(old.mpv_window_mode, None);
        assert_eq!(old.mpv_window, None);
        assert_eq!(
            MpvWindowMode::from_config(old.mpv_window_mode.as_deref()),
            Ok(MpvWindowMode::Remember),
            "a missing mode is remember (D2(a))"
        );
        let unset = serde_json::to_string(&old).unwrap();
        assert!(
            !unset.contains("mpv_window"),
            "unset window fields must not be serialized: {unset}"
        );

        let stored = MpvWindow {
            fullscreen: Some(true),
            maximized: Some(false),
            screen: Some("DP-3".to_string()),
            width: Some(1280),
            height: Some(720),
        };
        for mode in ["remember", "fullscreen", "default"] {
            let cfg = AppConfig {
                mpv_window_mode: Some(mode.to_string()),
                mpv_window: Some(stored.clone()),
                ..Default::default()
            };
            crate::storage::save_json(&path, &cfg).unwrap();
            let back = load_unmigrated_at(&path).unwrap();
            assert_eq!(back.mpv_window_mode.as_deref(), Some(mode));
            assert_eq!(back.mpv_window.as_ref(), Some(&stored));
        }
        // Parts never observed are not written either.
        let partial = serde_json::to_string(&MpvWindow {
            fullscreen: Some(false),
            ..Default::default()
        })
        .unwrap();
        assert_eq!(partial, r#"{"fullscreen":false}"#);
        fs::remove_dir_all(root).unwrap();
    }

    fn ended(
        fullscreen: Option<bool>,
        maximized: Option<bool>,
        size: Option<(u32, u32)>,
    ) -> crate::playback::PlaybackWindowState {
        crate::playback::PlaybackWindowState {
            fullscreen,
            maximized,
            windowed_size: size,
        }
    }

    #[test]
    fn an_external_play_that_ends_in_remember_mode_stores_its_window() {
        use crate::player::PlayerKind;
        let mut cfg = AppConfig::default();
        assert!(remember_mpv_window(
            &mut cfg,
            PlayerKind::External,
            ended(Some(true), Some(false), Some((1280, 720))),
            Some("DP-3"),
        ));
        assert_eq!(
            cfg.mpv_window,
            Some(MpvWindow {
                fullscreen: Some(true),
                maximized: Some(false),
                screen: Some("DP-3".to_string()),
                width: Some(1280),
                height: Some(720),
            })
        );
        cfg.validate().expect("what is stored is valid");

        // The same window again changes nothing.
        let before = cfg.mpv_window.clone();
        assert!(!remember_mpv_window(
            &mut cfg,
            PlayerKind::External,
            ended(Some(true), Some(false), Some((1280, 720))),
            Some("DP-3"),
        ));
        assert_eq!(cfg.mpv_window, before);

        // A play that observed only part keeps the rest; one that observed
        // nothing keeps all of it.
        assert!(remember_mpv_window(
            &mut cfg,
            PlayerKind::External,
            ended(Some(false), None, None),
            None,
        ));
        assert_eq!(cfg.mpv_window.as_ref().unwrap().fullscreen, Some(false));
        assert_eq!(cfg.mpv_window.as_ref().unwrap().maximized, Some(false));
        assert_eq!(cfg.mpv_window.as_ref().unwrap().width, Some(1280));
        assert_eq!(
            cfg.mpv_window.as_ref().unwrap().screen.as_deref(),
            Some("DP-3")
        );
        assert!(!remember_mpv_window(
            &mut cfg,
            PlayerKind::External,
            ended(None, None, None),
            None,
        ));

        // A play that observed nothing, with nothing stored, stores nothing.
        let mut empty = AppConfig::default();
        assert!(!remember_mpv_window(
            &mut empty,
            PlayerKind::External,
            ended(None, None, None),
            None,
        ));
        assert_eq!(empty.mpv_window, None);

        // A screen name the settings would reject is not taken.
        let mut odd = AppConfig::default();
        remember_mpv_window(
            &mut odd,
            PlayerKind::External,
            ended(Some(false), None, None),
            Some("bad\nname"),
        );
        assert_eq!(odd.mpv_window.as_ref().unwrap().screen, None);
        odd.validate().expect("still valid");

        // Nor is a window the settings would reject, so the recents stamp
        // written with it cannot fail over it. (The window-size rule never
        // reports such a size; this is the last line if it ever did.)
        let mut cfg = AppConfig::default();
        assert!(!remember_mpv_window(
            &mut cfg,
            PlayerKind::External,
            ended(Some(false), None, Some((10, 10))),
            None,
        ));
        assert_eq!(cfg.mpv_window, None);
        cfg.validate().expect("still valid");
    }

    // A screen name is kept up to 256 bytes; a longer one is not a screen
    // name, whether it is stored or observed.
    #[test]
    fn a_screen_name_longer_than_256_bytes_is_neither_stored_nor_loaded() {
        let (path, _, _, _, root) = temp_paths("mpv-window-screen-length");
        let longest = "S".repeat(256);
        let too_long = "S".repeat(257);
        fs::write(
            &path,
            format!(r#"{{"mpv_window":{{"screen":"{longest}"}}}}"#),
        )
        .unwrap();
        assert_eq!(
            load_unmigrated_at(&path)
                .unwrap()
                .mpv_window
                .and_then(|window| window.screen),
            Some(longest.clone())
        );
        fs::write(
            &path,
            format!(r#"{{"mpv_window":{{"screen":"{too_long}"}}}}"#),
        )
        .unwrap();
        assert_eq!(
            load_unmigrated_at(&path).err().unwrap().kind(),
            io::ErrorKind::InvalidData
        );
        fs::remove_dir_all(root).unwrap();

        let mut cfg = AppConfig::default();
        remember_mpv_window(
            &mut cfg,
            crate::player::PlayerKind::External,
            ended(Some(false), None, None),
            Some(&too_long),
        );
        assert_eq!(cfg.mpv_window.as_ref().unwrap().screen, None);
        remember_mpv_window(
            &mut cfg,
            crate::player::PlayerKind::External,
            ended(Some(false), None, None),
            Some(&longest),
        );
        assert_eq!(cfg.mpv_window.unwrap().screen, Some(longest));
    }

    #[test]
    fn only_remember_mode_and_only_the_external_player_store_the_window() {
        use crate::player::PlayerKind;
        let window = ended(Some(true), Some(false), Some((1280, 720)));
        for mode in ["fullscreen", "default"] {
            let mut cfg = AppConfig {
                mpv_window_mode: Some(mode.to_string()),
                ..Default::default()
            };
            assert!(!remember_mpv_window(
                &mut cfg,
                PlayerKind::External,
                window,
                Some("DP-3")
            ));
            assert_eq!(cfg.mpv_window, None, "{mode} mode must not store a window");
        }
        // An embedded play has no mpv window: what libmpv reports about
        // fullscreen says nothing about how the next run's window opens.
        let mut cfg = AppConfig::default();
        assert!(!remember_mpv_window(
            &mut cfg,
            PlayerKind::Embedded,
            window,
            Some("DP-3")
        ));
        assert_eq!(cfg.mpv_window, None);
    }

    // Missing is remember; anything that is not one of the three is an
    // error, never a guess.
    #[test]
    fn the_player_window_mode_resolves_only_known_values() {
        for (stored, mode) in [
            (None, MpvWindowMode::Remember),
            (Some("remember"), MpvWindowMode::Remember),
            (Some("fullscreen"), MpvWindowMode::Fullscreen),
            (Some("default"), MpvWindowMode::Default),
        ] {
            assert_eq!(MpvWindowMode::from_config(stored), Ok(mode));
        }
        for unknown in ["future", "Remember", ""] {
            assert!(MpvWindowMode::from_config(Some(unknown)).is_err(), "{unknown:?}");
        }
    }

    #[test]
    fn the_first_window_of_a_run_follows_the_mode() {
        let stored = MpvWindow {
            fullscreen: Some(false),
            maximized: Some(true),
            screen: Some("HDMI-1".to_string()),
            width: Some(800),
            height: Some(450),
        };
        let with = |mode: Option<&str>, window: Option<MpvWindow>| AppConfig {
            mpv_window_mode: mode.map(str::to_string),
            mpv_window: window,
            ..Default::default()
        };
        assert_eq!(
            first_window_of_run(&with(None, Some(stored.clone()))).unwrap(),
            (
                ended(Some(false), Some(true), Some((800, 450))),
                Some("HDMI-1".to_string())
            )
        );
        assert_eq!(
            first_window_of_run(&with(None, None)).unwrap(),
            OpeningWindow::default()
        );
        // Full screen and nothing else, whatever is stored.
        assert_eq!(
            first_window_of_run(&with(Some("fullscreen"), Some(stored.clone()))).unwrap(),
            (ended(Some(true), None, None), None)
        );
        assert_eq!(
            first_window_of_run(&with(Some("default"), Some(stored))).unwrap(),
            OpeningWindow::default()
        );
    }

    #[test]
    fn strict_settings_reject_unknown_keys_values_and_incomplete_legacy_credentials() {
        let root =
            std::env::temp_dir().join(format!("vela-config-strict-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        let path = root.join("config.json");

        for invalid in [
            r#"{"future_setting":true}"#,
            r#"{"continue_playing":"future"}"#,
            r#"{"auth_token":"synthetic-only"}"#,
            r#"{"last_server_host":"plex.example"}"#,
            r#"{"watched_threshold_percent":0}"#,
            r#"{"watched_threshold_percent":101}"#,
            r#"{"mpv_autocrop":"future"}"#,
            r#"{"player":"future"}"#,
            r#"{"player":"Embedded"}"#,
            r#"{"player":true}"#,
            r#"{"player":""}"#,
            r#"{"playback_source_policy":"future"}"#,
            r#"{"playback_display_resolution":"16k"}"#,
            r#"{"playback_display_hdr":"maybe"}"#,
            r#"{"skip_intros":"skip"}"#,
            r#"{"skip_credits":"Autoskip"}"#,
            r#"{"skip_commercials":1}"#,
            r#"{"section_sorts":{"":"titleSort:asc"}}"#,
            r#"{"section_sorts":{"jf:1":"future"}}"#,
            r#"{"plex_source_migration":{"from_id":"plex","to_id":"plex-new"}}"#,
            r#"{"connections_split_backup":{"file_name":"../outside.json","byte_length":1,"sha256":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"}}"#,
            r#"{"continue_playing":true}"#,
            r#"{"continue_playing":"on""#,
            r#"{"mpv_window_mode":"future"}"#,
            r#"{"mpv_window_mode":"Remember"}"#,
            r#"{"mpv_window_mode":""}"#,
            r#"{"mpv_window_mode":true}"#,
            r#"{"mpv_window":true}"#,
            r#"{"mpv_window":{"fullscreen":"yes"}}"#,
            r#"{"mpv_window":{"x":10,"y":10}}"#,
            r#"{"mpv_window":{"width":800}}"#,
            r#"{"mpv_window":{"height":450}}"#,
            r#"{"mpv_window":{"width":100,"height":450}}"#,
            r#"{"mpv_window":{"width":800,"height":20000}}"#,
            r#"{"mpv_window":{"width":-800,"height":450}}"#,
            r#"{"mpv_window":{"screen":""}}"#,
            r#"{"mpv_window":{"screen":"   "}}"#,
            r#"{"mpv_window":{"screen":"DP-3\n"}}"#,
        ] {
            fs::write(&path, invalid).unwrap();
            assert_eq!(
                load_unmigrated_at(&path).err().unwrap().kind(),
                io::ErrorKind::InvalidData,
                "{invalid} must fail the whole settings document"
            );
        }

        fs::write(&path, br#"{"continue_playing":"on"}"#).unwrap();
        assert_eq!(
            load_unmigrated_at(&path)
                .unwrap()
                .continue_playing
                .as_deref(),
            Some("on")
        );

        let mut settings = AppConfig {
            hidden_from_continue: vec!["jf:item".to_string(); crate::recents::MAX_HIDDEN + 1],
            ..Default::default()
        };
        assert!(settings.validate().is_err());
        settings.hidden_from_continue.clear();
        settings.recents = vec![
            RecentEntry {
                item: item("jf"),
                session_id: None,
                started_at_ms: 0,
                ended_at_ms: 0,
            };
            crate::recents::MAX_RECENTS + 1
        ];
        assert!(settings.validate().is_err());
        settings.recents.clear();
        settings
            .section_sorts
            .insert("x".repeat(513), "titleSort:asc".to_string());
        assert!(settings.validate().is_err());
        fs::remove_dir_all(root).unwrap();
    }
}
