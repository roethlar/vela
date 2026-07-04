//! Jellyfin / Emby backend. The two share almost all of their HTTP API (Jellyfin
//! forked from Emby), so one client serves both; the places they genuinely differ
//! — chiefly the auth header scheme — are isolated behind [`Flavor`] so they can
//! keep diverging without reshaping the rest of the code.

use async_trait::async_trait;
use serde::de::DeserializeOwned;
use serde::Deserialize;

use super::{namespace_key, HubDto, ItemDto, MediaSource, SectionDto, StreamResolution};
use crate::playback::{JellyfinTrack, ProgressTarget};

/// From Cargo.toml, so the device-identity header can't drift from the package
/// version (matches the Plex/UI version).
const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Error string the frontend recognizes: the stored token/key was rejected and
/// we have no credential to retry with, so P2f must prompt for reconnection.
pub const RECONNECT_REQUIRED: &str = "RECONNECT_REQUIRED";

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

    async fn get_json<T: DeserializeOwned>(
        &self,
        path: &str,
        query: &[(&str, String)],
    ) -> Result<T, String> {
        let url = format!("{}{}", self.base_url, path);
        // Per-request timeout as a safety net: if the shared client ever fell back
        // to a default (no-timeout) build, a stuck request still can't hang here.
        let mut rb = self
            .http
            .get(&url)
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
    /// NOTE: the api_key rides in the URL (and thus mpv's argv) — accepted as a
    /// local-only exposure, consistent with the Plex path.
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
            ("api_key", self.token.as_str()),
        ];
        if let Some(ps) = play_session_id {
            pairs.push(("PlaySessionId", ps));
        }
        self.build_url(&["Videos", item_id, "stream"], &pairs)
    }

    /// Negotiate playback: returns the real `MediaSourceId` and a `PlaySessionId`
    /// to thread through the stream URL and check-in posts.
    async fn playback_info(&self, item_id: &str) -> Result<(String, Option<String>), String> {
        let info: PlaybackInfoResp = self
            .get_json(
                &format!("/Items/{}/PlaybackInfo", item_id),
                &[("UserId", self.user_id.clone())],
            )
            .await?;
        let media_source_id = select_media_source(&info.media_sources)
            .and_then(|m| (!m.id.is_empty()).then(|| m.id.clone()))
            .unwrap_or_else(|| item_id.to_string());
        Ok((media_source_id, info.play_session_id))
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

    /// Landscape backdrop, sized for the hero/resume rendering (larger than
    /// grid posters). Same token-in-URL exposure as `poster_url`.
    fn backdrop_url(&self, item_id: &str, tag: &str) -> String {
        self.build_url(
            &["Items", item_id, "Images", "Backdrop", "0"],
            &[
                ("fillHeight", "720"),
                ("fillWidth", "1280"),
                ("tag", tag),
                ("api_key", self.token.as_str()),
            ],
        )
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
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct PlaybackInfoResp {
    #[serde(default)]
    media_sources: Vec<MediaSourceInfo>,
    play_session_id: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct MediaSourceInfo {
    #[serde(default)]
    id: String,
    supports_direct_play: Option<bool>,
    supports_direct_stream: Option<bool>,
    is_remote: Option<bool>,
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

#[derive(Deserialize)]
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
    series_primary_image_tag: Option<String>,
    backdrop_image_tags: Option<Vec<String>>,
    image_tags: Option<ImageTags>,
    collection_type: Option<String>,
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

fn media_source_quality_key(source: &MediaSourceInfo) -> (u8, bool, bool, u32, u32, u64, u64) {
    (
        source.direct_rank(),
        !source.is_remote.unwrap_or(false),
        source.is_hdr(),
        source.video_height(),
        source.video_width(),
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

/// Translate the UI's Plex-style sort token to Jellyfin's SortBy/SortOrder.
fn map_sort(sort: Option<&str>) -> (String, String) {
    let s = sort.unwrap_or("titleSort:asc");
    let (field, dir) = s.split_once(':').unwrap_or((s, "asc"));
    let by = match field {
        "year" => "ProductionYear,PremiereDate",
        "addedAt" => "DateCreated",
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
pub fn build_source(cfg: &crate::config::SourceConfig) -> Option<std::sync::Arc<dyn MediaSource>> {
    let flavor = Flavor::from_kind(&cfg.kind)?;
    // Require everything requests actually need, so a corrupt/partial config
    // can't restore as a broken live source. access_token (user login) or
    // api_key (headless) — either is the bearer token.
    let nonempty = |o: &Option<String>| o.clone().filter(|s| !s.is_empty());
    let token = nonempty(&cfg.access_token).or_else(|| nonempty(&cfg.api_key))?;
    let user_id = nonempty(&cfg.user_id)?;
    let device_id = nonempty(&cfg.device_id)?;
    if cfg.base_url.is_empty() {
        return None;
    }
    let client = JellyfinClient::new(flavor, &cfg.base_url, &device_id, &token, &user_id);
    Some(std::sync::Arc::new(JellyfinSource::new(
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
            .map(|tag| self.client.backdrop_url(&item.id, tag));
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
            index: item.index_number,
            parent_index: item.parent_index_number,
            grandparent_title: item.series_name.clone(),
            parent_title: item.season_name.clone(),
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
        }
    }

    fn user_items_path(&self) -> String {
        format!("/Users/{}/Items", self.client.user_id)
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

    async fn resolve_stream(
        &self,
        item_key: &str,
        _duration_ms: Option<u64>,
    ) -> Result<StreamResolution, String> {
        // Fetch the item to read its server-side resume position.
        let item: BaseItem = self
            .client
            .get_json(
                &format!("/Users/{}/Items/{}", self.client.user_id, item_key),
                &[],
            )
            .await?;
        let resume_ms = item
            .user_data
            .and_then(|u| u.playback_position_ticks)
            .filter(|t| *t > 0)
            .map(ticks_to_ms)
            .unwrap_or(0);

        // Negotiate the real media source + play session for the stream and
        // check-ins (multi-version items, history/dashboard correctness).
        let (media_source_id, play_session_id) = self.client.playback_info(item_key).await?;

        Ok(StreamResolution {
            url: self
                .client
                .stream_url(item_key, &media_source_id, play_session_id.as_deref()),
            resume_ms,
            progress: ProgressTarget::Jellyfin(JellyfinTrack {
                base_url: self.client.base_url.clone(),
                item_id: item_key.to_string(),
                media_source_id,
                play_session_id,
                headers: self.client.auth_headers(),
            }),
            // Jellyfin/Emby stream URLs still carry their token; header parity
            // is a recorded follow-up (`.agents/decisions.md`, 2026-07-03).
            http_headers: Vec::new(),
        })
    }

    async fn mark_played(&self, item_key: &str, played: bool) -> Result<(), String> {
        self.client.set_played(item_key, played).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
            is_remote: Some(false),
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
    fn artwork_urls_are_sized_and_encoded() {
        let c = test_client();
        let bd = c.backdrop_url("item1", "tag/1");
        assert!(bd.starts_with("http://jf.example:8096/Items/item1/Images/Backdrop/0?"));
        assert!(bd.contains("fillHeight=720"));
        assert!(bd.contains("fillWidth=1280"));
        // The tag rides percent-encoded so it can't malform the query.
        assert!(bd.contains("tag=tag%2F1"));

        // The series poster reuses the primary-image shape at grid size.
        let sp = c.poster_url("series9", "t9");
        assert!(sp.contains("/Items/series9/Images/Primary"));
        assert!(sp.contains("fillHeight=450"));
        assert!(sp.contains("fillWidth=300"));
    }

    #[test]
    fn media_source_selection_prefers_hdr_direct_candidates() {
        let sources = vec![
            media_source("sdr-4k", true, true, false, 2160, 80_000_000),
            media_source("hdr-1080", true, true, true, 1080, 20_000_000),
        ];

        assert_eq!(
            select_media_source(&sources).map(|s| s.id.as_str()),
            Some("hdr-1080")
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
