//! Plex backend implementing [`MediaSource`]. Wraps [`PlexLibrary`] and owns the
//! server discovery / stale-server-rediscovery / config-persistence logic that
//! previously lived in the command handlers.

use async_trait::async_trait;
use tokio::sync::Mutex as AsyncMutex;

use super::{
    namespace_key, CastMember, DetailDto, HubDto, ItemDto, MediaSource, MediaStreamDto,
    MediaVersionDto, PersonRef, SectionDto, StreamResolution,
};
use crate::playback::{ProgressTarget, TrackInfo};
use crate::plex_library::{PlexDetail, PlexLibrary, PlexServer, PlexVideo};

pub struct PlexSource {
    id: String,
    name: String,
    lib: AsyncMutex<PlexLibrary>,
}

impl PlexSource {
    pub fn new(id: impl Into<String>, name: impl Into<String>, lib: PlexLibrary) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            lib: AsyncMutex::new(lib),
        }
    }

    /// A clone of the client with a server already selected, discovering one on
    /// first use. Cloned out so we don't hold the lock across network calls.
    async fn ensure_ready(&self) -> Result<PlexLibrary, String> {
        {
            let guard = self.lib.lock().await;
            if guard.server_base().is_some() {
                return Ok(guard.clone());
            }
        }
        self.rediscover().await
    }

    /// Run discovery, pick a server, persist it. Recovers from a stale saved server.
    async fn rediscover(&self) -> Result<PlexLibrary, String> {
        self.rediscover_on(None).await
    }

    /// Rediscover, but only among servers whose machine id matches `machine`
    /// (when given). Discovery normally takes the first REACHABLE server on the
    /// account and PERSISTS it — so on a multi-server account an unfiltered
    /// rediscover can silently repoint this source at a different machine. Any
    /// caller holding a server-LOCAL id (a section key) must pin the machine,
    /// or its id will be applied to a stranger's library. Rejecting the result
    /// afterwards is NOT enough: by then the new server is already installed
    /// and persisted, and the next call would use it (codex r4).
    async fn rediscover_same_machine(&self, machine: &str) -> Result<PlexLibrary, String> {
        self.rediscover_on(Some(machine)).await
    }

    async fn rediscover_on(&self, machine: Option<&str>) -> Result<PlexLibrary, String> {
        let lib = {
            let guard = self.lib.lock().await;
            guard.clone()
        };
        let all = lib.discover_servers().await.map_err(|e| e.to_string())?;
        let servers = same_machine_candidates(all, machine);
        let chosen = lib
            .choose_reachable_server(&servers, false)
            .await
            .ok_or_else(|| {
                if servers.is_empty() {
                    "no Plex servers found".to_string()
                } else {
                    "no reachable direct HTTPS Plex server found; check Plex Remote Access or connect to the server's network. Plex Relay is not used by default for HDR playback.".to_string()
                }
            })?;
        let updated = {
            let mut guard = self.lib.lock().await;
            guard.set_server(chosen.clone());
            guard.clone()
        };
        let (host, port, scheme) = (chosen.host.clone(), chosen.port, chosen.scheme.clone());
        if let Err(e) = crate::config::update(move |cfg| {
            cfg.last_server_host = Some(host);
            cfg.last_server_port = Some(port);
            cfg.last_server_scheme = Some(scheme);
            Ok::<(), String>(())
        }) {
            // Non-fatal for this session (the server is selected in memory), but
            // surface it so a persistent lock/permission/disk failure isn't silent.
            eprintln!(
                "plex: failed to persist rediscovered server ({e}); will rediscover next launch"
            );
        }
        Ok(updated)
    }

    fn to_item(&self, lib: &PlexLibrary, v: PlexVideo) -> ItemDto {
        ItemDto {
            // Request a grid-sized thumbnail, not the full-resolution poster.
            poster: v
                .thumb
                .as_deref()
                .and_then(|t| lib.poster_transcode_url(t, 300, 450)),
            series_poster: v
                .grandparent_thumb
                .as_deref()
                .and_then(|t| lib.poster_transcode_url(t, 300, 450)),
            // Hero art renders at window width, so request it big. Episodes
            // use their own scene still (thumb) there; other types use the
            // backdrop/fanart.
            backdrop: if v.media_type.as_deref() == Some("episode") {
                v.thumb.as_deref()
            } else {
                v.art.as_deref()
            }
            .and_then(|t| lib.poster_transcode_url(t, 1920, 1080)),
            rating_key: namespace_key(&self.id, &v.rating_key),
            title: v.title,
            year: v.year,
            summary: v.summary,
            duration_ms: v.duration,
            media_type: v.media_type,
            view_offset_ms: v.view_offset,
            // Plex omits viewCount for unwatched items, so absent == 0 == unwatched
            // (Some(false)), never "unknown" — the source always knows watched state.
            played: Some(v.view_count.unwrap_or(0) > 0),
            last_watched_at_ms: v.last_viewed_at.map(|s| s.saturating_mul(1000)),
            // Plex addedAt is epoch seconds; carry it in ms for the date-added sort.
            added_at_ms: v.added_at.map(|s| s.saturating_mul(1000)),
            index: v.index,
            parent_index: v.parent_index,
            grandparent_title: v.grandparent_title,
            parent_title: v.parent_title,
            parent_rating_key: v
                .parent_rating_key
                .as_deref()
                .map(|k| namespace_key(&self.id, k)),
            grandparent_rating_key: v
                .grandparent_rating_key
                .as_deref()
                .map(|k| namespace_key(&self.id, k)),
            source_id: self.id.clone(),
            // "imdb://tt0133093" → "imdb:tt0133093"; includes plex:// ids,
            // which are stable across Plex servers on the new agents.
            provider_ids: v
                .guids
                .iter()
                .filter_map(|g| {
                    let (scheme, rest) = g.id.split_once("://")?;
                    let rest = rest.split('?').next().unwrap_or(rest);
                    (!rest.is_empty()).then(|| format!("{}:{rest}", scheme.to_lowercase()))
                })
                .collect(),
            backing: None,
            canonical_id: None,
            watch_key: None,
            detail_key: None,
        }
    }

    /// Map a fetched `/library/metadata/{rk}` record to the frontend [`DetailDto`],
    /// building image URLs through the same tokened transcode path as posters.
    /// A namespaced person key from a Plex tag id — only when the id is the
    /// expected server-local digits form; anything else stays plain text
    /// (never a dangling or malformed key).
    fn person_key_of(&self, id: &Option<String>) -> Option<String> {
        id.as_deref()
            .filter(|s| !s.is_empty() && s.chars().all(|c| c.is_ascii_digit()))
            .map(|s| namespace_key(&self.id, s))
    }

    fn to_detail(&self, lib: &PlexLibrary, d: PlexDetail) -> DetailDto {
        let tags = |v: Vec<crate::plex_library::PlexTag>| -> Vec<String> {
            v.into_iter()
                .map(|t| t.tag)
                .filter(|s| !s.is_empty())
                .collect()
        };
        let people = |v: Vec<crate::plex_library::PlexTag>| -> Vec<PersonRef> {
            v.into_iter()
                .filter(|t| !t.tag.is_empty())
                .map(|t| PersonRef {
                    person_key: self.person_key_of(&t.id),
                    name: t.tag,
                })
                .collect()
        };
        DetailDto {
            rating_key: namespace_key(&self.id, &d.rating_key),
            poster: d
                .thumb
                .as_deref()
                .and_then(|t| lib.poster_transcode_url(t, 300, 450)),
            // Episodes use their scene still as the backdrop; other types use art.
            backdrop: if d.media_type.as_deref() == Some("episode") {
                d.thumb.as_deref()
            } else {
                d.art.as_deref()
            }
            .and_then(|t| lib.poster_transcode_url(t, 1920, 1080)),
            cast: d
                .roles
                .into_iter()
                .filter(|r| !r.tag.is_empty())
                .map(|r| CastMember {
                    person_key: self.person_key_of(&r.id),
                    name: r.tag,
                    role: r.role.filter(|s| !s.is_empty()),
                    thumb: r
                        .thumb
                        .as_deref()
                        .and_then(|t| lib.poster_transcode_url(t, 300, 300)),
                })
                .collect(),
            genres: tags(d.genres),
            directors: people(d.directors),
            writers: people(d.writers),
            countries: tags(d.countries),
            media: d
                .media
                .into_iter()
                .map(|m| MediaVersionDto {
                    hdr: m
                        .video_dynamic_range
                        .as_deref()
                        .map(is_hdr_range)
                        .unwrap_or(false),
                    streams: m
                        .parts
                        .into_iter()
                        .flat_map(|p| p.streams)
                        .map(|s| MediaStreamDto {
                            stream_type: s.stream_type,
                            codec: s.codec,
                            language: s.language,
                            channels: s.channels,
                            display_title: s.display_title,
                        })
                        .collect(),
                    video_resolution: m.video_resolution,
                    width: m.width,
                    height: m.height,
                    video_codec: m.video_codec,
                    audio_codec: m.audio_codec,
                    container: m.container,
                })
                .collect(),
            // Plex omits viewCount when unwatched; absent == 0 == unwatched
            // (always Some — the server knows, matching `to_item`).
            played: Some(d.view_count.unwrap_or(0) > 0),
            view_offset_ms: d.view_offset,
            title: d.title,
            year: d.year,
            summary: d.summary,
            tagline: d.tagline,
            duration_ms: d.duration,
            media_type: d.media_type,
            content_rating: d.content_rating,
            rating: d.rating,
            audience_rating: d.audience_rating,
            studio: d.studio,
            originally_available_at: d.originally_available_at,
            index: d.index,
            parent_index: d.parent_index,
            grandparent_title: d.grandparent_title,
            parent_title: d.parent_title,
            parent_rating_key: d
                .parent_rating_key
                .as_deref()
                .map(|k| namespace_key(&self.id, k)),
            grandparent_rating_key: d
                .grandparent_rating_key
                .as_deref()
                .map(|k| namespace_key(&self.id, k)),
            source_id: self.id.clone(),
        }
    }
}

/// True when a Plex `videoDynamicRange`/`videoProfile` string names an HDR variant
/// (mirrors the playback-side detection in `get_part_url_for_rating_key`).
fn is_hdr_range(v: &str) -> bool {
    let v = v.to_ascii_lowercase();
    v.contains("hdr")
        || v.contains("dolby")
        || v.contains("dovi")
        || v.contains("hlg")
        || v.contains("pq")
}

#[async_trait]
impl MediaSource for PlexSource {
    fn id(&self) -> String {
        self.id.clone()
    }
    fn name(&self) -> String {
        self.name.clone()
    }
    fn kind(&self) -> &'static str {
        "plex"
    }

    /// Ask the server to rescan one section for new files. The request path
    /// MUST come from [`scan_path`] — validation and endpoint shape are
    /// unit-tested there, and this is its only production call site.
    async fn scan_library(&self, section_key: &str) -> Result<(), String> {
        let path = scan_path(section_key)?;
        let lib = self.ensure_ready().await?;
        // The section key is a numeric id that is only meaningful ON THE SERVER
        // IT CAME FROM. `rediscover()` re-runs discovery and takes the first
        // REACHABLE server on the account, which on a multi-server account need
        // not be the one we just failed against — so the read paths' blind
        // rediscover-and-retry would fire a scan at a DIFFERENT server's section
        // with the same number (an unrelated library) and report success for the
        // one the user actually clicked. A scan is an authenticated server
        // action, so it retries only when the rediscovered server is provably
        // the SAME machine (codex r3).
        let before = lib.server_machine_id();
        // map_err first so the non-Send error is dropped before the next await.
        let first = lib
            .request_library_scan(&path)
            .await
            .map_err(|e| e.to_string());
        match first {
            Ok(()) => Ok(()),
            Err(first_err) => {
                // Retry only against the SAME machine. The plain rediscover()
                // would install and persist whichever account server answers
                // first, so refusing the retry afterwards would be too late:
                // this source would already be repointed, and the next scan
                // would send the user's section key to a stranger's server
                // (codex r4). Pin the machine BEFORE the choice is made.
                let Some(machine) = before.as_deref() else {
                    return Err(first_err); // no known server to pin to
                };
                let lib = match self.rediscover_same_machine(machine).await {
                    Ok(l) => l,
                    Err(_) => return Err(first_err), // that server is gone/unreachable
                };
                if !may_retry_scan_on(before.as_deref(), lib.server_machine_id().as_deref()) {
                    return Err(first_err); // belt-and-braces: never act off-machine
                }
                lib.request_library_scan(&path)
                    .await
                    .map_err(|e| e.to_string())
            }
        }
    }

    async fn sections(&self) -> Result<Vec<SectionDto>, String> {
        let lib = self.ensure_ready().await?;
        // A saved server endpoint can go stale (changed IP / plex.direct host).
        // map_err first so the non-Send error is dropped before the next await.
        let first = lib.get_library_sections().await.map_err(|e| e.to_string());
        let sections = match first {
            Ok(s) => s,
            Err(_) => {
                let lib = self.rediscover().await?;
                lib.get_library_sections()
                    .await
                    .map_err(|e| e.to_string())?
            }
        };
        Ok(sections
            .into_iter()
            // Only video libraries — skip music/photo sections so non-playable
            // items never reach the nav or get routed into mpv.
            .filter(|s| s.section_type == "movie" || s.section_type == "show")
            .map(|s| SectionDto {
                key: namespace_key(&self.id, &s.key),
                title: s.title,
                section_type: s.section_type,
                source_id: self.id.clone(),
                source_name: self.name.clone(),
                sort: None, // stamped from config by get_sections
            })
            .collect())
    }

    async fn hubs(&self) -> Result<Vec<HubDto>, String> {
        let lib = self.ensure_ready().await?;
        let first = lib.get_hubs().await.map_err(|e| e.to_string());
        let (lib, hubs) = match first {
            Ok(h) => (lib, h),
            Err(_) => {
                let lib2 = self.rediscover().await?;
                let h = lib2.get_hubs().await.map_err(|e| e.to_string())?;
                (lib2, h)
            }
        };
        let mut out: Vec<HubDto> = hubs
            .into_iter()
            .map(|h| HubDto {
                title: h.title,
                hub_identifier: h.hub_identifier,
                hub_type: h.hub_type,
                // Keep only playable video items so music/photo hubs don't reach
                // the home rails or the playback path.
                items: h
                    .items
                    .into_iter()
                    .filter(|v| is_playable_video(v.media_type.as_deref()))
                    .map(|v| self.to_item(&lib, v))
                    .collect(),
                source_id: self.id.clone(),
                source_name: self.name.clone(),
            })
            .filter(|h: &HubDto| !h.items.is_empty())
            .collect();
        // On Deck folds into the Continue Watching flow (decision 2026-07-04):
        // built from /library/onDeck because the /hubs On Deck hub is
        // server-controlled and often absent. A fetch failure degrades to no
        // hub, matching the per-hub resilience stance.
        if let Ok(deck) = lib.get_on_deck().await.map_err(|e| e.to_string()) {
            let items: Vec<_> = deck
                .into_iter()
                .filter(|v| is_playable_video(v.media_type.as_deref()))
                .map(|v| self.to_item(&lib, v))
                .collect();
            if !items.is_empty() {
                out.push(HubDto {
                    title: "On Deck".to_string(),
                    hub_identifier: "vela.ondeck".to_string(),
                    hub_type: "mixed".to_string(),
                    items,
                    source_id: self.id.clone(),
                    source_name: self.name.clone(),
                });
            }
        }
        Ok(out)
    }

    async fn items(
        &self,
        section_key: &str,
        section_type: &str,
        sort: Option<&str>,
        start: usize,
        size: usize,
    ) -> Result<Vec<ItemDto>, String> {
        validate_plex_id("section key", section_key)?;
        let lib = self.ensure_ready().await?;
        let sort_ref = Some(plex_sort_key(sort.unwrap_or("titleSort:asc")));
        let fetch = |lib: PlexLibrary| async move {
            if section_type == "movie" {
                lib.get_section_content_with_type_alpha_sorted(
                    section_key,
                    "1",
                    None,
                    sort_ref,
                    start,
                    size,
                )
                .await
            } else if section_type == "show" {
                lib.get_section_content_with_type_alpha_sorted(
                    section_key,
                    "2",
                    None,
                    sort_ref,
                    start,
                    size,
                )
                .await
            } else {
                lib.get_section_content_with_type_alpha(section_key, "", None, start, size)
                    .await
            }
            .map(|videos| (lib, videos))
            .map_err(|e| e.to_string())
        };
        let (lib, videos) = match fetch(lib).await {
            Ok(ok) => ok,
            Err(_) => {
                let lib = self.rediscover().await?;
                fetch(lib).await?
            }
        };
        Ok(videos.into_iter().map(|v| self.to_item(&lib, v)).collect())
    }

    async fn search(&self, query: &str) -> Result<Vec<ItemDto>, String> {
        let lib = self.ensure_ready().await?;
        let first = lib.search(query).await.map_err(|e| e.to_string());
        let (lib, videos) = match first {
            Ok(v) => (lib, v),
            Err(_) => {
                let lib2 = self.rediscover().await?;
                let v = lib2.search(query).await.map_err(|e| e.to_string())?;
                (lib2, v)
            }
        };
        Ok(videos.into_iter().map(|v| self.to_item(&lib, v)).collect())
    }

    async fn children(
        &self,
        item_key: &str,
        start: usize,
        size: usize,
    ) -> Result<Vec<ItemDto>, String> {
        validate_plex_id("item key", item_key)?;
        let lib = self.ensure_ready().await?;
        let fetch = |lib: PlexLibrary| async move {
            lib.fetch_children(item_key, None, start, size)
                .await
                .map(|videos| (lib, videos))
                .map_err(|e| e.to_string())
        };
        let (lib, videos) = match fetch(lib).await {
            Ok(ok) => ok,
            Err(_) => {
                let lib = self.rediscover().await?;
                fetch(lib).await?
            }
        };
        Ok(videos.into_iter().map(|v| self.to_item(&lib, v)).collect())
    }

    async fn item_detail(&self, item_key: &str) -> Result<DetailDto, String> {
        validate_plex_id("item key", item_key)?;
        let lib = self.ensure_ready().await?;
        let fetch = |lib: PlexLibrary| async move {
            lib.get_item_detail(item_key)
                .await
                .map(|d| (lib, d))
                .map_err(|e| e.to_string())
        };
        let (lib, detail) = match fetch(lib).await {
            Ok(ok) => ok,
            Err(_) => {
                let lib = self.rediscover().await?;
                fetch(lib).await?
            }
        };
        Ok(self.to_detail(&lib, detail))
    }

    async fn person_items(&self, person_key: &str, kind: &str) -> Result<Vec<ItemDto>, String> {
        validate_plex_id("person key", person_key)?;
        let filter = match kind {
            "actor" | "director" | "writer" => kind,
            _ => return Err("invalid person kind".to_string()),
        };
        let lib = self.ensure_ready().await?;
        // Section enumeration with the standard rediscover-once fallback
        // (map_err first so the non-Send error drops before the next await).
        let first = lib.get_library_sections().await.map_err(|e| e.to_string());
        let (lib, sections) = match first {
            Ok(s) => (lib, s),
            Err(_) => {
                let lib = self.rediscover().await?;
                let s = lib
                    .get_library_sections()
                    .await
                    .map_err(|e| e.to_string())?;
                (lib, s)
            }
        };
        const PAGE: usize = 200;
        let mut out = Vec::new();
        for s in sections
            .into_iter()
            .filter(|s| s.section_type == "movie" || s.section_type == "show")
        {
            let type_filter = if s.section_type == "movie" { "1" } else { "2" };
            let mut start = 0;
            loop {
                let page = lib
                    .get_section_person_filtered(
                        &s.key,
                        filter,
                        person_key,
                        type_filter,
                        start,
                        PAGE,
                    )
                    .await
                    .map_err(|e| e.to_string())?;
                let n = page.len();
                out.extend(page.into_iter().map(|v| self.to_item(&lib, v)));
                if n < PAGE {
                    break;
                }
                start += n;
            }
        }
        // Newest first, title tiebreak (owner default for person pages).
        out.sort_by(|a, b| {
            b.year
                .cmp(&a.year)
                .then_with(|| a.title.to_lowercase().cmp(&b.title.to_lowercase()))
        });
        Ok(out)
    }

    async fn resolve_stream(
        &self,
        item_key: &str,
        duration_ms: Option<u64>,
    ) -> Result<StreamResolution, String> {
        validate_plex_id("item key", item_key)?;
        let lib = self.ensure_ready().await?;
        let resolve_url = |lib: PlexLibrary| async move {
            let url = lib
                .get_part_url_for_rating_key(item_key)
                .await
                .map_err(|e| e.to_string())?
                .ok_or("no playable part found")?;
            Ok::<_, String>((lib, url))
        };
        let (lib, url) = match resolve_url(lib).await {
            Ok(ok) => ok,
            Err(_) => {
                let lib = self.rediscover().await?;
                resolve_url(lib).await?
            }
        };

        // The part URL is credential-free; the token travels as a header
        // instead — on this preflight and on mpv's own requests (threaded
        // through `StreamResolution`). See `.agents/decisions.md`, 2026-07-03.
        let stream_headers = vec![("X-Plex-Token".to_string(), lib.auth_token_clone())];

        // Preflight: a stale Plex DB entry can point at a file that no longer
        // exists, which would otherwise launch an mpv window that silently fails.
        // For split-file media the play URL is an `edl://` wrapper, so check each
        // underlying part it references — a missing segment must fail here too.
        let part_urls: Vec<String> = if url.starts_with("edl://") {
            edl_part_urls(&url)
        } else if url.starts_with("http") {
            vec![url.clone()]
        } else {
            Vec::new()
        };
        if !part_urls.is_empty() {
            // Propagate a builder failure rather than falling back to a default
            // client with no timeout (which could hang the preflight forever).
            let client = reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(8))
                .build()
                .map_err(|e| format!("couldn't initialize the HTTP client: {e}"))?;
            for u in &part_urls {
                let mut req = client.head(u);
                for (name, value) in &stream_headers {
                    req = req.header(name.as_str(), value.as_str());
                }
                let resp = req.send().await.map_err(|e| {
                    format!("couldn't reach the media server to start playback: {e}")
                })?;
                let status = resp.status();
                // 405 = the server doesn't allow HEAD here; we can't preflight, so
                // let it through (GET may still stream). Any other non-success
                // means the part won't play — fail closed with a clear message
                // rather than launching mpv to fail silently.
                if status == reqwest::StatusCode::METHOD_NOT_ALLOWED {
                    continue;
                }
                if !status.is_success() {
                    return Err(if status == reqwest::StatusCode::NOT_FOUND {
                        "File not found on the server — it may have been moved or deleted.".into()
                    } else {
                        format!(
                            "the media server rejected playback (HTTP {})",
                            status.as_u16()
                        )
                    });
                }
            }
        }

        let resume_ms = lib
            .get_resume_offset_ms(item_key)
            .await
            .ok()
            .flatten()
            .unwrap_or(0);
        let info = TrackInfo {
            server_base: lib.server_base().unwrap_or_default(),
            token: lib.auth_token_clone(),
            client_identifier: lib.client_identifier_clone(),
            rating_key: item_key.to_string(),
            key: format!("/library/metadata/{}", item_key),
            duration_ms: duration_ms.unwrap_or(0),
        };
        Ok(StreamResolution {
            url,
            resume_ms,
            progress: ProgressTarget::Plex(info),
            http_headers: stream_headers,
        })
    }

    async fn mark_played(&self, item_key: &str, played: bool) -> Result<(), String> {
        validate_plex_id("item key", item_key)?;
        let lib = self.ensure_ready().await?;
        let run = |lib: PlexLibrary| async move {
            lib.set_played(item_key, played)
                .await
                .map(|_| lib)
                .map_err(|e| e.to_string())
        };
        match run(lib).await {
            Ok(_) => Ok(()),
            Err(_) => {
                let lib = self.rediscover().await?;
                run(lib).await.map(|_| ())
            }
        }
    }

    async fn remove_from_continue(&self, item_key: &str) -> Result<(), String> {
        validate_plex_id("item key", item_key)?;
        let lib = self.ensure_ready().await?;
        // Single attempt, no rediscover: callers treat this as best-effort
        // (Vela's tombstone already guarantees the UX).
        lib.remove_from_continue_watching(item_key)
            .await
            .map_err(|e| e.to_string())
    }
}

/// Vela's sort keys are Plex-native EXCEPT the leaf-added recency sort: Plex
/// exposes it on show sections as `episode.addedAt` (the key behind Plex
/// Web's "Last Episode Date Added"). Translate at this one boundary; every
/// other key passes through verbatim.
fn plex_sort_key(sort: &str) -> &str {
    match sort {
        "episodeAddedAt:desc" => "episode.addedAt:desc",
        other => other,
    }
}

/// Extract the underlying part URLs from an mpv concat EDL (`edl://%N%url;...`),
/// using each `%len%` quote to slice exactly (URLs may contain `;`/`&`/`?`).
fn edl_part_urls(edl: &str) -> Vec<String> {
    let mut body = edl.strip_prefix("edl://").unwrap_or(edl);
    let mut urls = Vec::new();
    while let Some(rest) = body.strip_prefix('%') {
        let Some(pct) = rest.find('%') else { break };
        let Ok(len) = rest[..pct].parse::<usize>() else {
            break;
        };
        let after = &rest[pct + 1..];
        if after.len() < len {
            break;
        }
        urls.push(after[..len].to_string());
        body = after[len..].strip_prefix(';').unwrap_or(&after[len..]);
    }
    urls
}

/// Plex media types Vela can play or drill into (excludes music/photo).
fn is_playable_video(media_type: Option<&str>) -> bool {
    matches!(
        media_type,
        Some("movie" | "show" | "season" | "episode" | "clip")
    )
}

fn validate_plex_id(name: &str, value: &str) -> Result<(), String> {
    if !value.is_empty() && value.chars().all(|c| c.is_ascii_digit()) {
        Ok(())
    } else {
        Err(format!("invalid Plex {name}"))
    }
}

/// Narrow a discovery result to one machine. `None` keeps every candidate (the
/// ordinary rediscover: any reachable account server will do). `Some(id)` keeps
/// only that physical server — the filter a caller holding a server-LOCAL id
/// must apply BEFORE the choice is installed and persisted. The ONLY place the
/// candidate set is narrowed (`PlexSource::rediscover_on`).
fn same_machine_candidates(servers: Vec<PlexServer>, machine: Option<&str>) -> Vec<PlexServer> {
    match machine {
        None => servers,
        Some(id) => servers
            .into_iter()
            .filter(|s| s.machine_identifier == id)
            .collect(),
    }
}

/// May a failed scan be retried against the server we just landed on? Section
/// keys are numeric ids that mean nothing off the server they came from. A
/// final assertion behind [`same_machine_candidates`]: even if the filter were
/// ever loosened, the scan still refuses to act on a different machine.
fn may_retry_scan_on(before: Option<&str>, after: Option<&str>) -> bool {
    matches!((before, after), (Some(b), Some(a)) if b == a && !b.is_empty())
}

/// Path for a section scan ("scan library files"). The ONLY way production
/// may build this path — key validation and the endpoint shape are
/// unit-tested here, so a hostile/garbled key can't reshape the URL.
fn scan_path(key: &str) -> Result<String, String> {
    validate_plex_id("section key", key)?;
    Ok(format!("/library/sections/{key}/refresh"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plex_library::{
        PlexDetail, PlexDetailMedia, PlexDetailPart, PlexRole, PlexStream, PlexTag,
    };

    fn server_with_id(machine: &str, host: &str) -> PlexServer {
        PlexServer {
            name: machine.to_string(),
            host: host.to_string(),
            port: 32400,
            scheme: "https".to_string(),
            uri: format!("https://{host}:32400"),
            local: false,
            relay: false,
            machine_identifier: machine.to_string(),
            version: "1.0".to_string(),
        }
    }

    #[test]
    fn scan_rediscover_only_considers_the_same_machine() {
        // Two servers on one account. Discovery would hand back both, and
        // choose_reachable_server takes the first REACHABLE one — installing
        // AND persisting it. A caller holding server A's section key must never
        // let B into the candidate set: by the time a post-hoc check could
        // reject it, this source is already repointed at B (codex r4).
        let servers = vec![
            server_with_id("machine-A", "a.example"),
            server_with_id("machine-B", "b.example"),
        ];
        let pinned = same_machine_candidates(servers.clone(), Some("machine-A"));
        assert_eq!(pinned.len(), 1, "only A's server may be a candidate");
        assert_eq!(pinned[0].machine_identifier, "machine-A");

        // A machine that has vanished from the account yields NO candidate —
        // the retry then fails rather than silently landing elsewhere.
        assert!(same_machine_candidates(servers.clone(), Some("machine-Z")).is_empty());

        // Unpinned (the ordinary rediscover) keeps everything, as before.
        assert_eq!(same_machine_candidates(servers, None).len(), 2);
    }

    #[test]
    fn scan_retry_never_crosses_to_another_server() {
        // Same machine: the rediscover just re-resolved the SAME server's
        // address (the case the retry exists for — a stale saved URI).
        assert!(may_retry_scan_on(Some("machine-A"), Some("machine-A")));
        // Different machine: discovery fell through to another server on the
        // account. Its section "2" is an UNRELATED library — retrying there
        // would scan a stranger's files and report success for the one the user
        // clicked. This is the guard: making the fn return true unconditionally
        // fails right here.
        assert!(!may_retry_scan_on(Some("machine-A"), Some("machine-B")));
        // Unknown on either side is not a match.
        assert!(!may_retry_scan_on(None, Some("machine-A")));
        assert!(!may_retry_scan_on(Some("machine-A"), None));
        assert!(!may_retry_scan_on(Some(""), Some("")));
    }

    #[test]
    fn scan_path_shape_and_rejections() {
        assert_eq!(scan_path("42").unwrap(), "/library/sections/42/refresh");
        // Non-numeric ids can't reshape the endpoint path or smuggle a query.
        for bad in ["", "abc", "42/refresh", "../7", "42?x=1", "4 2"] {
            assert!(scan_path(bad).is_err(), "{bad:?} must be rejected");
        }
    }

    #[test]
    fn plex_sort_key_translates_only_the_leaf_added_sort() {
        assert_eq!(
            plex_sort_key("episodeAddedAt:desc"),
            "episode.addedAt:desc",
            "the one Vela key that isn't Plex-native must translate"
        );
        // Every other allowed key is Plex-native and passes through verbatim.
        for key in [
            "titleSort:asc",
            "year:desc",
            "addedAt:desc",
            "originallyAvailableAt:desc",
            "rating:desc",
            "lastViewedAt:desc",
        ] {
            assert_eq!(plex_sort_key(key), key, "{key} must pass through");
        }
    }

    #[test]
    fn hdr_range_detection() {
        for s in ["Dolby Vision", "HDR10", "hlg", "SMPTE ST 2084 (PQ)", "DoVi"] {
            assert!(is_hdr_range(s), "{s} should be HDR");
        }
        for s in ["SDR", "Rec. 709", ""] {
            assert!(!is_hdr_range(s), "{s} should not be HDR");
        }
    }

    #[test]
    fn to_item_namespaces_parent_and_grandparent_keys() {
        let src = PlexSource::new(
            "plexA",
            "Plex",
            PlexLibrary::new("tok".into(), "cid".into()),
        );
        let lib = PlexLibrary::new("tok".into(), "cid".into());
        let ep = PlexVideo {
            rating_key: "202".into(),
            title: "Next Up".into(),
            media_type: Some("episode".into()),
            parent_rating_key: Some("150".into()),
            grandparent_rating_key: Some("100".into()),
            ..Default::default()
        };
        let dto = src.to_item(&lib, ep);
        assert_eq!(dto.parent_rating_key.as_deref(), Some("plexA:150"));
        assert_eq!(dto.grandparent_rating_key.as_deref(), Some("plexA:100"));

        // Absent upstream keys stay absent — never a dangling "plexA:" prefix.
        let movie = PlexVideo {
            rating_key: "9".into(),
            title: "A Movie".into(),
            ..Default::default()
        };
        let dto = src.to_item(&lib, movie);
        assert_eq!(dto.parent_rating_key, None);
        assert_eq!(dto.grandparent_rating_key, None);
    }

    #[test]
    fn to_detail_maps_and_namespaces() {
        // A server-less library builds no image URLs (poster_transcode_url -> None),
        // which lets us assert the non-URL mapping deterministically.
        let src = PlexSource::new(
            "plexA",
            "Plex",
            PlexLibrary::new("tok".into(), "cid".into()),
        );
        let lib = PlexLibrary::new("tok".into(), "cid".into());
        let detail = PlexDetail {
            rating_key: "42".into(),
            title: "A Movie".into(),
            media_type: Some("movie".into()),
            view_count: Some(0),
            genres: vec![
                PlexTag {
                    tag: "Action".into(),
                    id: None,
                },
                PlexTag {
                    tag: String::new(),
                    id: None,
                }, // blank tag is dropped
            ],
            directors: vec![
                PlexTag {
                    tag: "Dir One".into(),
                    id: Some("456".into()),
                },
                PlexTag {
                    tag: "Dir NoId".into(),
                    id: None,
                },
                PlexTag {
                    tag: "Dir BadId".into(),
                    id: Some("abc".into()),
                }, // non-numeric id -> no key
            ],
            writers: vec![PlexTag {
                tag: "Writer One".into(),
                id: Some("789".into()),
            }],
            roles: vec![
                PlexRole {
                    tag: "Actor One".into(),
                    id: Some("123".into()),
                    role: Some("Hero".into()),
                    thumb: Some("/library/metadata/42/role/1".into()),
                },
                PlexRole {
                    tag: String::new(),
                    id: None,
                    role: None,
                    thumb: None,
                }, // nameless dropped
            ],
            media: vec![PlexDetailMedia {
                video_resolution: Some("1080".into()),
                video_dynamic_range: Some("Dolby Vision".into()),
                parts: vec![PlexDetailPart {
                    streams: vec![PlexStream {
                        stream_type: Some(2),
                        channels: Some(6),
                        codec: Some("eac3".into()),
                        ..Default::default()
                    }],
                }],
                ..Default::default()
            }],
            thumb: Some("/library/metadata/42/thumb/1".into()),
            parent_rating_key: Some("150".into()),
            grandparent_rating_key: Some("100".into()),
            ..Default::default()
        };

        let dto = src.to_detail(&lib, detail);

        assert_eq!(dto.rating_key, "plexA:42"); // namespaced
        assert_eq!(dto.source_id, "plexA");
        assert_eq!(dto.genres, ["Action"]); // blank filtered
        assert_eq!(dto.cast.len(), 1); // nameless filtered
        assert_eq!(dto.cast[0].name, "Actor One");
        assert_eq!(dto.cast[0].role.as_deref(), Some("Hero"));
        assert_eq!(dto.cast[0].thumb, None); // no server -> no URL
                                             // Person-browse keys: namespaced when the tag id is numeric; absent
                                             // (plain text) when the id is missing or malformed.
        assert_eq!(dto.cast[0].person_key.as_deref(), Some("plexA:123"));
        assert_eq!(dto.directors.len(), 3);
        assert_eq!(dto.directors[0].name, "Dir One");
        assert_eq!(dto.directors[0].person_key.as_deref(), Some("plexA:456"));
        assert_eq!(dto.directors[1].person_key, None);
        assert_eq!(dto.directors[2].person_key, None); // "abc" never becomes a key
        assert_eq!(dto.writers[0].person_key.as_deref(), Some("plexA:789"));
        assert_eq!(dto.poster, None); // no server -> no URL
        assert_eq!(dto.played, Some(false)); // viewCount 0
                                             // Episode parent keys are namespaced like every other key — they let
                                             // an episode opened without season context (stale hero snapshot)
                                             // upgrade to its shared season page.
        assert_eq!(dto.parent_rating_key.as_deref(), Some("plexA:150"));
        assert_eq!(dto.grandparent_rating_key.as_deref(), Some("plexA:100"));
        assert_eq!(dto.media.len(), 1);
        assert!(dto.media[0].hdr); // Dolby Vision
        assert_eq!(dto.media[0].video_resolution.as_deref(), Some("1080"));
        assert_eq!(dto.media[0].streams.len(), 1);
        assert_eq!(dto.media[0].streams[0].channels, Some(6));
        assert_eq!(dto.media[0].streams[0].codec.as_deref(), Some("eac3"));
    }
}
