//! Plex backend implementing [`MediaSource`]. Wraps [`PlexLibrary`] and owns the
//! server discovery / stale-server-rediscovery / config-persistence logic that
//! previously lived in the command handlers.

use async_trait::async_trait;
use tokio::sync::Mutex as AsyncMutex;

use super::{namespace_key, HubDto, ItemDto, MediaSource, SectionDto, StreamResolution};
use crate::playback::{ProgressTarget, TrackInfo};
use crate::plex_library::{PlexLibrary, PlexVideo};

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
        let lib = {
            let guard = self.lib.lock().await;
            guard.clone()
        };
        let servers = lib.discover_servers().await.map_err(|e| e.to_string())?;
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
            rating_key: namespace_key(&self.id, &v.rating_key),
            title: v.title,
            year: v.year,
            summary: v.summary,
            duration_ms: v.duration,
            media_type: v.media_type,
            view_offset_ms: v.view_offset,
            played: v.view_count.map(|c| c > 0),
            index: v.index,
            parent_index: v.parent_index,
            grandparent_title: v.grandparent_title,
            parent_title: v.parent_title,
            source_id: self.id.clone(),
        }
    }
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
        Ok(hubs
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
            .collect())
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
        let sort_ref = sort.or(Some("titleSort:asc"));
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
                let resp = client.head(u).send().await.map_err(|e| {
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
