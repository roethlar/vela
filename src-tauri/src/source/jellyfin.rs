//! Jellyfin / Emby backend. The two share almost all of their HTTP API (Jellyfin
//! forked from Emby), so one client serves both; the places they genuinely differ
//! — chiefly the auth header scheme — are isolated behind [`Flavor`] so they can
//! keep diverging without reshaping the rest of the code.

use async_trait::async_trait;
use serde::de::DeserializeOwned;
use serde::Deserialize;

use super::{
    namespace_key, normalize_markers, EpisodeContext, HubDto, ItemDto, MarkerKind, MediaMarker,
    MediaSource, PlaybackVersion, PlaylistDto, SectionDto, StreamResolution,
};
use crate::playback::{JellyfinTrack, ProgressTarget};

/// From Cargo.toml, so the device-identity header can't drift from the package
/// version (matches the Plex/UI version).
const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Error string the frontend recognizes: the stored token/key was rejected and
/// we have no credential to retry with, so P2f must prompt for reconnection.
pub const RECONNECT_REQUIRED: &str = "RECONNECT_REQUIRED";

/// Library scans are admin-gated on Jellyfin/Emby even when browsing works —
/// map FORBIDDEN to something a non-admin user can act on.
const SCAN_FORBIDDEN: &str = "the server refused the scan (administrator permission required)";

/// Which server dialect we're talking to. Only the auth headers differ today,
/// but keeping this explicit lets the two diverge further later.
#[derive(Clone, Copy, PartialEq)]
pub enum Flavor {
    Jellyfin,
    Emby,
}

impl Flavor {
    pub fn from_kind(kind: &str) -> Option<Flavor> {
        match kind {
            "jellyfin" => Some(Flavor::Jellyfin),
            "emby" => Some(Flavor::Emby),
            _ => None,
        }
    }
    pub fn kind(&self) -> &'static str {
        match self {
            Flavor::Jellyfin => "jellyfin",
            Flavor::Emby => "emby",
        }
    }
}

/// The `MediaBrowser ...` identity string both dialects expect.
fn device_value(device_id: &str) -> String {
    format!(
        "MediaBrowser Client=\"Vela\", Device=\"{}\", DeviceId=\"{}\", Version=\"{}\"",
        crate::platform_name(),
        device_id,
        VERSION
    )
}

/// Auth headers for an authenticated request — the one real Jellyfin/Emby split.
fn auth_headers(flavor: Flavor, device_id: &str, token: &str) -> Vec<(String, String)> {
    match flavor {
        Flavor::Jellyfin => {
            let v = format!("{}, Token=\"{}\"", device_value(device_id), token);
            vec![("Authorization".to_string(), v)]
        }
        Flavor::Emby => vec![
            ("X-Emby-Authorization".to_string(), device_value(device_id)),
            ("X-Emby-Token".to_string(), token.to_string()),
        ],
    }
}

/// How a flavor lists its concrete server libraries (the valid scan targets).
#[derive(Clone, Copy, PartialEq, Debug)]
enum VfEnvelope {
    /// Bare JSON array (Jellyfin's `GET /Library/VirtualFolders`).
    Bare,
    /// `{"Items": […]}` wrapper (Emby's `GET /Library/VirtualFolders/Query`).
    Items,
}

/// Each flavor's documented virtual-folders route. Emby's 4.9 REST reference
/// only documents the `/Query` form (an `Items` envelope); Jellyfin only the
/// bare route. Sharing either shape would break the other server, so the
/// mapping is explicit and unit-tested per flavor.
fn vf_route(flavor: Flavor) -> (&'static str, VfEnvelope) {
    match flavor {
        Flavor::Jellyfin => ("/Library/VirtualFolders", VfEnvelope::Bare),
        Flavor::Emby => ("/Library/VirtualFolders/Query", VfEnvelope::Items),
    }
}

/// Query for `POST /Items/{id}/Refresh` matching the dashboard's plain "scan
/// for new files": recursive, no forced metadata/artwork replacement. Servers
/// ignore params they don't know (Emby predates `RegenerateTrickplay`).
fn scan_query() -> [(&'static str, &'static str); 6] {
    [
        ("Recursive", "true"),
        ("MetadataRefreshMode", "Default"),
        ("ImageRefreshMode", "Default"),
        ("ReplaceAllMetadata", "false"),
        ("ReplaceAllImages", "false"),
        ("RegenerateTrickplay", "false"),
    ]
}

fn playlist_list_query() -> [(&'static str, &'static str); 6] {
    [
        ("Recursive", "true"),
        ("IncludeItemTypes", "Playlist"),
        ("MediaTypes", "Video"),
        ("SortBy", "SortName"),
        ("SortOrder", "Ascending"),
        ("Fields", "ChildCount"),
    ]
}

// ---- HTTP client ---------------------------------------------------------

pub struct JellyfinClient {
    flavor: Flavor,
    base_url: String,
    device_id: String,
    token: String,
    user_id: String,
    http: reqwest::Client,
}

/// Result of a successful credential exchange.
pub struct Authed {
    pub token: String,
    pub user_id: String,
    pub device_id: String,
    pub server_name: String,
}

impl JellyfinClient {
    pub fn new(
        flavor: Flavor,
        base_url: &str,
        device_id: &str,
        token: &str,
        user_id: &str,
    ) -> Self {
        Self {
            flavor,
            base_url: base_url.trim_end_matches('/').to_string(),
            device_id: device_id.to_string(),
            token: token.to_string(),
            user_id: user_id.to_string(),
            http: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(15))
                .build()
                .unwrap_or_default(),
        }
    }

    fn auth_headers(&self) -> Vec<(String, String)> {
        auth_headers(self.flavor, &self.device_id, &self.token)
    }

    /// mpv's header list cannot safely encode the quoted/comma-separated
    /// MediaBrowser identity header. Both Jellyfin and Emby accept the bearer
    /// token alone as X-Emby-Token, which is safe in the owner-only include.
    fn stream_auth_headers(&self) -> Vec<(String, String)> {
        vec![("X-Emby-Token".to_string(), self.token.clone())]
    }

    async fn get_json<T: DeserializeOwned>(
        &self,
        path: &str,
        query: &[(&str, String)],
    ) -> Result<T, String> {
        let url = format!("{}{}", self.base_url, path);
        self.get_json_url(&url, query).await
    }

    async fn get_json_url<T: DeserializeOwned>(
        &self,
        url: &str,
        query: &[(&str, String)],
    ) -> Result<T, String> {
        // Per-request timeout as a safety net: if the shared client ever fell back
        // to a default (no-timeout) build, a stuck request still can't hang here.
        let mut rb = self
            .http
            .get(url)
            .timeout(std::time::Duration::from_secs(15))
            .query(query);
        for (k, v) in self.auth_headers() {
            rb = rb.header(k, v);
        }
        let resp = rb.send().await.map_err(|e| e.to_string())?;
        // A rejected token can't be refreshed without stored creds (we don't keep
        // the password) — surface a reconnect signal for the UI to handle.
        if resp.status() == reqwest::StatusCode::UNAUTHORIZED {
            return Err(RECONNECT_REQUIRED.to_string());
        }
        let resp = resp.error_for_status().map_err(|e| e.to_string())?;
        resp.json::<T>().await.map_err(|e| e.to_string())
    }

    /// Best-effort skip ranges for one item.
    ///
    /// Jellyfin owns the MediaSegments route; Emby's current published OpenAPI
    /// has no equivalent, so an Emby server is never asked — shared ancestry is
    /// not evidence of a shared contract. Every failure (unsupported route on an
    /// older Jellyfin, transport error, unreadable body) yields no markers and
    /// never fails the play.
    /// Its own bound, far below the general per-request timeout: markers are
    /// optional, and a server that cannot answer this quickly must not be able
    /// to hold up a launch the user is waiting on.
    const MARKER_LOOKUP_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(4);

    async fn media_segments(&self, item_id: &str) -> Vec<MediaMarker> {
        if self.flavor != Flavor::Jellyfin {
            return Vec::new();
        }
        // Repeated `includeSegmentTypes` pairs are the route's own filter form.
        let query = [
            ("includeSegmentTypes", "Intro".to_string()),
            ("includeSegmentTypes", "Outro".to_string()),
            ("includeSegmentTypes", "Commercial".to_string()),
        ];
        let path = format!("/MediaSegments/{item_id}");
        let lookup = self.get_json::<MediaSegmentsEnvelope>(&path, &query);
        match tokio::time::timeout(Self::MARKER_LOOKUP_TIMEOUT, lookup).await {
            Ok(Ok(envelope)) => normalize_markers(
                envelope
                    .items
                    .iter()
                    .filter_map(media_segment_to_marker)
                    .collect(),
            ),
            Ok(Err(error)) => {
                // The client's own message: server-address detail at worst,
                // never the token, which travels as a header.
                eprintln!("jellyfin: marker lookup failed, playing without markers: {error}");
                Vec::new()
            }
            Err(_) => {
                eprintln!("jellyfin: marker lookup timed out, playing without markers");
                Vec::new()
            }
        }
    }

    /// HLS transcode URL for one item at one tier.
    ///
    /// Unlike Plex there is no client-invented session id: Jellyfin keys an
    /// encoding by `deviceId` + `playSessionId`, and those two are what later
    /// stop it. `mediaSourceId` is REQUIRED on this route — omitting it makes
    /// the server pick a source, which for a multi-version title is not
    /// necessarily the copy the user chose.
    ///
    /// `deviceProfileId` is deliberately absent: Jellyfin marks it obsolete and
    /// never reads it, so capabilities travel as the explicit parameters below.
    fn transcode_url(
        &self,
        item_id: &str,
        media_source_id: &str,
        play_session_id: Option<&str>,
        tier: crate::source::QualityTier,
        start_ticks: i64,
    ) -> String {
        let bitrate = (tier.bitrate_kbps as u64 * 1000).to_string();
        let width = tier.width.to_string();
        let height = tier.height.to_string();
        let start = start_ticks.max(0).to_string();
        let mut pairs: Vec<(&str, &str)> = vec![
            ("mediaSourceId", media_source_id),
            ("deviceId", self.device_id.as_str()),
            ("videoBitRate", bitrate.as_str()),
            ("maxWidth", width.as_str()),
            ("maxHeight", height.as_str()),
            ("startTimeTicks", start.as_str()),
            // Let the server copy a stream it does not need to touch: a file
            // whose audio already fits should not be re-encoded just because
            // the video is.
            ("enableAutoStreamCopy", "true"),
            ("api_key", self.token.as_str()),
        ];
        if let Some(ps) = play_session_id {
            pairs.push(("PlaySessionId", ps));
        }
        self.build_url(&["Videos", item_id, "master.m3u8"], &pairs)
    }

    /// Stop one active encoding. Best-effort by return type but MANDATORY to
    /// call: an abandoned Jellyfin encoding keeps a transcoder busy on the
    /// user's server.
    async fn stop_transcode(&self, play_session_id: &str) {
        let url = self.build_url(
            &["Videos", "ActiveEncodings"],
            &[
                ("deviceId", self.device_id.as_str()),
                ("PlaySessionId", play_session_id),
            ],
        );
        let mut request = self.http.delete(&url);
        for (name, value) in self.auth_headers() {
            request = request.header(name, value);
        }
        if let Err(error) = request.send().await {
            // Never print the URL: it carries the api_key.
            eprintln!("jellyfin: could not stop an active encoding: {error}");
        }
    }

    /// Mark an item played (POST) or unplayed (DELETE) for the current user.
    async fn set_played(&self, item_id: &str, played: bool) -> Result<(), String> {
        let url = self.build_url(&["Users", &self.user_id, "PlayedItems", item_id], &[]);
        let base = if played {
            self.http.post(&url)
        } else {
            self.http.delete(&url)
        };
        let mut rb = base.timeout(std::time::Duration::from_secs(15));
        for (k, v) in self.auth_headers() {
            rb = rb.header(k, v);
        }
        let resp = rb.send().await.map_err(|e| e.to_string())?;
        if resp.status() == reqwest::StatusCode::UNAUTHORIZED {
            return Err(RECONNECT_REQUIRED.to_string());
        }
        resp.error_for_status().map_err(|e| e.to_string())?;
        Ok(())
    }

    /// Direct-play stream URL (original file, no transcode) for true HDR
    /// passthrough, using the negotiated media source + play session.
    /// Authentication travels through mpv's owner-only header include, never
    /// through argv, the window title, or the stream URL.
    fn stream_url(
        &self,
        item_id: &str,
        media_source_id: &str,
        play_session_id: Option<&str>,
    ) -> String {
        let mut pairs = vec![
            ("static", "true"),
            ("mediaSourceId", media_source_id),
            ("deviceId", self.device_id.as_str()),
        ];
        if let Some(ps) = play_session_id {
            pairs.push(("PlaySessionId", ps));
        }
        self.build_url(&["Videos", item_id, "stream"], &pairs)
    }

    /// Negotiate playback: returns the real `MediaSourceId` and a `PlaySessionId`
    /// to thread through the stream URL and check-in posts.
    async fn playback_info(&self, item_id: &str) -> Result<(String, Option<String>), String> {
        let info = self.playback_info_response(item_id).await?;
        let media_source_id = select_media_source(&info.media_sources)
            .and_then(|m| (!m.id.is_empty()).then(|| m.id.clone()))
            .unwrap_or_else(|| item_id.to_string());
        Ok((media_source_id, info.play_session_id))
    }

    async fn playback_info_response(&self, item_id: &str) -> Result<PlaybackInfoResp, String> {
        self
            .get_json(
                &format!("/Items/{}/PlaybackInfo", item_id),
                &[("UserId", self.user_id.clone())],
            )
            .await
    }

    fn poster_url(&self, item_id: &str, tag: &str) -> String {
        self.build_url(
            &["Items", item_id, "Images", "Primary"],
            &[
                ("fillHeight", "450"),
                ("fillWidth", "300"),
                ("tag", tag),
                ("api_key", self.token.as_str()),
            ],
        )
    }

    /// Landscape backdrop at hero resolution (the hero renders at window
    /// width). Same token-in-URL exposure as `poster_url`.
    fn backdrop_url(&self, item_id: &str, tag: &str) -> String {
        self.build_url(
            &["Items", item_id, "Images", "Backdrop", "0"],
            &[
                ("fillHeight", "1080"),
                ("fillWidth", "1920"),
                ("tag", tag),
                ("api_key", self.token.as_str()),
            ],
        )
    }

    /// An episode's Primary image (its 16:9 scene still) at hero resolution,
    /// for hero rendering when no backdrop exists on the item itself.
    fn hero_still_url(&self, item_id: &str, tag: &str) -> String {
        self.build_url(
            &["Items", item_id, "Images", "Primary"],
            &[
                ("fillHeight", "1080"),
                ("fillWidth", "1920"),
                ("tag", tag),
                ("api_key", self.token.as_str()),
            ],
        )
    }

    /// Concrete server libraries — the valid scan targets. Mirrors
    /// [`Self::get_json`]'s auth/timeout/401 handling but ALSO maps
    /// FORBIDDEN, because scans are admin-gated even when browsing works.
    async fn get_virtual_folders(&self) -> Result<Vec<VirtualFolderInfo>, String> {
        let (path, envelope) = vf_route(self.flavor);
        let url = format!("{}{}", self.base_url, path);
        let mut rb = self
            .http
            .get(&url)
            .timeout(std::time::Duration::from_secs(15));
        for (k, v) in self.auth_headers() {
            rb = rb.header(k, v);
        }
        let resp = rb.send().await.map_err(|e| e.to_string())?;
        if resp.status() == reqwest::StatusCode::UNAUTHORIZED {
            return Err(RECONNECT_REQUIRED.to_string());
        }
        if resp.status() == reqwest::StatusCode::FORBIDDEN {
            return Err(SCAN_FORBIDDEN.to_string());
        }
        let resp = resp.error_for_status().map_err(|e| e.to_string())?;
        match envelope {
            VfEnvelope::Bare => resp
                .json::<Vec<VirtualFolderInfo>>()
                .await
                .map_err(|e| e.to_string()),
            VfEnvelope::Items => Ok(resp
                .json::<VirtualFoldersEnvelope>()
                .await
                .map_err(|e| e.to_string())?
                .items),
        }
    }

    /// Scan-trigger URL for one concrete library. Rejects empty/dot ids up
    /// front — `PathSegmentsMut::extend` silently drops `""`/`"."`/`".."`
    /// segments, which would mutate the endpoint shape instead of failing.
    fn scan_url(&self, item_id: &str) -> Result<String, String> {
        if item_id.is_empty() || item_id == "." || item_id == ".." {
            return Err("invalid library id".to_string());
        }
        Ok(self.build_url(&["Items", item_id, "Refresh"], &scan_query()))
    }

    fn playlist_items_url(
        &self,
        playlist_id: &str,
        start: usize,
        size: usize,
    ) -> Result<String, String> {
        if playlist_id.is_empty() || playlist_id == "." || playlist_id == ".." {
            return Err("invalid playlist id".to_string());
        }
        let start_value = start.to_string();
        let size_value = size.to_string();
        Ok(self.build_url(
            &["Playlists", playlist_id, "Items"],
            &[
                ("UserId", self.user_id.as_str()),
                ("Fields", "Overview,ProviderIds"),
                ("EnableUserData", "true"),
                ("StartIndex", start_value.as_str()),
                ("Limit", size_value.as_str()),
            ],
        ))
    }

    fn user_item_url(&self, item_id: &str) -> Result<String, String> {
        if item_id.is_empty() || item_id == "." || item_id == ".." {
            return Err("invalid item id".to_string());
        }
        Ok(self.build_url(&["Users", &self.user_id, "Items", item_id], &[]))
    }

    /// POST with an empty body (the scan trigger). Same auth/timeout/401/403
    /// mapping as [`Self::get_virtual_folders`].
    async fn post_empty_url(&self, url: &str) -> Result<(), String> {
        let mut rb = self
            .http
            .post(url)
            .timeout(std::time::Duration::from_secs(15));
        for (k, v) in self.auth_headers() {
            rb = rb.header(k, v);
        }
        let resp = rb.send().await.map_err(|e| e.to_string())?;
        if resp.status() == reqwest::StatusCode::UNAUTHORIZED {
            return Err(RECONNECT_REQUIRED.to_string());
        }
        if resp.status() == reqwest::StatusCode::FORBIDDEN {
            return Err(SCAN_FORBIDDEN.to_string());
        }
        resp.error_for_status().map_err(|e| e.to_string())?;
        Ok(())
    }

    /// Build a URL from path segments + query pairs, percent-encoding both so an
    /// id/tag/token containing `&`, `?`, `#`, or a space can't malform the URL
    /// or leak a token into an adjacent parameter.
    fn build_url(&self, segments: &[&str], pairs: &[(&str, &str)]) -> String {
        match url::Url::parse(&self.base_url) {
            Ok(mut u) => {
                if let Ok(mut seg) = u.path_segments_mut() {
                    seg.extend(segments);
                }
                u.query_pairs_mut().extend_pairs(pairs.iter().copied());
                u.to_string()
            }
            // base_url is validated at connect time, so this is unreachable in
            // practice; fall back to a best-effort join that still carries the
            // query pairs (encoded) — dropping them would strip api_key etc.
            Err(_) => {
                let mut s = url::form_urlencoded::Serializer::new(String::new());
                s.extend_pairs(pairs.iter().copied());
                let query = s.finish();
                let base = format!(
                    "{}/{}",
                    self.base_url.trim_end_matches('/'),
                    segments.join("/")
                );
                if query.is_empty() {
                    base
                } else {
                    format!("{base}?{query}")
                }
            }
        }
    }

    /// Exchange username/password for an access token (`Pw` may be empty).
    pub async fn authenticate(
        flavor: Flavor,
        base_url: &str,
        username: &str,
        password: &str,
    ) -> Result<Authed, String> {
        let base_url = base_url.trim_end_matches('/').to_string();
        let device_id = uuid::Uuid::new_v4().to_string();
        let http = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(15))
            .build()
            .map_err(|e| e.to_string())?;

        // The handshake carries the device identity but no token yet.
        let header_name = match flavor {
            Flavor::Jellyfin => "Authorization",
            Flavor::Emby => "X-Emby-Authorization",
        };
        let resp = http
            .post(format!("{}/Users/AuthenticateByName", base_url))
            .header(header_name, device_value(&device_id))
            .json(&serde_json::json!({ "Username": username, "Pw": password }))
            .send()
            .await
            .map_err(|e| e.to_string())?;
        if resp.status() == reqwest::StatusCode::UNAUTHORIZED {
            return Err("Incorrect username or password.".to_string());
        }
        let resp = resp.error_for_status().map_err(|e| e.to_string())?;
        let body: AuthResponse = resp.json().await.map_err(|e| e.to_string())?;

        let server_name = public_server_name(&http, &base_url)
            .await
            .unwrap_or_else(|| flavor.kind().to_string());
        Ok(Authed {
            token: body.access_token,
            user_id: body.user.id,
            device_id,
            server_name,
        })
    }

    /// Validate a pre-issued API key / access token and resolve the user to act
    /// as (the given one, or the server's first user). For headless setups.
    pub async fn from_api_key(
        flavor: Flavor,
        base_url: &str,
        api_key: &str,
        user_id: Option<&str>,
    ) -> Result<Authed, String> {
        let base_url = base_url.trim_end_matches('/').to_string();
        let device_id = uuid::Uuid::new_v4().to_string();
        let probe = JellyfinClient::new(
            flavor,
            &base_url,
            &device_id,
            api_key,
            user_id.unwrap_or(""),
        );

        let resolved_user = match user_id {
            Some(u) if !u.is_empty() => u.to_string(),
            _ => {
                let users: Vec<AuthUser> = probe.get_json("/Users", &[]).await?;
                users
                    .first()
                    .map(|u| u.id.clone())
                    .ok_or("no users on server")?
            }
        };
        // Always make an authenticated request for the selected user, so a bogus
        // token (or wrong user id) can't be persisted as a working source — the
        // unauthenticated public-info call alone would happily "succeed".
        let _: AuthUser = probe
            .get_json(&format!("/Users/{}", resolved_user), &[])
            .await
            .map_err(|e| {
                if e == RECONNECT_REQUIRED {
                    "the API key was rejected by the server".to_string()
                } else {
                    e
                }
            })?;
        let server_name = public_server_name(&probe.http, &base_url)
            .await
            .unwrap_or_else(|| flavor.kind().to_string());
        Ok(Authed {
            token: api_key.to_string(),
            user_id: resolved_user,
            device_id,
            server_name,
        })
    }
}

async fn public_server_name(http: &reqwest::Client, base_url: &str) -> Option<String> {
    #[derive(Deserialize)]
    #[serde(rename_all = "PascalCase")]
    struct PublicInfo {
        server_name: Option<String>,
    }
    http.get(format!("{}/System/Info/Public", base_url))
        .timeout(std::time::Duration::from_secs(15))
        .send()
        .await
        .ok()?
        .json::<PublicInfo>()
        .await
        .ok()?
        .server_name
}

// ---- response DTOs -------------------------------------------------------

/// One concrete server library from the virtual-folders route — the valid
/// scan targets. User views usually share ids with these; grouped/merged
/// views don't appear here (no single scan target exists for them).
#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct VirtualFolderInfo {
    item_id: Option<String>,
}

/// Emby's `/Library/VirtualFolders/Query` envelope.
#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct VirtualFoldersEnvelope {
    #[serde(default)]
    items: Vec<VirtualFolderInfo>,
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct AuthResponse {
    access_token: String,
    user: AuthUser,
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct AuthUser {
    id: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct ItemsResponse {
    #[serde(default)]
    items: Vec<BaseItem>,
    total_record_count: Option<usize>,
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct PlaybackInfoResp {
    #[serde(default)]
    media_sources: Vec<MediaSourceInfo>,
    play_session_id: Option<String>,
}

fn exact_playback_identity(
    info: PlaybackInfoResp,
    item_key: &str,
    version_id: &str,
) -> Result<(String, Option<String>), String> {
    let media_source_id = if info.media_sources.is_empty() && version_id == item_key {
        item_key.to_string()
    } else {
        info.media_sources
            .iter()
            .find(|source| source.id == version_id)
            .map(|source| source.id.clone())
            .ok_or_else(|| {
                "the selected media-server version is no longer available".to_string()
            })?
    };
    Ok((media_source_id, info.play_session_id))
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct MediaSourceInfo {
    #[serde(default)]
    id: String,
    supports_direct_play: Option<bool>,
    supports_direct_stream: Option<bool>,
    /// Whether this server will transcode this source at all.
    supports_transcoding: Option<bool>,
    /// Server-built transcode URL, when it offers one. Vela builds its own from
    /// the chosen tier rather than using this, but its presence is corroborating
    /// evidence that transcoding is available.
    transcoding_url: Option<String>,
    /// Diagnostic only, and nothing reads it yet — it exists so the tolerant
    /// parse below is exercised now rather than discovered later. Deliberately
    /// untyped: Jellyfin has shipped this as both an array of strings and a
    /// comma-joined flags string, and a wrong concrete type here would fail the
    /// whole PlaybackInfo parse and break playback for a field nothing depends
    /// on.
#[allow(dead_code)] // diagnostic; parsed so the tolerant shape is exercised
    transcode_reasons: Option<serde_json::Value>,
    bitrate: Option<u64>,
    size: Option<u64>,
    width: Option<u32>,
    height: Option<u32>,
    #[serde(default)]
    media_streams: Vec<MediaStreamInfo>,
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct MediaStreamInfo {
    #[serde(rename = "Type")]
    stream_type: Option<String>,
    video_range: Option<String>,
    video_range_type: Option<String>,
    bit_rate: Option<u64>,
    width: Option<u32>,
    height: Option<u32>,
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "PascalCase")]
struct BaseItem {
    id: String,
    name: Option<String>,
    production_year: Option<u32>,
    overview: Option<String>,
    run_time_ticks: Option<i64>,
    #[serde(rename = "Type")]
    item_type: Option<String>,
    user_data: Option<UserData>,
    index_number: Option<u32>,
    parent_index_number: Option<u32>,
    series_name: Option<String>,
    season_name: Option<String>,
    series_id: Option<String>,
    season_id: Option<String>,
    series_primary_image_tag: Option<String>,
    backdrop_image_tags: Option<Vec<String>>,
    image_tags: Option<ImageTags>,
    collection_type: Option<String>,
    media_type: Option<String>,
    child_count: Option<usize>,
    provider_ids: Option<std::collections::HashMap<String, String>>,
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct UserData {
    playback_position_ticks: Option<i64>,
    played: Option<bool>,
}

#[derive(Deserialize)]
struct ImageTags {
    #[serde(rename = "Primary")]
    primary: Option<String>,
}

/// 100ns ticks → milliseconds.
/// The MediaSegments response: the normal Jellyfin query envelope. It gets its
/// own DTO rather than widening `BaseItem`, because segments are a different
/// resource that happens to share an envelope shape.
#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct MediaSegmentsEnvelope {
    #[serde(default)]
    items: Vec<MediaSegmentDto>,
}

/// One segment. This is a range API: `StartTicks`/`EndTicks` are authoritative,
/// and chapter data is never used to infer a range.
#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct MediaSegmentDto {
    #[serde(rename = "Type", default)]
    segment_type: String,
    #[serde(default)]
    start_ticks: i64,
    #[serde(default)]
    end_ticks: i64,
}

/// Map one Jellyfin segment onto a Vela range. Jellyfin's `Outro` is what Vela
/// calls Credits; segment types Vela does not model are ignored.
fn media_segment_to_marker(segment: &MediaSegmentDto) -> Option<MediaMarker> {
    let kind = match segment.segment_type.trim() {
        "Intro" => MarkerKind::Intro,
        "Outro" => MarkerKind::Credits,
        "Commercial" => MarkerKind::Commercial,
        _ => return None,
    };
    Some(MediaMarker {
        kind,
        start_ms: ticks_to_ms(segment.start_ticks),
        end_ms: ticks_to_ms(segment.end_ticks),
    })
}

fn ticks_to_ms(ticks: i64) -> u64 {
    (ticks / 10_000).max(0) as u64
}

fn select_media_source(sources: &[MediaSourceInfo]) -> Option<&MediaSourceInfo> {
    sources
        .iter()
        .filter(|source| source.direct_rank() > 0)
        .max_by_key(|source| media_source_quality_key(source))
        .or_else(|| {
            sources
                .iter()
                .max_by_key(|source| media_source_quality_key(source))
        })
}

fn media_source_quality_key(source: &MediaSourceInfo) -> (u8, u32, u32, bool, u64, u64) {
    (
        source.direct_rank(),
        source.video_height(),
        source.video_width(),
        source.is_hdr(),
        source.bitrate(),
        source.size.unwrap_or(0),
    )
}

impl MediaSourceInfo {
    fn direct_rank(&self) -> u8 {
        if self.supports_direct_play.unwrap_or(false) {
            2
        } else if self.supports_direct_stream.unwrap_or(false) {
            1
        } else {
            0
        }
    }

    /// What Vela may offer for this exact copy. Direct play covers both
    /// direct-play and direct-stream: either delivers the original video
    /// untouched, which is what "Original" means to the user.
    ///
    /// Transcoding availability is taken from `SupportsTranscoding`, falling
    /// back to the presence of a server-built `TranscodingUrl`. Absent both, we
    /// assume NO — never offer a conversion the server has not said it can do.
    fn playback_options(&self) -> crate::source::PlaybackOptions {
        let can_transcode = self
            .supports_transcoding
            .unwrap_or_else(|| self.transcoding_url.is_some());
        crate::source::PlaybackOptions::new(
            self.direct_rank() > 0,
            can_transcode,
            self.video_width(),
            self.video_height(),
            // Jellyfin reports bits per second; the ladder is in kbps.
            (self.bitrate() / 1000) as u32,
        )
    }

    fn candidate_direct_rank(&self) -> u8 {
        match self.direct_rank() {
            2 => 0,
            1 => 1,
            _ => 2,
        }
    }

    fn is_hdr(&self) -> bool {
        self.media_streams
            .iter()
            .filter(|stream| stream.is_video())
            .any(|stream| stream.is_hdr())
    }

    fn bitrate(&self) -> u64 {
        self.bitrate
            .or_else(|| {
                self.media_streams
                    .iter()
                    .filter(|stream| stream.is_video())
                    .filter_map(|stream| stream.bit_rate)
                    .max()
            })
            .unwrap_or(0)
    }

    fn video_width(&self) -> u32 {
        self.width
            .or_else(|| {
                self.media_streams
                    .iter()
                    .filter(|stream| stream.is_video())
                    .filter_map(|stream| stream.width)
                    .max()
            })
            .unwrap_or(0)
    }

    fn video_height(&self) -> u32 {
        self.height
            .or_else(|| {
                self.media_streams
                    .iter()
                    .filter(|stream| stream.is_video())
                    .filter_map(|stream| stream.height)
                    .max()
            })
            .unwrap_or(0)
    }
}

impl MediaStreamInfo {
    fn is_video(&self) -> bool {
        self.stream_type.as_deref() == Some("Video")
    }

    fn is_hdr(&self) -> bool {
        self.video_range.as_deref().is_some_and(is_hdr_value)
            || self.video_range_type.as_deref().is_some_and(is_hdr_value)
    }
}

fn is_hdr_value(value: &str) -> bool {
    let value = value.to_ascii_lowercase();
    value.contains("hdr")
        || value.contains("hlg")
        || value.contains("dovi")
        || value.contains("dolby")
}

fn map_type(t: Option<&str>) -> Option<String> {
    Some(
        match t.unwrap_or("") {
            "Movie" => "movie",
            "Series" => "show",
            "Season" => "season",
            "Episode" => "episode",
            other => return (!other.is_empty()).then(|| other.to_lowercase()),
        }
        .to_string(),
    )
}

fn is_video_playlist(item: &BaseItem) -> bool {
    item.media_type
        .as_deref()
        .is_none_or(|kind| kind.eq_ignore_ascii_case("video"))
}

/// Translate the UI's Plex-style sort token to Jellyfin's SortBy/SortOrder.
fn map_sort(sort: Option<&str>) -> (String, String) {
    let s = sort.unwrap_or("titleSort:asc");
    let (field, dir) = s.split_once(':').unwrap_or((s, "asc"));
    let by = match field {
        "year" => "ProductionYear,PremiereDate",
        "addedAt" => "DateCreated",
        // Leaf-added recency for series ("Last episode added"): the server-
        // side sort by newest content inside the container. Emby is assumed
        // to accept the same name (shared client code); a server that
        // ignores it returns its default order — degraded, not broken.
        "episodeAddedAt" => "DateLastContentAdded",
        "originallyAvailableAt" => "PremiereDate",
        "rating" => "CommunityRating",
        "lastViewedAt" => "DatePlayed",
        _ => "SortName",
    };
    let order = if dir.eq_ignore_ascii_case("desc") {
        "Descending"
    } else {
        "Ascending"
    };
    (by.to_string(), order.to_string())
}

// ---- MediaSource impl ----------------------------------------------------

/// Rebuild a live source from its persisted config (used at startup and right
/// after a successful connect). Returns `None` if the config is incomplete or
/// the kind isn't a Jellyfin/Emby flavor.
pub fn build_source(
    cfg: &crate::config::SourceConfig,
) -> Result<std::sync::Arc<dyn MediaSource>, String> {
    crate::connections::validate_source(cfg)?;
    let flavor =
        Flavor::from_kind(&cfg.kind).ok_or_else(|| "unknown Jellyfin/Emby kind".to_string())?;
    // Require everything requests actually need, so a corrupt/partial config
    // can't restore as a broken live source. access_token (user login) or
    // api_key (headless) — either is the bearer token.
    let nonempty = |o: &Option<String>| o.clone().filter(|s| !s.is_empty());
    let token = nonempty(&cfg.access_token)
        .or_else(|| nonempty(&cfg.api_key))
        .ok_or_else(|| "missing Jellyfin/Emby token".to_string())?;
    let user_id =
        nonempty(&cfg.user_id).ok_or_else(|| "missing Jellyfin/Emby user".to_string())?;
    let device_id =
        nonempty(&cfg.device_id).ok_or_else(|| "missing Jellyfin/Emby device".to_string())?;
    if cfg.base_url.is_empty() {
        return Err("missing Jellyfin/Emby endpoint".to_string());
    }
    let client = JellyfinClient::new(flavor, &cfg.base_url, &device_id, &token, &user_id);
    Ok(std::sync::Arc::new(JellyfinSource::new(
        cfg.id.clone(),
        cfg.name.clone(),
        client,
    )))
}

pub struct JellyfinSource {
    id: String,
    name: String,
    client: JellyfinClient,
}

impl JellyfinSource {
    /// What this server can do with one exact copy. A PlaybackInfo lookup that
    /// fails leaves the copy direct-play-only, so a transient error can never
    /// silently start converting.
    async fn playback_options_for(
        &self,
        item_key: &str,
        media_source_id: &str,
    ) -> crate::source::PlaybackOptions {
        match self.client.playback_info_response(item_key).await {
            Ok(info) => info
                .media_sources
                .iter()
                .find(|source| source.id == media_source_id)
                .or_else(|| info.media_sources.first())
                .map(|source| source.playback_options())
                .unwrap_or_else(|| crate::source::PlaybackOptions::new(true, false, 0, 0, 0)),
            Err(_) => crate::source::PlaybackOptions::new(true, false, 0, 0, 0),
        }
    }

    /// Choose how this play is delivered, and build the matching URL.
    ///
    /// The Original path is exactly the previous behaviour: the same
    /// `static=true` direct-stream URL, no capability request, no extra round
    /// trip. Only an actual tier request diverges, and a copy the server will
    /// not convert degrades back to the original rather than failing the play.
    async fn deliver(
        &self,
        item_key: &str,
        media_source_id: &str,
        play_session_id: Option<&str>,
        quality: &str,
        start_ticks: i64,
    ) -> (String, crate::source::Delivery, Option<String>) {
        let direct = || {
            (
                self.client
                    .stream_url(item_key, media_source_id, play_session_id),
                crate::source::Delivery::Original,
                None,
            )
        };

        // Jellyfin keys an encoding by device + play session, so that pair IS
        // the only teardown handle. Without a session id a transcode could be
        // started and never stopped, so refuse to start one at all (tr-5).
        let Some(session) = play_session_id else {
            return direct();
        };

        // Only a real tier request costs a capability lookup. Original must not
        // pay a second PlaybackInfo round trip on every play (tr-7).
        if quality == crate::config::PLAYBACK_QUALITY_ORIGINAL
            || quality == crate::config::PLAYBACK_QUALITY_AUTOMATIC
        {
            return direct();
        }

        let options = self.playback_options_for(item_key, media_source_id).await;
        match options.resolve(quality).tier() {
            Some(tier) => (
                self.client.transcode_url(
                    item_key,
                    media_source_id,
                    Some(session),
                    tier,
                    start_ticks,
                ),
                crate::source::Delivery::Transcode(tier),
                Some(session.to_string()),
            ),
            None => direct(),
        }
    }

    /// Marker lookup, but only when a policy actually asked for it: a disabled
    /// feature must cost the server no request at all.
    async fn markers_if_enabled(&self, item_key: &str, include_markers: bool) -> Vec<MediaMarker> {
        if !include_markers {
            return Vec::new();
        }
        self.client.media_segments(item_key).await
    }

    pub fn new(id: impl Into<String>, name: impl Into<String>, client: JellyfinClient) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            client,
        }
    }

    fn to_item(&self, item: &BaseItem) -> ItemDto {
        let poster = item
            .image_tags
            .as_ref()
            .and_then(|t| t.primary.as_ref())
            .map(|tag| self.client.poster_url(&item.id, tag));
        // An episode's series poster (portrait art for catalog rows).
        let series_poster = item
            .series_id
            .as_ref()
            .zip(item.series_primary_image_tag.as_ref())
            .map(|(sid, tag)| self.client.poster_url(sid, tag));
        let backdrop = item
            .backdrop_image_tags
            .as_ref()
            .and_then(|tags| tags.first())
            .map(|tag| self.client.backdrop_url(&item.id, tag))
            .or_else(|| {
                // Episodes rarely carry backdrops; their Primary image IS the
                // 16:9 scene still — request it at hero resolution.
                if item.item_type.as_deref() == Some("Episode") {
                    item.image_tags
                        .as_ref()
                        .and_then(|t| t.primary.as_ref())
                        .map(|tag| self.client.hero_still_url(&item.id, tag))
                } else {
                    None
                }
            });
        let view_offset_ms = item
            .user_data
            .as_ref()
            .and_then(|u| u.playback_position_ticks)
            .filter(|t| *t > 0)
            .map(ticks_to_ms);
        ItemDto {
            rating_key: namespace_key(&self.id, &item.id),
            title: item.name.clone().unwrap_or_default(),
            year: item.production_year,
            summary: item.overview.clone(),
            duration_ms: item.run_time_ticks.map(ticks_to_ms),
            media_type: map_type(item.item_type.as_deref()),
            poster,
            series_poster,
            backdrop,
            view_offset_ms,
            played: item.user_data.as_ref().and_then(|u| u.played),
            // Jellyfin reports LastPlayedDate as an ISO-8601 string; parsing
            // it without a date dependency isn't worth it yet (follow-up).
            last_watched_at_ms: None,
            // DateCreated is an ISO-8601 string not requested in Fields= today;
            // date-added sort works server-side for JF, so the DTO field is a
            // follow-up (needed only for the merged view). None for now.
            added_at_ms: None,
            index: item.index_number,
            parent_index: item.parent_index_number,
            grandparent_title: item.series_name.clone(),
            parent_title: item.season_name.clone(),
            // Container-navigation keys: an episode's season/series, a
            // season's series (its parent). Namespaced like rating_key.
            parent_rating_key: match item.item_type.as_deref() {
                Some("Episode") => item.season_id.as_deref(),
                Some("Season") => item.series_id.as_deref(),
                _ => None,
            }
            .map(|k| namespace_key(&self.id, k)),
            grandparent_rating_key: if item.item_type.as_deref() == Some("Episode") {
                item.series_id
                    .as_deref()
                    .map(|k| namespace_key(&self.id, k))
            } else {
                None
            },
            source_id: self.id.clone(),
            // {"Imdb": "tt0133093"} → "imdb:tt0133093", matching Plex's form.
            provider_ids: item
                .provider_ids
                .as_ref()
                .map(|m| {
                    m.iter()
                        .filter(|(_, v)| !v.is_empty())
                        .map(|(k, v)| format!("{}:{v}", k.to_lowercase()))
                        .collect()
                })
                .unwrap_or_default(),
            backing: None,
            canonical_id: None,
            watch_key: None,
            detail_key: None,
        }
    }

    fn user_items_path(&self) -> String {
        format!("/Users/{}/Items", self.client.user_id)
    }

    fn to_playlist(&self, item: &BaseItem) -> PlaylistDto {
        PlaylistDto {
            key: namespace_key(&self.id, &item.id),
            title: item.name.clone().unwrap_or_default(),
            item_count: item.child_count,
            source_id: self.id.clone(),
            source_name: self.name.clone(),
        }
    }

    fn to_episode_context(&self, item: &BaseItem) -> Option<EpisodeContext> {
        (item.item_type.as_deref() == Some("Episode")).then_some(EpisodeContext {
            item_key: namespace_key(&self.id, &item.id),
            season_key: item
                .season_id
                .as_deref()
                .map(|key| namespace_key(&self.id, key)),
            show_key: item
                .series_id
                .as_deref()
                .map(|key| namespace_key(&self.id, key)),
            episode_index: item.index_number,
            season_index: item.parent_index_number,
        })
    }
}

#[async_trait]
impl MediaSource for JellyfinSource {
    fn id(&self) -> String {
        self.id.clone()
    }
    fn name(&self) -> String {
        self.name.clone()
    }
    fn kind(&self) -> &'static str {
        self.client.flavor.kind()
    }

    /// Trigger a server-side scan of one library. The user view must map to
    /// a concrete server library (same id in the virtual-folders list) —
    /// grouped views have no single scan target, so they're rejected with
    /// guidance instead of blind-POSTing an id the endpoint may misread.
    ///
    /// Provenance is unused: this source is pinned to ONE server address for
    /// its whole life (no discovery, no rediscovery), and a Jellyfin/Emby
    /// library id is a server-issued GUID rather than a small server-local
    /// number — so a key cannot silently come to mean another server's library
    /// the way a Plex section key can. The virtual-folders check above already
    /// proves the id belongs to the server being asked.
    async fn scan_library(&self, section_key: &str, _provenance: Option<&str>) -> Result<(), String> {
        let folders = self.client.get_virtual_folders().await?;
        let known = folders
            .iter()
            .any(|f| f.item_id.as_deref() == Some(section_key));
        if !known {
            return Err(
                "this view groups multiple server libraries; scan them individually from the server dashboard"
                    .to_string(),
            );
        }
        let url = self.client.scan_url(section_key)?;
        self.client.post_empty_url(&url).await
    }

    async fn sections(&self) -> Result<Vec<SectionDto>, String> {
        let r: ItemsResponse = self
            .client
            .get_json(&format!("/Users/{}/Views", self.client.user_id), &[])
            .await?;
        Ok(r.items
            .into_iter()
            .filter_map(|v| {
                // Only video libraries — skip music/photos/books/livetv/etc. so
                // non-playable media never reaches the nav or mpv.
                let section_type = match v.collection_type.as_deref() {
                    Some("movies") => "movie",
                    Some("tvshows") => "show",
                    Some("homevideos") | Some("musicvideos") => "video",
                    _ => return None,
                };
                Some(SectionDto {
                    key: namespace_key(&self.id, &v.id),
                    title: v.name.unwrap_or_default(),
                    section_type: section_type.to_string(),
                    source_id: self.id.clone(),
                    source_name: self.name.clone(),
                    sort: None, // stamped from config by get_sections
                    // Not needed here (see `scan_library`): one fixed server,
                    // and library ids are server-issued GUIDs.
                    provenance: None,
                    binding: 0, // this source cannot rebind: one address for life
                })
            })
            .collect())
    }

    async fn hubs(&self) -> Result<Vec<HubDto>, String> {
        let uid = &self.client.user_id;
        let mut hubs = Vec::new();

        let resume: ItemsResponse = self
            .client
            .get_json(
                &format!("/Users/{uid}/Items/Resume"),
                &[
                    ("Limit", "12".to_string()),
                    ("Recursive", "true".to_string()),
                    ("MediaTypes", "Video".to_string()),
                    ("Fields", "Overview,ProviderIds".to_string()),
                ],
            )
            .await?;
        if !resume.items.is_empty() {
            hubs.push(HubDto {
                title: "Continue Watching".to_string(),
                hub_identifier: "resume".to_string(),
                hub_type: "video".to_string(),
                items: resume.items.iter().map(|i| self.to_item(i)).collect(),
                source_id: self.id.clone(),
                source_name: self.name.clone(),
            });
        }

        // /Items/Latest returns a bare array, not an ItemsResponse.
        let latest: Vec<BaseItem> = self
            .client
            .get_json(
                &format!("/Users/{uid}/Items/Latest"),
                &[
                    ("Limit", "20".to_string()),
                    ("Fields", "Overview,ProviderIds".to_string()),
                    // Keep mixed libraries from surfacing audio/photos/books here.
                    (
                        "IncludeItemTypes",
                        "Movie,Episode,Video,MusicVideo".to_string(),
                    ),
                ],
            )
            .await?;
        if !latest.is_empty() {
            hubs.push(HubDto {
                title: "Recently Added".to_string(),
                hub_identifier: "latest".to_string(),
                hub_type: "video".to_string(),
                items: latest.iter().map(|i| self.to_item(i)).collect(),
                source_id: self.id.clone(),
                source_name: self.name.clone(),
            });
        }
        Ok(hubs)
    }

    async fn items(
        &self,
        section_key: &str,
        section_type: &str,
        sort: Option<&str>,
        start: usize,
        size: usize,
    ) -> Result<Vec<ItemDto>, String> {
        let (by, order) = map_sort(sort);
        let mut query = vec![
            ("ParentId", section_key.to_string()),
            ("StartIndex", start.to_string()),
            ("Limit", size.to_string()),
            ("Recursive", "true".to_string()),
            ("SortBy", by),
            ("SortOrder", order),
            ("Fields", "Overview,ProviderIds".to_string()),
        ];
        match section_type {
            "movie" => query.push(("IncludeItemTypes", "Movie".to_string())),
            "show" => query.push(("IncludeItemTypes", "Series".to_string())),
            // homevideos / musicvideos libraries hold Video/MusicVideo items.
            "video" => query.push(("IncludeItemTypes", "Movie,Video,MusicVideo".to_string())),
            _ => {}
        }
        let r: ItemsResponse = self
            .client
            .get_json(&self.user_items_path(), &query)
            .await?;
        Ok(r.items.iter().map(|i| self.to_item(i)).collect())
    }

    async fn search(&self, query: &str) -> Result<Vec<ItemDto>, String> {
        let r: ItemsResponse = self
            .client
            .get_json(
                &self.user_items_path(),
                &[
                    ("searchTerm", query.to_string()),
                    ("Recursive", "true".to_string()),
                    (
                        "IncludeItemTypes",
                        "Movie,Series,Episode,Video,MusicVideo".to_string(),
                    ),
                    ("Limit", "50".to_string()),
                    ("Fields", "Overview,ProviderIds".to_string()),
                ],
            )
            .await?;
        Ok(r.items.iter().map(|i| self.to_item(i)).collect())
    }

    async fn children(
        &self,
        item_key: &str,
        start: usize,
        size: usize,
    ) -> Result<Vec<ItemDto>, String> {
        let r: ItemsResponse = self
            .client
            .get_json(
                &self.user_items_path(),
                &[
                    ("ParentId", item_key.to_string()),
                    ("StartIndex", start.to_string()),
                    ("Limit", size.to_string()),
                    (
                        "SortBy",
                        "ParentIndexNumber,IndexNumber,SortName".to_string(),
                    ),
                    ("SortOrder", "Ascending".to_string()),
                    ("Fields", "Overview,ProviderIds".to_string()),
                ],
            )
            .await?;
        Ok(r.items.iter().map(|i| self.to_item(i)).collect())
    }

    async fn playlists(&self) -> Result<Vec<PlaylistDto>, String> {
        let query = playlist_list_query().map(|(key, value)| (key, value.to_string()));
        let response: ItemsResponse = self
            .client
            .get_json(&self.user_items_path(), &query)
            .await?;
        Ok(response
            .items
            .iter()
            // The query is authoritative; this defensive filter catches a
            // server that ignores MediaTypes without rejecting older servers
            // that omit MediaType from a filtered response.
            .filter(|item| is_video_playlist(item))
            .map(|item| self.to_playlist(item))
            .collect())
    }

    async fn playlist_items(&self, playlist_key: &str) -> Result<Vec<ItemDto>, String> {
        const PAGE: usize = 500;
        let mut start = 0;
        let mut all = Vec::new();
        let mut previous_signature: Option<(String, String)> = None;
        loop {
            let url = self.client.playlist_items_url(playlist_key, start, PAGE)?;
            let response: ItemsResponse = self.client.get_json_url(&url, &[]).await?;
            let total = response.total_record_count;
            let count = response.items.len();
            if count == 0 {
                break;
            }
            let signature = (
                response.items.first().map(|item| item.id.clone()).unwrap_or_default(),
                response.items.last().map(|item| item.id.clone()).unwrap_or_default(),
            );
            if start > 0 && previous_signature.as_ref() == Some(&signature) {
                return Err("the server did not advance playlist pagination".to_string());
            }
            previous_signature = Some(signature);
            all.extend(response.items);
            start += count;
            if total.is_some_and(|total| start >= total) || (total.is_none() && count < PAGE) {
                break;
            }
        }
        Ok(all
            .iter()
            .filter(|item| {
                matches!(
                    item.item_type.as_deref(),
                    Some("Movie" | "Episode" | "Video" | "MusicVideo")
                )
            })
            .map(|item| self.to_item(item))
            .collect())
    }

    async fn episode_context(&self, item_key: &str) -> Result<Option<EpisodeContext>, String> {
        let url = self.client.user_item_url(item_key)?;
        let item: BaseItem = self.client.get_json_url(&url, &[]).await?;
        Ok(self.to_episode_context(&item))
    }

    async fn resolve_stream(
        &self,
        item_key: &str,
        _duration_ms: Option<u64>,
        include_markers: bool,
        quality: &str,
    ) -> Result<StreamResolution, String> {
        // The optional marker lookup rides alongside ALL of the mandatory
        // resolve work, not just its first request, so in the normal case it
        // costs no extra wall-clock before mpv launches.
        let item_path = format!("/Users/{}/Items/{}", self.client.user_id, item_key);
        let mandatory = async {
            // Fetch the item to read its server-side resume position.
            let item: BaseItem = self.client.get_json(&item_path, &[]).await?;
            // Negotiate the real media source + play session for the stream and
            // check-ins (multi-version items, history/dashboard correctness).
            let identity = self.client.playback_info(item_key).await?;
            Ok::<_, String>((item, identity))
        };
        let (mandatory, markers) =
            tokio::join!(mandatory, self.markers_if_enabled(item_key, include_markers));
        let (item, (media_source_id, play_session_id)) = mandatory?;
        let resume_ms = item
            .user_data
            .and_then(|u| u.playback_position_ticks)
            .filter(|t| *t > 0)
            .map(ticks_to_ms)
            .unwrap_or(0);

        let (url, delivery, transcode_session) = self.deliver(
            item_key,
            &media_source_id,
            play_session_id.as_deref(),
            quality,
            (resume_ms * 10_000) as i64,
        )
        .await;
        Ok(StreamResolution {
            url,
            resume_ms,
            progress: ProgressTarget::Jellyfin(JellyfinTrack {
                base_url: self.client.base_url.clone(),
                item_id: item_key.to_string(),
                media_source_id,
                play_session_id,
                headers: self.client.auth_headers(),
            }),
            http_headers: self.client.stream_auth_headers(),
            delivery,
            transcode_session,
            markers,
        })
    }

    async fn playback_options(
        &self,
        item_key: &str,
        version_id: Option<&str>,
    ) -> Result<crate::source::PlaybackOptions, String> {
        let info = self.client.playback_info_response(item_key).await?;
        let source = version_id
            .and_then(|id| info.media_sources.iter().find(|source| source.id == id))
            .or_else(|| info.media_sources.first());
        Ok(source
            .map(|source| source.playback_options())
            .unwrap_or_else(|| crate::source::PlaybackOptions::new(true, false, 0, 0, 0)))
    }

    /// Stop an encoding this source started. Jellyfin keys it by device plus
    /// play session, so the session id carried out of `resolve_stream` is the
    /// handle.
    async fn stop_transcode(&self, session: &str) {
        self.client.stop_transcode(session).await;
    }

    async fn playback_versions(&self, item_key: &str) -> Result<Vec<PlaybackVersion>, String> {
        let info = self.client.playback_info_response(item_key).await?;
        let endpoint = url::Url::parse(&self.client.base_url)
            .map_err(|_| "media server endpoint is invalid".to_string())?;
        if info.media_sources.is_empty() {
            return Ok(vec![PlaybackVersion {
                source_id: self.id.clone(),
                source_name: self.name.clone(),
                item_key: namespace_key(&self.id, item_key),
                version_id: item_key.to_string(),
                width: 0,
                height: 0,
                hdr: false,
                bitrate: 0,
                direct_play_rank: 2,
                endpoint,
                provider_verified_local: false,
            }]);
        }
        Ok(info
            .media_sources
            .iter()
            .filter(|source| !source.id.is_empty())
            .map(|source| PlaybackVersion {
                source_id: self.id.clone(),
                source_name: self.name.clone(),
                item_key: namespace_key(&self.id, item_key),
                version_id: source.id.clone(),
                width: source.video_width(),
                height: source.video_height(),
                hdr: source.is_hdr(),
                bitrate: source.bitrate(),
                direct_play_rank: source.candidate_direct_rank(),
                endpoint: endpoint.clone(),
                // MediaSource.IsRemote describes the server's relationship to
                // its storage, not Vela's network path to the server.
                provider_verified_local: false,
            })
            .collect())
    }

    async fn resolve_stream_version(
        &self,
        item_key: &str,
        _duration_ms: Option<u64>,
        version_id: &str,
        include_markers: bool,
        quality: &str,
    ) -> Result<StreamResolution, String> {
        // As in `resolve_stream`: the optional marker lookup overlaps every
        // mandatory request, never sits in front of one.
        let item_path = format!("/Users/{}/Items/{}", self.client.user_id, item_key);
        let mandatory = async {
            let item: BaseItem = self.client.get_json(&item_path, &[]).await?;
            let info = self.client.playback_info_response(item_key).await?;
            Ok::<_, String>((item, info))
        };
        let (mandatory, markers) =
            tokio::join!(mandatory, self.markers_if_enabled(item_key, include_markers));
        let (item, info) = mandatory?;
        let resume_ms = item
            .user_data
            .and_then(|data| data.playback_position_ticks)
            .filter(|ticks| *ticks > 0)
            .map(ticks_to_ms)
            .unwrap_or(0);
        let (media_source_id, play_session_id) =
            exact_playback_identity(info, item_key, version_id)?;
        let (url, delivery, transcode_session) = self.deliver(
            item_key,
            &media_source_id,
            play_session_id.as_deref(),
            quality,
            (resume_ms * 10_000) as i64,
        )
        .await;
        Ok(StreamResolution {
            url,
            resume_ms,
            progress: ProgressTarget::Jellyfin(JellyfinTrack {
                base_url: self.client.base_url.clone(),
                item_id: item_key.to_string(),
                media_source_id,
                play_session_id,
                headers: self.client.auth_headers(),
            }),
            http_headers: self.client.stream_auth_headers(),
            delivery,
            transcode_session,
            markers,
        })
    }

    async fn mark_played(&self, item_key: &str, played: bool) -> Result<(), String> {
        self.client.set_played(item_key, played).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn map_sort_maps_leaf_added_to_date_last_content_added() {
        let (by, order) = map_sort(Some("episodeAddedAt:desc"));
        assert_eq!(by, "DateLastContentAdded");
        assert_eq!(order, "Descending");
        // The series-level date-added sort stays distinct from the leaf one.
        let (by, _) = map_sort(Some("addedAt:desc"));
        assert_eq!(by, "DateCreated");
    }

    /// One-shot HTTP responder that hands back `response` and returns whatever
    /// request it received, so tests can pin the exact wire form.
    async fn segment_server(response: Vec<u8>) -> (u16, tokio::task::JoinHandle<String>) {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let task = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut bytes = Vec::new();
            let mut chunk = [0_u8; 2048];
            loop {
                let count = stream.read(&mut chunk).await.unwrap();
                if count == 0 {
                    break;
                }
                bytes.extend_from_slice(&chunk[..count]);
                if bytes.windows(4).any(|window| window == b"\r\n\r\n") {
                    break;
                }
            }
            stream.write_all(&response).await.unwrap();
            String::from_utf8(bytes).unwrap()
        });
        (port, task)
    }

    fn segment_client(flavor: Flavor, port: u16) -> JellyfinClient {
        JellyfinClient::new(
            flavor,
            &format!("http://127.0.0.1:{port}"),
            "test-device",
            "synthetic-token",
            "user-1",
        )
    }

    fn json_response(status: &str, body: &str) -> Vec<u8> {
        format!(
            "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{body}",
            body.len()
        )
        .into_bytes()
    }

    // Jellyfin's Outro is Vela's Credits, ticks are 100ns, and segment kinds
    // Vela does not model must not be skipped past.
    #[tokio::test]
    async fn media_segments_map_kinds_and_ticks_over_the_pinned_route() {
        let body = r#"{"Items":[
            {"Type":"Outro","StartTicks":13800000000,"EndTicks":14400000000},
            {"Type":"Intro","StartTicks":70000000,"EndTicks":670000000},
            {"Type":"Commercial","StartTicks":3000000000,"EndTicks":3300000000},
            {"Type":"Preview","StartTicks":0,"EndTicks":50000000},
            {"Type":"Intro","StartTicks":900000000,"EndTicks":800000000}
        ],"TotalRecordCount":5}"#;
        let (port, captured) = segment_server(json_response("200 OK", body)).await;
        let markers = segment_client(Flavor::Jellyfin, port)
            .media_segments("item-7")
            .await;
        assert_eq!(
            markers,
            vec![
                MediaMarker {
                    kind: MarkerKind::Intro,
                    start_ms: 7_000,
                    end_ms: 67_000
                },
                MediaMarker {
                    kind: MarkerKind::Commercial,
                    start_ms: 300_000,
                    end_ms: 330_000
                },
                MediaMarker {
                    kind: MarkerKind::Credits,
                    start_ms: 1_380_000,
                    end_ms: 1_440_000
                },
            ],
            "Preview is dropped and the inverted Intro does not survive normalize"
        );
        let request = captured.await.unwrap();
        let request_line = request.lines().next().unwrap_or_default();
        assert!(
            request_line.starts_with("GET /MediaSegments/item-7?"),
            "the segments route is a range API of its own: {request_line}"
        );
        for kind in ["Intro", "Outro", "Commercial"] {
            assert!(
                request_line.contains(&format!("includeSegmentTypes={kind}")),
                "the repeated filter must ask for {kind}: {request_line}"
            );
        }
    }

    // An older Jellyfin without the route must still play; markers are always
    // best-effort. The signature carries no error channel at all, so a marker
    // failure structurally cannot fail a resolve.
    #[tokio::test]
    async fn media_segments_yield_nothing_when_the_route_is_unsupported() {
        let (port, _captured) = segment_server(json_response("404 Not Found", "{}")).await;
        let markers = segment_client(Flavor::Jellyfin, port)
            .media_segments("item-7")
            .await;
        assert!(markers.is_empty());
    }

    // A marker endpoint that accepts the connection and then goes silent must
    // not be able to hold a launch for the general 15-second request timeout.
    // The user is waiting on mpv; markers are optional.
    #[tokio::test]
    async fn media_segments_are_bounded_when_the_endpoint_stalls() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        // Accept the connection, then never answer.
        let _silent = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            std::future::pending::<()>().await;
            drop(stream);
        });
        let started = std::time::Instant::now();
        let markers = segment_client(Flavor::Jellyfin, port)
            .media_segments("item-7")
            .await;
        let elapsed = started.elapsed();
        assert!(markers.is_empty(), "a stalled lookup yields no markers");
        assert!(
            elapsed < std::time::Duration::from_secs(10),
            "the marker lookup must be bounded well below the 15s general \
             request timeout, took {elapsed:?}"
        );
    }

    // Emby publishes no MediaSegments contract. Shared ancestry is not evidence
    // of a shared route, so Vela must not probe for it at all.
    #[tokio::test]
    async fn emby_never_requests_media_segments() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let markers = segment_client(Flavor::Emby, port)
            .media_segments("item-7")
            .await;
        assert!(markers.is_empty());
        let accepted =
            tokio::time::timeout(std::time::Duration::from_millis(250), listener.accept()).await;
        assert!(
            accepted.is_err(),
            "an Emby server must receive no MediaSegments request"
        );
    }

    // The transcode URL must name the copy the user chose and carry the tier's
    // ceiling. Omitting mediaSourceId lets the server pick a source, which for
    // a multi-version title is not necessarily the one that was selected.
    #[test]
    fn jellyfin_transcode_url_pins_the_source_and_the_tier() {
        let client = segment_client(Flavor::Jellyfin, 8096);
        let tier = crate::source::QUALITY_TIERS
            .iter()
            .find(|tier| tier.id == "720p-2000")
            .copied()
            .expect("tier exists");

        let url = client.transcode_url("item-7", "source-2", Some("ps-9"), tier, 0);
        assert!(url.contains("/Videos/item-7/master.m3u8"), "{url}");
        assert!(url.contains("mediaSourceId=source-2"), "{url}");
        assert!(url.contains("PlaySessionId=ps-9"), "{url}");
        assert!(
            url.contains("videoBitRate=2000000"),
            "the ladder is kbps; Jellyfin wants bits per second: {url}"
        );
        assert!(url.contains("maxWidth=1280") && url.contains("maxHeight=720"), "{url}");

        // Resume must reach the transcoder, or the server starts at zero and
        // mpv seeks into a stream that begins somewhere else.
        let resumed = client.transcode_url("item-7", "source-2", None, tier, 9_300_000_000);
        assert!(resumed.contains("startTimeTicks=9300000000"), "{resumed}");
        assert!(
            !resumed.contains("PlaySessionId"),
            "an absent session id must not appear as an empty parameter: {resumed}"
        );
    }

    // A server that has not said it can transcode must never have a conversion
    // offered on its behalf — the menu would list something that then fails.
    #[test]
    fn transcoding_is_offered_only_when_the_server_says_so() {
        let mut source = media_source("m1", true, false, false, 1080, 10_000_000);

        source.supports_transcoding = None;
        source.transcoding_url = None;
        assert!(
            source.playback_options().tiers.is_empty(),
            "silence is not consent: no flag and no URL means no transcoding"
        );

        source.supports_transcoding = Some(false);
        assert!(source.playback_options().tiers.is_empty());

        source.supports_transcoding = Some(true);
        let options = source.playback_options();
        assert_eq!(options.tiers.len(), 9);
        assert!(options.can_direct_play);
        assert_eq!(
            options.source_bitrate_kbps, 10_000,
            "Jellyfin reports bits per second; the ladder is kbps"
        );

        // Older servers omit the flag but still hand back a transcoding URL.
        source.supports_transcoding = None;
        source.transcoding_url = Some("/videos/m1/master.m3u8".to_string());
        assert_eq!(source.playback_options().tiers.len(), 9);
    }

    // TranscodeReasons has shipped as both an array and a joined string. Either
    // must parse, because failing here would break playback for a field nothing
    // reads.
    #[test]
    fn transcode_reasons_parses_in_both_wire_shapes() {
        for body in [
            r#"{"Id":"m1","SupportsTranscoding":true,"TranscodeReasons":["ContainerNotSupported"]}"#,
            r#"{"Id":"m1","SupportsTranscoding":true,"TranscodeReasons":"ContainerNotSupported,VideoCodecNotSupported"}"#,
            r#"{"Id":"m1","SupportsTranscoding":true}"#,
        ] {
            let parsed: MediaSourceInfo =
                serde_json::from_str(body).unwrap_or_else(|e| panic!("{body} failed: {e}"));
            assert_eq!(parsed.supports_transcoding, Some(true));
        }
    }

    #[test]
    fn vf_route_is_flavor_specific() {
        // Branch-flip guard: each flavor must map to ITS documented route —
        // swapping the match arms fails both assertions.
        assert_eq!(
            vf_route(Flavor::Jellyfin),
            ("/Library/VirtualFolders", VfEnvelope::Bare)
        );
        assert_eq!(
            vf_route(Flavor::Emby),
            ("/Library/VirtualFolders/Query", VfEnvelope::Items)
        );
    }

    #[test]
    fn playlist_queries_are_read_only_video_contracts() {
        assert_eq!(
            playlist_list_query(),
            [
                ("Recursive", "true"),
                ("IncludeItemTypes", "Playlist"),
                ("MediaTypes", "Video"),
                ("SortBy", "SortName"),
                ("SortOrder", "Ascending"),
                ("Fields", "ChildCount"),
            ]
        );
        let client = JellyfinClient::new(
            Flavor::Jellyfin,
            "http://jf.example:8096/base",
            "device",
            "token",
            "user-1",
        );
        let url = client
            .playlist_items_url("../odd/id?x=1", 500, 500)
            .expect("hostile ids are encoded as one segment");
        let parsed = url::Url::parse(&url).unwrap();
        let segments: Vec<_> = parsed.path_segments().unwrap().collect();
        assert_eq!(segments[segments.len() - 3], "Playlists");
        assert_eq!(segments[segments.len() - 1], "Items");
        assert!(segments[segments.len() - 2].contains("%2F"));
        assert!(segments[segments.len() - 2].contains("%3F"));
        let query: std::collections::HashMap<_, _> = parsed.query_pairs().into_owned().collect();
        assert_eq!(query.get("UserId").map(String::as_str), Some("user-1"));
        assert_eq!(query.get("EnableUserData").map(String::as_str), Some("true"));
        assert_eq!(query.get("StartIndex").map(String::as_str), Some("500"));
        assert_eq!(query.get("Limit").map(String::as_str), Some("500"));
    }

    #[test]
    fn jellyfin_and_emby_playlist_fixtures_map_video_descriptors() {
        let parsed: ItemsResponse = serde_json::from_str(
            r#"{
              "Items": [
                {"Id":"p-video","Name":"Film Night","Type":"Playlist","MediaType":"Video","ChildCount":3},
                {"Id":"p-audio","Name":"Songs","Type":"Playlist","MediaType":"Audio","ChildCount":9}
              ],
              "TotalRecordCount":2
            }"#,
        )
        .unwrap();
        assert!(is_video_playlist(&parsed.items[0]));
        assert!(!is_video_playlist(&parsed.items[1]));
        for (flavor, source_id) in [
            (Flavor::Jellyfin, "jf-one"),
            (Flavor::Emby, "emby-one"),
        ] {
            let source = JellyfinSource::new(
                source_id,
                "Server",
                JellyfinClient::new(flavor, "http://server", "dev", "token", "user"),
            );
            let dto = source.to_playlist(&parsed.items[0]);
            assert_eq!(dto.key, format!("{source_id}:p-video"));
            assert_eq!(dto.title, "Film Night");
            assert_eq!(dto.item_count, Some(3));
            assert_eq!(dto.source_id, source_id);
        }
    }

    #[test]
    fn virtual_folders_parse_both_envelope_shapes() {
        let bare: Vec<VirtualFolderInfo> =
            serde_json::from_str(r#"[{"Name":"Movies","ItemId":"lib1"}]"#).unwrap();
        assert_eq!(bare[0].item_id.as_deref(), Some("lib1"));

        let wrapped: VirtualFoldersEnvelope = serde_json::from_str(
            r#"{"Items":[{"Name":"Movies","ItemId":"lib2"}],"TotalRecordCount":1}"#,
        )
        .unwrap();
        assert_eq!(wrapped.items[0].item_id.as_deref(), Some("lib2"));
    }

    #[test]
    fn scan_url_shape_and_rejections() {
        let c = JellyfinClient::new(Flavor::Jellyfin, "http://s:8096", "dev", "sekrit", "u1");
        let url = c.scan_url("lib1").unwrap();
        // LITERAL expectations, not a loop over scan_query(): deriving them
        // from the function under test is tautological — flipping
        // ReplaceAllMetadata to true (a destructive metadata rewrite on the
        // user's server, not a scan) would stay green (lrs-7).
        assert_eq!(
            url,
            "http://s:8096/Items/lib1/Refresh?Recursive=true&MetadataRefreshMode=Default\
             &ImageRefreshMode=Default&ReplaceAllMetadata=false&ReplaceAllImages=false\
             &RegenerateTrickplay=false"
        );
        // Auth travels in headers; the token must never leak into the URL.
        assert!(!url.contains("sekrit"));
        // Ids that PathSegmentsMut::extend would silently drop are rejected
        // up front instead of mutating the endpoint shape.
        for bad in ["", ".", ".."] {
            assert!(c.scan_url(bad).is_err(), "{bad:?} must be rejected");
        }
    }

    #[test]
    fn stream_url_keeps_tokens_out_of_mpv_argv() {
        for flavor in [Flavor::Jellyfin, Flavor::Emby] {
            let client = JellyfinClient::new(
                flavor,
                "http://s:8096",
                "device-1",
                "sekrit",
                "user-1",
            );
            let url = client.stream_url("item-1", "version-2", Some("session-3"));
            assert!(!url.contains("sekrit"));
            assert!(!url.contains("api_key"));
            assert!(url.contains("mediaSourceId=version-2"));
            assert!(url.contains("PlaySessionId=session-3"));
            assert_eq!(
                client.stream_auth_headers(),
                vec![("X-Emby-Token".to_string(), "sekrit".to_string())]
            );
        }
    }

    /// The scan POST carries admin-capable credentials, and the raw section key
    /// is frontend-supplied, so a hostile id must stay ONE path segment and
    /// never steer the request at another authenticated endpoint. Guard proof:
    /// rebuild `scan_url` with `format!` interpolation and every case below
    /// fails (the plan's required coverage; the old test only rejected dot ids,
    /// so raw interpolation stayed green — lrs-6).
    #[test]
    fn scan_url_keeps_a_hostile_id_in_one_path_segment() {
        let c = JellyfinClient::new(Flavor::Jellyfin, "http://s:8096", "dev", "sekrit", "u1");
        for bad in [
            "../System/Shutdown?x=",
            "..\\System\\Shutdown",
            "a/b",
            "lib1?ReplaceAllMetadata=true",
            "lib1#frag",
        ] {
            let raw = c
                .scan_url(bad)
                .unwrap_or_else(|e| panic!("{bad:?} should be encoded, not rejected: {e}"));
            // Re-parse the way reqwest will: escapes that survive serialization
            // but collapse on parse are exactly the hole we are guarding.
            let u = url::Url::parse(&raw).unwrap_or_else(|e| panic!("{bad:?} -> {raw}: {e}"));
            let segs: Vec<&str> = u.path_segments().expect("cannot-be-a-base").collect();
            assert_eq!(
                segs.len(),
                3,
                "{bad:?} escaped the /Items/<id>/Refresh shape: {}",
                u.path()
            );
            assert_eq!(segs[0], "Items", "{bad:?} moved the endpoint: {}", u.path());
            assert_eq!(
                segs[2], "Refresh",
                "{bad:?} moved the endpoint: {}",
                u.path()
            );
            assert!(u.fragment().is_none(), "{bad:?} smuggled a fragment");
            // Only scan_query's own pairs may appear — no smuggled overrides.
            let q = u.query().unwrap_or_default();
            assert_eq!(
                q.split('&').count(),
                scan_query().len(),
                "{bad:?} smuggled query params: {q}"
            );
        }
    }

    #[test]
    fn scan_query_is_a_plain_nondestructive_scan() {
        // A scan must not become a destructive metadata/image rewrite: these
        // are the values jellyfin-web's own scan dialog sends. Literal by
        // design (see scan_url_shape_and_rejections).
        assert_eq!(
            scan_query(),
            [
                ("Recursive", "true"),
                ("MetadataRefreshMode", "Default"),
                ("ImageRefreshMode", "Default"),
                ("ReplaceAllMetadata", "false"),
                ("ReplaceAllImages", "false"),
                ("RegenerateTrickplay", "false"),
            ]
        );
    }

    fn media_source(
        id: &str,
        direct_play: bool,
        direct_stream: bool,
        hdr: bool,
        height: u32,
        bitrate: u64,
    ) -> MediaSourceInfo {
        MediaSourceInfo {
            id: id.to_string(),
            supports_direct_play: Some(direct_play),
            supports_direct_stream: Some(direct_stream),
            supports_transcoding: None,
            transcoding_url: None,
            transcode_reasons: None,
            bitrate: Some(bitrate),
            size: None,
            width: Some(height * 16 / 9),
            height: Some(height),
            media_streams: vec![MediaStreamInfo {
                stream_type: Some("Video".to_string()),
                video_range: Some(if hdr { "HDR10" } else { "SDR" }.to_string()),
                video_range_type: None,
                bit_rate: Some(bitrate),
                width: Some(height * 16 / 9),
                height: Some(height),
            }],
        }
    }

    fn test_client() -> JellyfinClient {
        JellyfinClient {
            flavor: Flavor::Jellyfin,
            base_url: "http://jf.example:8096".into(),
            device_id: "dev".into(),
            token: "tok".into(),
            user_id: "u1".into(),
            http: reqwest::Client::new(),
        }
    }

    #[test]
    fn to_item_namespaces_season_and_series_keys() {
        let src = JellyfinSource::new("jfA", "JF", test_client());
        let ep = BaseItem {
            id: "ep7".into(),
            item_type: Some("Episode".into()),
            season_id: Some("sea4".into()),
            series_id: Some("ser1".into()),
            ..Default::default()
        };
        let dto = src.to_item(&ep);
        assert_eq!(dto.parent_rating_key.as_deref(), Some("jfA:sea4"));
        assert_eq!(dto.grandparent_rating_key.as_deref(), Some("jfA:ser1"));
        assert_eq!(
            src.to_episode_context(&ep),
            Some(EpisodeContext {
                item_key: "jfA:ep7".into(),
                season_key: Some("jfA:sea4".into()),
                show_key: Some("jfA:ser1".into()),
                episode_index: None,
                season_index: None,
            })
        );

        // A season's parent is its series; it has no grandparent.
        let season = BaseItem {
            id: "sea4".into(),
            item_type: Some("Season".into()),
            series_id: Some("ser1".into()),
            ..Default::default()
        };
        let dto = src.to_item(&season);
        assert_eq!(dto.parent_rating_key.as_deref(), Some("jfA:ser1"));
        assert_eq!(dto.grandparent_rating_key, None);

        // Movies carry neither.
        let movie = BaseItem {
            id: "m1".into(),
            item_type: Some("Movie".into()),
            ..Default::default()
        };
        let dto = src.to_item(&movie);
        assert_eq!(dto.parent_rating_key, None);
        assert_eq!(dto.grandparent_rating_key, None);
        assert_eq!(src.to_episode_context(&movie), None);
    }

    #[test]
    fn episode_context_item_url_keeps_hostile_ids_in_one_segment() {
        let url = test_client().user_item_url("../odd/id?x=1").unwrap();
        let parsed = url::Url::parse(&url).unwrap();
        let segments: Vec<_> = parsed.path_segments().unwrap().collect();
        assert_eq!(segments[segments.len() - 2], "Items");
        assert!(segments[segments.len() - 1].contains("%2F"));
        assert!(segments[segments.len() - 1].contains("%3F"));
    }

    #[test]
    fn artwork_urls_are_sized_and_encoded() {
        let c = test_client();
        let bd = c.backdrop_url("item1", "tag/1");
        assert!(bd.starts_with("http://jf.example:8096/Items/item1/Images/Backdrop/0?"));
        assert!(bd.contains("fillHeight=1080"));
        assert!(bd.contains("fillWidth=1920"));
        // The tag rides percent-encoded so it can't malform the query.
        assert!(bd.contains("tag=tag%2F1"));

        // Episode hero stills come from Primary at the same hero resolution.
        let hs = c.hero_still_url("ep7", "t7");
        assert!(hs.contains("/Items/ep7/Images/Primary"));
        assert!(hs.contains("fillHeight=1080"));
        assert!(hs.contains("fillWidth=1920"));

        // The series poster reuses the primary-image shape at grid size.
        let sp = c.poster_url("series9", "t9");
        assert!(sp.contains("/Items/series9/Images/Primary"));
        assert!(sp.contains("fillHeight=450"));
        assert!(sp.contains("fillWidth=300"));
    }

    #[test]
    fn media_source_selection_prefers_resolution_before_hdr() {
        let sources = vec![
            media_source("sdr-4k", true, true, false, 2160, 80_000_000),
            media_source("hdr-1080", true, true, true, 1080, 20_000_000),
        ];

        assert_eq!(
            select_media_source(&sources).map(|s| s.id.as_str()),
            Some("sdr-4k")
        );
    }

    #[test]
    fn media_source_selection_prefers_direct_play_over_direct_stream() {
        let sources = vec![
            media_source("direct-stream-hdr", false, true, true, 2160, 80_000_000),
            media_source("direct-play-sdr", true, true, false, 1080, 20_000_000),
        ];

        assert_eq!(
            select_media_source(&sources).map(|s| s.id.as_str()),
            Some("direct-play-sdr")
        );
    }

    #[test]
    fn exact_media_source_revalidation_keeps_the_fresh_tracking_session() {
        let info = PlaybackInfoResp {
            media_sources: vec![
                media_source("source-a", true, true, false, 1080, 20_000_000),
                media_source("source-b", true, true, true, 2160, 80_000_000),
            ],
            play_session_id: Some("fresh-session".to_string()),
        };
        let (media_source_id, play_session_id) =
            exact_playback_identity(info, "item-1", "source-b").unwrap();
        assert_eq!(media_source_id, "source-b");
        assert_eq!(play_session_id.as_deref(), Some("fresh-session"));
    }

    #[test]
    fn exact_media_source_revalidation_rejects_a_disappeared_version() {
        let info = PlaybackInfoResp {
            media_sources: vec![media_source(
                "source-b",
                true,
                true,
                true,
                2160,
                80_000_000,
            )],
            play_session_id: Some("fresh-session".to_string()),
        };
        assert!(exact_playback_identity(info, "item-1", "source-a").is_err());
    }

    #[test]
    fn media_source_selection_falls_back_to_best_quality() {
        let sources = vec![
            media_source("sdr-720", false, false, false, 720, 5_000_000),
            media_source("hdr-4k", false, false, true, 2160, 60_000_000),
        ];

        assert_eq!(
            select_media_source(&sources).map(|s| s.id.as_str()),
            Some("hdr-4k")
        );
    }
}
