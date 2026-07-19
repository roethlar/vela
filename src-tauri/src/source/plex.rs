//! Plex backend implementing [`MediaSource`]. Wraps [`PlexLibrary`] and owns the
//! server discovery / stale-server-rediscovery / config-persistence logic that
//! previously lived in the command handlers.

use async_trait::async_trait;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use tokio::sync::Mutex as AsyncMutex;

use super::{
    namespace_key, CastMember, DetailDto, HubDto, ItemDto, MediaSource, MediaStreamDto,
    MediaVersionDto, PersonRef, PlaylistDto, SectionDto, StreamResolution,
};
use crate::playback::{ProgressTarget, TrackInfo};
use crate::plex_library::{PlexDetail, PlexLibrary, PlexPlaylist, PlexServer, PlexVideo};

fn library_from_config(cfg: &crate::config::SourceConfig) -> Option<PlexLibrary> {
    if cfg.kind != "plex" || cfg.id.trim().is_empty() || cfg.name.trim().is_empty() {
        return None;
    }
    let token = cfg.access_token.clone().filter(|value| !value.is_empty())?;
    let client_identifier = cfg.device_id.clone().filter(|value| !value.is_empty())?;
    let mut lib = PlexLibrary::new(token, client_identifier);

    if !cfg.base_url.trim().is_empty() {
        let parsed = url::Url::parse(&cfg.base_url).ok();
        let endpoint = parsed.as_ref().and_then(|url| {
            if url.scheme() != "https" {
                return None;
            }
            Some((
                url.host_str()?.to_string(),
                url.port_or_known_default()?,
            ))
        });
        if let Some((host, port)) = endpoint {
            lib.set_server_manual(host, port, true, Some(cfg.name.clone()));
            if let Some(machine_identifier) = cfg
                .machine_identifier
                .clone()
                .filter(|value| !value.is_empty())
            {
                lib.set_machine_identifier(machine_identifier);
            }
        } else if cfg
            .machine_identifier
            .as_deref()
            .is_some_and(|value| !value.is_empty())
        {
            // Never discard a known physical-server pin and then rediscover
            // freely across the account. A malformed saved endpoint with a pin
            // is a broken source, not authorization to choose another machine.
            return None;
        }
    } else if cfg
        .machine_identifier
        .as_deref()
        .is_some_and(|value| !value.is_empty())
    {
        return None;
    }
    Some(lib)
}

/// Rebuild one persisted Plex source. Credentials and physical-server binding
/// live together on the source row, so restoring N rows creates N independent
/// clients without any process-global Plex singleton.
pub fn build_source(
    cfg: &crate::config::SourceConfig,
) -> Option<std::sync::Arc<dyn MediaSource>> {
    let lib = library_from_config(cfg)?;
    Some(std::sync::Arc::new(PlexSource::new(
        &cfg.id, &cfg.name, lib,
    )))
}

#[cfg(not(test))]
async fn persist_source_binding(
    source_id: &str,
    source_name: Option<&str>,
    base_url: &str,
    machine_identifier: &str,
) -> Result<(), String> {
    let source_id = source_id.to_string();
    let source_name = source_name.map(str::to_string);
    let base_url = base_url.to_string();
    let machine_identifier = machine_identifier.to_string();
    tokio::task::spawn_blocking(move || {
        crate::config::update(move |cfg| {
            update_source_binding(
                cfg,
                &source_id,
                source_name.as_deref(),
                &base_url,
                &machine_identifier,
            )
        })
    })
    .await
    .map_err(|error| format!("could not join Plex config persistence: {error}"))?
}

fn update_source_binding(
    cfg: &mut crate::config::AppConfig,
    source_id: &str,
    source_name: Option<&str>,
    base_url: &str,
    machine_identifier: &str,
) -> Result<(), String> {
    let source = cfg
        .sources
        .iter_mut()
        .find(|source| source.kind == "plex" && source.id == source_id)
        .ok_or_else(|| "persisted Plex source disappeared".to_string())?;
    if let Some(name) = source_name {
        source.name = name.to_string();
    }
    source.base_url = base_url.to_string();
    source.machine_identifier = Some(machine_identifier.to_string());
    Ok(())
}

pub struct PlexSource {
    id: String,
    name: String,
    lib: AsyncMutex<PlexLibrary>,
    /// Which BINDING of this source issued the keys it hands out — bumped when
    /// the source rebinds to a server it cannot prove is the same one
    /// ([`rebind_voids_keys`]). Section keys are server-local numbers, so every
    /// key issued under an earlier binding may now name a different library.
    ///
    /// Provenance ([`SectionDto::provenance`]) cannot carry this: it is `None`
    /// exactly when the machine is unknown, which is exactly when a rebind is
    /// possible — so a caller comparing provenance would see `None -> Some(A)`
    /// and could not tell a source that REBOUND to another server from one whose
    /// `/identity` probe merely recovered on the SAME server (codex r12).
    binding: AtomicU64,
    /// Whether a READ has already paid for an `/identity` probe. Reads make at
    /// most one attempt in the source's lifetime; the sections path keeps trying
    /// while the machine is unknown (see `ensure_ready`, codex r16).
    identity_probed: AtomicBool,
    /// Test observation of the production persistence boundary. The config
    /// writer itself is covered as a pure mutation below; this proves the live
    /// identity/discovery paths actually hand their binding to it.
    #[cfg(test)]
    persisted_binding: std::sync::Mutex<Option<(Option<String>, String, String)>>,
}

/// How hard a caller is willing to work to learn WHICH server this is.
#[derive(Clone, Copy)]
enum Probe {
    /// Reads: at most one attempt, ever. A probe that times out must not tax
    /// every click for the life of the session.
    Once,
    /// The sections path: keep trying while the machine is unknown — the keys it
    /// issues are the ones a scan will act on, and the user's Refresh is the retry.
    WhileUnknown,
}

impl PlexSource {
    pub fn new(id: impl Into<String>, name: impl Into<String>, lib: PlexLibrary) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            lib: AsyncMutex::new(lib),
            binding: AtomicU64::new(0),
            identity_probed: AtomicBool::new(false),
            #[cfg(test)]
            persisted_binding: std::sync::Mutex::new(None),
        }
    }

    async fn persist_binding(
        &self,
        source_name: Option<&str>,
        base_url: &str,
        machine_identifier: &str,
    ) -> Result<(), String> {
        #[cfg(test)]
        {
            *self
                .persisted_binding
                .lock()
                .unwrap_or_else(|error| error.into_inner()) = Some((
                source_name.map(str::to_string),
                base_url.to_string(),
                machine_identifier.to_string(),
            ));
            Ok(())
        }
        #[cfg(not(test))]
        {
            persist_source_binding(&self.id, source_name, base_url, machine_identifier).await
        }
    }

    /// A clone of the client with a server already selected, discovering one on
    /// first use. Cloned out so we don't hold the lock across network calls.
    ///
    /// A read never RE-probes `/identity`. Retrying it on every call cost five
    /// seconds — the probe's timeout — on every browse, search, detail open,
    /// playback start and watch-state edit, for as long as the probe kept timing
    /// out, which is forever for a server behind something that blackholes
    /// `/identity` while serving library routes fine. The app appeared to hang on
    /// every click (codex r16). One attempt is made; after that, reads take the
    /// server as they find it.
    async fn ensure_ready(&self) -> Result<PlexLibrary, String> {
        Ok(self.ensure_ready_inner(Probe::Once).await?.0)
    }

    /// The client to use AND the binding it is bound under, captured in the SAME
    /// critical section so the two cannot disagree.
    ///
    /// Reading them separately is a race: a caller can be holding a clone of
    /// server A while another task's failed read rediscovers and installs B,
    /// bumping the binding — and a binding read after that lands stamps A's keys
    /// with B's binding. The frontend would then take A's still-correct list for
    /// B's, evict the live root it is standing on, and offer A's library as
    /// B-current (codex r13). Every read of `binding` must be under the lib lock,
    /// paired with the clone it describes.
    ///
    /// This is the SECTIONS path, so it always retries the identity probe while
    /// the machine is unknown: the list it is about to issue carries the keys a
    /// scan will act on, and a server that could not be named cannot be scanned
    /// (r8-1). Retrying here is also what lets a transient probe failure recover —
    /// the user's own Refresh is the retry, which is exactly what the refusal
    /// message tells them to do.
    async fn ensure_ready_bound(&self) -> Result<(PlexLibrary, u64), String> {
        self.ensure_ready_inner(Probe::WhileUnknown).await
    }

    async fn ensure_ready_inner(&self, probe: Probe) -> Result<(PlexLibrary, u64), String> {
        {
            let guard = self.lib.lock().await;
            if guard.server_base().is_some() {
                let lib = guard.clone();
                let binding = self.binding.load(Ordering::SeqCst); // pairs with THIS clone
                drop(guard); // never hold the lock across the network call below
                // A legacy server restored without a persisted machine identifier
                // would leave rediscovery UNPINNED and free to repoint this source
                // at another account server — under section keys that only mean
                // anything on the original (codex r7). Learn it once and persist
                // the pin onto this source row.
                let may_probe = rediscovery_pin(lib.server_machine_id()).is_none()
                    && match probe {
                        Probe::WhileUnknown => true,
                        // First caller through takes the cost; everyone after it
                        // finds the flag set and goes straight to the server. This
                        // also collapses a concurrent storm of probes into one.
                        Probe::Once => !self.identity_probed.swap(true, Ordering::SeqCst),
                    };
                if may_probe {
                    if let Ok(id) = lib.fetch_machine_identifier().await {
                        let mut guard = self.lib.lock().await;
                        // Read the binding under THIS lock — the same critical
                        // section as the id below, so the pair describes one server
                        // (r18). A rebind may have landed while we were probing, in
                        // which case `guard` is the NEW server.
                        let current = self.binding.load(Ordering::SeqCst);
                        // This id names the server we CLONED, and nothing else. If a
                        // rebind replaced it while we probed, and the install carried
                        // no machineIdentifier of its own (discovery yields such
                        // servers — the reachability probe accepts them), the new
                        // server is UNNAMED, the `is_none()` test below passes, and
                        // the old server's name lands on it.
                        //
                        // A source that believes it is A while talking to B is worse
                        // than one that admits it cannot say. The lie PINS rediscovery
                        // to A — and a pinned install is the one case that does NOT
                        // void outstanding keys (`rebind_voids_keys`). So B's section
                        // "2", provable and unvoided, is handed to the real A: the
                        // wrong-server scan, reached through the machinery built to
                        // forbid it (codex r19). The binding is the proof of sameness,
                        // and this was the one writer that never asked for it.
                        let learned = if current == binding
                            && rediscovery_pin(guard.server_machine_id()).is_none()
                        {
                            guard.set_machine_identifier(id.clone());
                            guard.server_base().map(|base| (base, id))
                        } else {
                            None
                        };
                        let current_lib = guard.clone();
                        drop(guard);
                        if let Some((base, machine_identifier)) = learned {
                            if let Err(error) = self
                                .persist_binding(None, &base, &machine_identifier)
                                .await
                            {
                                eprintln!(
                                    "plex: failed to persist learned server identity ({error}); will probe again next launch"
                                );
                            }
                        }
                        return Ok((current_lib, current));
                    }
                }
                // No probe, or it FAILED: we return the clone we took above, so we
                // must return the binding we took with it — a rebind that landed
                // while we probed belongs to a server this clone is not.
                return Ok((lib, binding));
            }
        }
        self.rediscover_bound().await
    }

    /// Run discovery, pick a server, persist it. Recovers from a stale saved
    /// server — the SAME machine at a new address.
    ///
    /// It never silently repoints this source at a DIFFERENT machine. Discovery
    /// returns every server on the account and picks the first reachable one,
    /// then installs and persists it; but a Vela source is one server, and the
    /// ids it hands the frontend (section keys, rating keys) are server-LOCAL.
    /// Swapping the machine underneath them makes every one of those ids mean
    /// something else — a scan fired at "section 2" would scan a stranger's
    /// library and report success (codex r4/r5). So once a server is installed,
    /// rediscovery is pinned to its machine; only a source that has never
    /// connected discovers freely.
    async fn rediscover(&self) -> Result<PlexLibrary, String> {
        Ok(self.rediscover_bound().await?.0)
    }

    /// As [`Self::rediscover`], returning the installed client together with the
    /// binding it is bound under — read under the SAME lock that installs it, so
    /// a caller can never pair this server with another's binding (see
    /// [`Self::ensure_ready_bound`]).
    async fn rediscover_bound(&self) -> Result<(PlexLibrary, u64), String> {
        let (lib, pin) = {
            let guard = self.lib.lock().await;
            (guard.clone(), rediscovery_pin(guard.server_machine_id()))
        };
        let all = lib.discover_servers().await.map_err(|e| e.to_string())?;
        let servers = same_machine_candidates(all, pin.as_deref());
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
        let (updated, installed, binding) = {
            let mut guard = self.lib.lock().await;
            self.install_under_lock(&mut guard, pin.is_some(), chosen.clone())
        };
        if !installed {
            return Ok((updated, binding)); // another task got there first — use its server
        }
        let base = updated.server_base().unwrap_or_default();
        let machine_identifier = updated.server_machine_id().unwrap_or_default();
        if let Err(e) = self
            .persist_binding(Some(&chosen.name), &base, &machine_identifier)
            .await
        {
            // Non-fatal for this session (the server is selected in memory), but
            // surface it so a persistent lock/permission/disk failure isn't silent.
            eprintln!(
                "plex: failed to persist rediscovered server ({e}); will rediscover next launch"
            );
        }
        Ok((updated, binding))
    }

    /// Install the discovered server (or decline to), and bump the binding if
    /// doing so VOIDS the keys already in the frontend's hands. Returns the client
    /// to use, whether we installed it, and the binding it is bound under — all
    /// read under the caller's lock, so they describe one server.
    ///
    /// This is `rediscover_bound`'s only install, extracted so it can be TESTED:
    /// the real path reaches it only through plex.tv discovery, so the increment
    /// had no coverage at all — delete it and every Rust test still passed, while
    /// an unpinned A→B install silently kept binding 0 and the frontend accepted
    /// B's colliding section key as A's library (codex r15).
    ///
    /// Don't clobber a server another task installed while we were discovering:
    /// two UNPINNED rediscoveries (first connect) can pick DIFFERENT machines on a
    /// multi-server account, and whoever loses would otherwise overwrite the
    /// winner, leaving ids already handed to the frontend pointing at the wrong
    /// server (codex r6).
    fn install_under_lock(
        &self,
        guard: &mut PlexLibrary,
        pinned: bool,
        chosen: PlexServer,
    ) -> (PlexLibrary, bool, u64) {
        if !should_install(pinned, guard.server_machine_id().as_deref()) {
            // Someone else installed while we discovered — take their server AND
            // their binding, still under the one lock.
            return (guard.clone(), false, self.binding.load(Ordering::SeqCst));
        }
        // Pinned: provably the same machine, keys stand. Unpinned with no server
        // yet (first connect): nothing was ever issued. Unpinned OVER an existing
        // server (a restored endpoint whose /identity never answered): the new
        // server may be another account server whose section 2 is a different
        // library — every outstanding key is now a guess (codex r12).
        if rebind_voids_keys(pinned, guard.server_base().is_some()) {
            self.binding.fetch_add(1, Ordering::SeqCst);
        }
        guard.set_server(chosen);
        // Read under the install's own lock: this is the binding OF the server we
        // just installed (codex r13).
        (guard.clone(), true, self.binding.load(Ordering::SeqCst))
    }

    /// One attempt at [`MediaSource::sections`]. `Ok(None)` means the source
    /// REBOUND while the list was in flight, so what came back describes a server
    /// this source no longer is — the caller must ask again, not serve it.
    async fn sections_once(&self) -> Result<Option<Vec<SectionDto>>, String> {
        // The client AND the binding it is bound under, taken together — never
        // read apart, or a rebind landing between the two would stamp this
        // server's keys with the next server's binding (codex r13).
        let (lib, mut binding) = self.ensure_ready_bound().await?;
        // A saved server endpoint can go stale (changed IP / plex.direct host).
        // map_err first so the non-Send error is dropped before the next await.
        let first = lib.get_library_sections().await.map_err(|e| e.to_string());
        let (served_by, sections) = match first {
            Ok(s) => (lib.server_machine_id(), s),
            Err(_) => {
                // That rediscovery may have REBOUND us; this list comes from the
                // server it installed, so it carries THAT server's binding.
                let (lib, rebound) = self.rediscover_bound().await?;
                binding = rebound;
                let s = lib
                    .get_library_sections()
                    .await
                    .map_err(|e| e.to_string())?;
                // The RETRY's server served these keys — not the one we started
                // against. Recording the pre-request machine here would refuse a
                // legitimate scan of the list actually on screen.
                (lib.server_machine_id(), s)
            }
        };
        // Take the source's CURRENT machine and CURRENT binding in ONE critical
        // section, so they describe the same instant. The binding is only ever
        // bumped under this lock (`install_under_lock`), so nothing can rebind us
        // between these two reads.
        //
        // Reading them apart is how a fix for one race becomes another: the check
        // "am I still bound to the server that served this?" proves nothing about a
        // machine id fetched AFTER it. A rebind landing in that gap would stamp
        // THIS server's keys with the NEXT server's id — and a scan would then
        // compare B against B, pass, and rescan B's same-numbered library. The
        // wrong-server scan this whole subsystem exists to forbid, reintroduced by
        // its own guard (grok r18).
        let (current_machine, current_binding) = {
            let guard = self.lib.lock().await;
            (guard.server_machine_id(), self.binding.load(Ordering::SeqCst))
        };
        // Rebound while the list was in flight? Then it is a report about a server
        // we are not talking to any more — and `current_machine` is not its name.
        if current_binding != binding {
            return Ok(None);
        }
        // Stamp each key with the machine that ACTUALLY SERVED it (the
        // rediscovered one, on the retry path above). These numbers are
        // server-local, so the key alone is not enough to act on later: it
        // travels with its origin, and a scan of a key this source can no
        // longer vouch for fails closed (see `SectionDto::provenance`). When we
        // cannot name the server — a restored endpoint whose /identity probe
        // failed — every key it serves is unprovable and unscannable, by
        // design (codex r8).
        let provenance = rediscovery_pin(served_by).or_else(|| {
            // Our own clone could not name the server — but a CONCURRENT caller's
            // probe may have identified it, and the binding equality above (read
            // WITH this id, under one lock) proves it is the same server that
            // served us. Without this, two parallel loads on a restored server
            // leave the winner's keys unprovable and Scan Library refusing every
            // library until the next refresh (codex r17).
            rediscovery_pin(current_machine)
        });
        Ok(Some(
            sections
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
                    provenance: provenance.clone(),
                    binding,
                })
                .collect(),
        ))
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

    fn to_playlist(&self, playlist: PlexPlaylist) -> PlaylistDto {
        PlaylistDto {
            key: namespace_key(&self.id, &playlist.rating_key),
            title: playlist.title,
            item_count: playlist.leaf_count,
            source_id: self.id.clone(),
            source_name: self.name.clone(),
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
    async fn scan_library(&self, section_key: &str, provenance: Option<&str>) -> Result<(), String> {
        let path = scan_path(section_key)?;
        let lib = self.ensure_ready().await?;
        // A scan is an authenticated ACTION and the section key is server-LOCAL,
        // so it must reach the server the KEY CAME FROM. Naming the current
        // server is not enough: if the source drifted after the sections load
        // (an unpinned rediscover following a failed /identity probe), the key
        // would address a stranger's same-numbered library and we would report
        // success (codex r8, r9). Both sides must be known AND identical.
        //
        // The key's origin travels WITH IT (`SectionDto::provenance`), because
        // the source cannot tell from its own state which list the caller is
        // holding: an open menu, or a listing a failed refresh left on screen,
        // both outlive the note "who served the last list I returned" (codex
        // r11). Provenance the source never issued (or issued while it could
        // not name the server) is `None`, and fails closed here.
        if !scan_target_ok(
            provenance,
            rediscovery_pin(lib.server_machine_id()).as_deref(),
        ) {
            return Err(
                "can't confirm which Plex server this library belongs to — refresh libraries, or reconnect the server, and try again"
                    .to_string(),
            );
        }
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
                // rediscover() is pinned to the installed machine by
                // construction, so it can only ever come back with the SAME
                // server (at a possibly new address) or fail. The check below
                // is a belt-and-braces assertion on that invariant: a scan is
                // an authenticated ACTION, so it refuses to run anywhere but
                // the machine whose section key it holds.
                // Unknown machine (a restored endpoint whose /identity probe
                // failed): rediscovery would be UNPINNED and could install a
                // different account server, which a later scan would then hit
                // with THIS server's section key (codex r7). Don't retry.
                if rediscovery_pin(before.clone()).is_none() {
                    return Err(first_err);
                }
                let lib = match self.rediscover().await {
                    Ok(l) => l,
                    Err(_) => return Err(first_err), // that server is gone/unreachable
                };
                if !may_retry_scan_on(before.as_deref(), lib.server_machine_id().as_deref()) {
                    return Err(first_err); // never act off-machine
                }
                lib.request_library_scan(&path)
                    .await
                    .map_err(|e| e.to_string())
            }
        }
    }

    async fn sections(&self) -> Result<Vec<SectionDto>, String> {
        // Bounded, because a list from a server we are NO LONGER BOUND TO is
        // already wrong when it arrives. Our own fetch can be perfectly correct
        // and perfectly stale: another task's failed read can rediscover and
        // install a different account server while we are still talking to the
        // old one (our `/identity` probe failing is the widest window). Serving
        // that list anyway puts the OLD server's libraries in the sidebar while
        // every read behind them — items, pagination, playback — goes to the NEW
        // one: the user browses B's films under A's library name, and nothing
        // reconciles it, because the list is internally consistent (codex r14).
        // So a rebind that lands while we fetch invalidates what we fetched.
        for _ in 0..2 {
            match self.sections_once().await? {
                Some(sections) => return Ok(sections),
                None => continue, // rebound under us — ask whoever we are bound to now
            }
        }
        Err(
            "the Plex server changed while its libraries were loading — refresh libraries again"
                .to_string(),
        )
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

    async fn playlists(&self) -> Result<Vec<PlaylistDto>, String> {
        const PAGE: usize = 200;
        let fetch = |lib: PlexLibrary| async move {
            let mut start = 0;
            let mut playlists = Vec::new();
            loop {
                let page = lib
                    .get_video_playlists(start, PAGE)
                    .await
                    .map_err(|error| error.to_string())?;
                let count = page.len();
                playlists.extend(page);
                if count < PAGE {
                    return Ok::<_, String>((lib, playlists));
                }
                start += count;
            }
        };
        let lib = self.ensure_ready().await?;
        let (_lib, playlists) = match fetch(lib).await {
            Ok(result) => result,
            Err(_) => fetch(self.rediscover().await?).await?,
        };
        Ok(playlists
            .into_iter()
            .map(|playlist| self.to_playlist(playlist))
            .collect())
    }

    async fn playlist_items(&self, playlist_key: &str) -> Result<Vec<ItemDto>, String> {
        validate_plex_id("playlist key", playlist_key)?;
        const PAGE: usize = 200;
        let fetch = |lib: PlexLibrary| async move {
            let mut start = 0;
            let mut items = Vec::new();
            loop {
                let page = lib
                    .get_playlist_items(playlist_key, start, PAGE)
                    .await
                    .map_err(|error| error.to_string())?;
                let count = page.len();
                items.extend(page);
                if count < PAGE {
                    return Ok::<_, String>((lib, items));
                }
                start += count;
            }
        };
        let lib = self.ensure_ready().await?;
        let (lib, items) = match fetch(lib).await {
            Ok(result) => result,
            Err(_) => fetch(self.rediscover().await?).await?,
        };
        Ok(items
            .into_iter()
            .filter(|item| is_directly_playable_video(item.media_type.as_deref()))
            .map(|item| self.to_item(&lib, item))
            .collect())
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

fn is_directly_playable_video(media_type: Option<&str>) -> bool {
    matches!(media_type, Some("movie" | "episode" | "clip"))
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

/// Which machine rediscovery must pin to, given the installed server's id.
///
/// An EMPTY id means we do not know the machine: `set_server_manual` (the
/// startup path that restores a saved host/port, `lib.rs`) stores no machine
/// identifier. Pinning on "" would match nothing, filter out every discovery
/// candidate, and leave Plex unable to recover from a stale saved address at
/// all (codex r6). Unknown machine -> no pin -> discover freely, as before.
fn rediscovery_pin(installed: Option<String>) -> Option<String> {
    installed.filter(|m| !m.is_empty())
}

/// May this rediscovery install the server it chose? A PINNED one always may —
/// it can only have chosen the same machine. An UNPINNED one (nothing known
/// installed yet) must not overwrite a server that appeared meanwhile: two such
/// calls racing on a multi-server account can choose DIFFERENT machines, and
/// the loser would repoint the source under ids the winner already handed out
/// (codex r6). An empty installed id is "nothing known", same as None.
fn should_install(pinned: bool, installed_now: Option<&str>) -> bool {
    pinned || installed_now.is_none_or(|m| m.is_empty())
}

/// Does this install VOID the keys already issued to the frontend?
///
/// Only when the source rebinds to a server it cannot prove is the one that
/// issued them: unpinned (the machine was never identified, so discovery was
/// free to pick any account server) AND a server was already installed (so keys
/// are outstanding). A PINNED install is provably the same machine — the
/// stale-address recovery — and keeps its keys. A first connect has issued
/// none. The one case that survives is an unpinned rediscovery that happens to
/// re-find the SAME unidentified server at a new address: its keys are still
/// good, but nothing here can prove that, so they are voided anyway and the user
/// is re-rooted to Home. That is the price of an unidentifiable server, and it is
/// the safe direction (codex r12).
fn rebind_voids_keys(pinned: bool, had_server: bool) -> bool {
    !pinned && had_server
}

/// May a scan fire? The section key came from `served_by`; the request would go
/// to `current`. Both must be KNOWN and IDENTICAL — an unidentified server, or a
/// source that drifted to another machine since the section list was fetched,
/// means the key no longer addresses the library the user clicked. The ONLY
/// decision point for whether a scan runs at all (`PlexSource::scan_library`).
fn scan_target_ok(served_by: Option<&str>, current: Option<&str>) -> bool {
    matches!((served_by, current), (Some(a), Some(b)) if a == b && !a.is_empty())
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
    use crate::config::SourceConfig;
    use crate::plex_library::{
        PlexDetail, PlexDetailMedia, PlexDetailPart, PlexRole, PlexStream, PlexTag,
    };
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::sync::mpsc::{channel, Receiver, Sender};
    use std::sync::{Arc, Mutex};
    use std::time::Duration;

    fn plex_config(base_url: &str, machine_identifier: Option<&str>) -> SourceConfig {
        SourceConfig {
            id: "plex-configured".to_string(),
            kind: "plex".to_string(),
            name: "Living Room".to_string(),
            base_url: base_url.to_string(),
            access_token: Some("token".to_string()),
            api_key: None,
            user_id: None,
            device_id: Some("client".to_string()),
            machine_identifier: machine_identifier.map(str::to_string),
        }
    }

    #[test]
    fn persisted_source_restores_credentials_endpoint_and_machine_pin() {
        let cfg = plex_config("https://plex.example", Some("machine-A"));
        let lib = library_from_config(&cfg).expect("complete Plex config restores");
        assert_eq!(lib.server_base().as_deref(), Some("https://plex.example:443"));
        assert_eq!(lib.server_machine_id().as_deref(), Some("machine-A"));
        assert_eq!(lib.auth_token_clone(), "token");
        assert_eq!(lib.client_identifier_clone(), "client");

        let source = build_source(&cfg).expect("source builds");
        assert_eq!(source.id(), "plex-configured");
        assert_eq!(source.name(), "Living Room");
        assert_eq!(source.kind(), "plex");

        let invalid_pinned = plex_config("http://plex.example", Some("machine-A"));
        assert!(
            library_from_config(&invalid_pinned).is_none(),
            "a known pin may not be discarded into free account discovery"
        );
    }

    #[test]
    fn binding_persistence_updates_only_the_matching_plex_row() {
        let mut cfg = crate::config::AppConfig {
            sources: vec![
                plex_config("https://a.example", Some("machine-A")),
                SourceConfig {
                    id: "plex-other".to_string(),
                    name: "Other".to_string(),
                    base_url: "https://b.example".to_string(),
                    machine_identifier: Some("machine-B".to_string()),
                    ..plex_config("", None)
                },
            ],
            ..Default::default()
        };
        update_source_binding(
            &mut cfg,
            "plex-configured",
            Some("Renamed A"),
            "https://a-new.example:443",
            "machine-A",
        )
        .unwrap();
        assert_eq!(cfg.sources[0].name, "Renamed A");
        assert_eq!(cfg.sources[0].base_url, "https://a-new.example:443");
        assert_eq!(cfg.sources[0].machine_identifier.as_deref(), Some("machine-A"));
        assert_eq!(cfg.sources[1].name, "Other");
        assert_eq!(cfg.sources[1].base_url, "https://b.example");
        assert_eq!(cfg.sources[1].machine_identifier.as_deref(), Some("machine-B"));
    }

    #[test]
    fn playlist_descriptor_uses_rating_key_namespace_not_content_path() {
        let source = PlexSource::new(
            "plex-a",
            "Plex A",
            PlexLibrary::new("token".to_string(), "client".to_string()),
        );
        let dto = source.to_playlist(PlexPlaylist {
            rating_key: "2561805".to_string(),
            title: "Background videos".to_string(),
            leaf_count: Some(8),
        });
        assert_eq!(dto.key, "plex-a:2561805");
        assert_eq!(dto.title, "Background videos");
        assert_eq!(dto.item_count, Some(8));
        assert_eq!(dto.source_id, "plex-a");
        assert_eq!(dto.source_name, "Plex A");
    }

    /// A minimal Plex-shaped HTTP server on 127.0.0.1, enough for `/identity`,
    /// the section list, and a scan. Every request path is recorded on ARRIVAL,
    /// so a test can assert a request was never made — the point of a scan guard
    /// is the request the wrong server never receives, which a return value alone
    /// cannot prove. Both mock servers serve a section "2": that collision is the
    /// whole hazard, since a Plex section key is only a number.
    ///
    /// `identity_failures` 500s that many `/identity` probes before answering —
    /// a server that cannot be identified YET, which is the state every rebind
    /// hazard grows out of.
    /// Parks a mock handler: it announces the request's ARRIVAL, then waits to be
    /// released — so a test can make something happen to the source WHILE that
    /// request is in flight.
    type Gate = (Sender<()>, Arc<Mutex<Receiver<()>>>);

    fn spawn_mock_plex(machine: &str) -> (u16, Arc<Mutex<Vec<String>>>) {
        spawn_mock_plex_with(machine, 0, None)
    }
    fn spawn_mock_plex_with(
        machine: &str,
        identity_failures: usize,
        // Gating `/identity` specifically: it is the network call the source makes
        // AFTER cloning its client, which is where the clone and the binding can
        // drift apart.
        identity_gate: Option<Gate>,
    ) -> (u16, Arc<Mutex<Vec<String>>>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let hits: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
        let left = Arc::new(Mutex::new(identity_failures));
        let (machine, out) = (machine.to_string(), hits.clone());
        std::thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(mut stream) = stream else { continue };
                let (machine, hits, left, identity_gate) = (
                    machine.clone(),
                    out.clone(),
                    left.clone(),
                    identity_gate.clone(),
                );
                std::thread::spawn(move || {
                    let mut buf = [0u8; 2048];
                    let n = stream.read(&mut buf).unwrap_or(0);
                    if n == 0 {
                        return;
                    }
                    let req = String::from_utf8_lossy(&buf[..n]);
                    let path = req.split_whitespace().nth(1).unwrap_or("/").to_string();
                    hits.lock().unwrap().push(path.clone());
                    // DEMAND the token, as a real server does. The mock used to
                    // parse the path and nothing else, so deleting the
                    // `X-Plex-Token` header left every Plex guard green while a
                    // real server answered 401 and Scan Library became unusable
                    // (codex r16). `/identity` is the one route Plex serves
                    // unauthenticated.
                    if !path.starts_with("/identity")
                        && !req.to_ascii_lowercase().contains("x-plex-token:")
                    {
                        let _ = stream.write_all(
                            b"HTTP/1.1 401 Unauthorized\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                        );
                        return;
                    }
                    if path.starts_with("/identity") {
                        if let Some((arrived, release)) = identity_gate.as_ref() {
                            let _ = arrived.send(());
                            let _ = release.lock().unwrap().recv();
                        }
                        let mut left = left.lock().unwrap();
                        if *left > 0 {
                            *left -= 1;
                            drop(left);
                            let _ = stream.write_all(
                                b"HTTP/1.1 500 Internal Server Error\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                            );
                            return;
                        }
                    }
                    let body = if path.starts_with("/identity") {
                        format!(r#"<MediaContainer machineIdentifier="{machine}" />"#)
                    } else if path.starts_with("/library/sections/") {
                        // a scan: /library/sections/{key}/refresh
                        r#"<MediaContainer size="0" />"#.to_string()
                    } else if path.starts_with("/library/sections") {
                        // Same KEY on both servers — that collision is the whole
                        // hazard — but a DIFFERENT library behind it, so a test can
                        // tell which server actually answered. Identical bodies let
                        // a retry that merely restamped the old server's list pass
                        // (codex r15).
                        format!(
                            r#"<MediaContainer size="1"><Directory key="2" type="movie" title="{machine} Films" /></MediaContainer>"#
                        )
                    } else {
                        r#"<MediaContainer size="0" />"#.to_string()
                    };
                    let resp = format!(
                        "HTTP/1.1 200 OK\r\nContent-Type: application/xml\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                        body.len()
                    );
                    let _ = stream.write_all(resp.as_bytes());
                });
            }
        });
        (port, hits)
    }

    fn mock_server(machine: &str, port: u16) -> PlexServer {
        PlexServer {
            name: machine.to_string(),
            host: "127.0.0.1".to_string(),
            port,
            scheme: "http".to_string(),
            uri: format!("http://127.0.0.1:{port}"),
            local: true,
            relay: false,
            machine_identifier: machine.to_string(),
            version: "1".to_string(),
        }
    }

    /// A scan must reach the server the section KEY came from — and the key the
    /// caller holds need not be from the list currently on screen. A context
    /// menu opened on A's library outlives the refresh that replaces the sidebar
    /// with B's; a failed refresh leaves A's list up after the source has already
    /// moved on. No source-global "who served the last list" note can tell those
    /// keys apart, because "2" is a real section on BOTH servers — so the origin
    /// travels WITH the key (codex r10, r11).
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn a_stale_key_never_scans_the_server_that_replaced_it() {
        let (port_a, _hits_a) = spawn_mock_plex("machine-A");
        let (port_b, hits_b) = spawn_mock_plex("machine-B");

        // The sidebar the user is looking at was built from A (whose identity we
        // learn on first use: a server restored from config carries no machine id).
        let mut lib = PlexLibrary::new("token".to_string(), "client".to_string());
        lib.set_server_manual("127.0.0.1".to_string(), port_a, false, Some("A".to_string()));
        let source = PlexSource::new("plex", "Plex", lib);
        let from_a = source.sections().await.expect("A serves its sections");
        assert_eq!(from_a.len(), 1);
        assert_eq!(from_a[0].provenance.as_deref(), Some("machine-A"));

        // The source drifts to another account server (in production: an unpinned
        // rediscovery after a failed read) and B's list replaces the sidebar. The
        // menu the user opened still holds A's section.
        source
            .lib
            .lock()
            .await
            .set_server(mock_server("machine-B", port_b));
        let from_b = source.sections().await.expect("B serves its sections");
        assert_eq!(from_b[0].provenance.as_deref(), Some("machine-B"));

        // Scanning the library A gave them must not touch B.
        let err = source
            .scan_library("2", from_a[0].provenance.as_deref())
            .await
            .expect_err("a key server B never issued must not be scanned");
        assert!(err.contains("can't confirm"), "unexpected error: {err}");
        let seen = hits_b.lock().unwrap().clone();
        assert!(
            !seen.iter().any(|p| p.contains("refresh")),
            "server B was sent a scan for a key it never issued: {seen:?}"
        );

        // ...while B's OWN key still scans B: the guard refuses stale keys, not
        // every key.
        source
            .scan_library("2", from_b[0].provenance.as_deref())
            .await
            .expect("B's own key scans B");
        let seen = hits_b.lock().unwrap().clone();
        assert!(
            seen.iter().any(|p| p == "/library/sections/2/refresh"),
            "B never received the scan it should have: {seen:?}"
        );
    }

    /// The benign case that a naive fix would break. A restored server carries no
    /// machine id, so `/identity` is probed on every call until it answers. A
    /// probe that fails and LATER SUCCEEDS on the SAME server is not a rebind:
    /// nothing moved, and the keys already on screen are still that server's.
    ///
    /// This is why the frontend cannot key its "is my library still here?" check
    /// on provenance. It would see `None -> Some(machine-A)` here — identical to
    /// what it sees when the source rebinds to ANOTHER server — and would kick
    /// the user to Home on an ordinary refresh of a server that never changed
    /// (codex r12; the false positive that the `binding` split exists to avoid).
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn an_identity_probe_that_recovers_is_not_a_rebind() {
        let (port, _hits) = spawn_mock_plex_with("machine-A", 1, None); // first probe 500s

        let mut lib = PlexLibrary::new("token".to_string(), "client".to_string());
        lib.set_server_manual("127.0.0.1".to_string(), port, false, Some("A".to_string()));
        let source = PlexSource::new("plex", "Plex", lib);

        // The probe fails: the server serves its list, but cannot be named — so
        // the keys are unprovable (and unscannable, r8-1).
        let unidentified = source.sections().await.expect("A serves its sections");
        assert_eq!(unidentified[0].provenance, None);
        assert_eq!(unidentified[0].binding, 0);

        // The probe recovers. Same server, same library, same keys.
        let identified = source.sections().await.expect("A serves its sections");
        assert_eq!(identified[0].provenance.as_deref(), Some("machine-A"));
        assert_eq!(
            identified[0].binding, 0,
            "identity finally answering is not a rebind: the keys did not change hands"
        );
        assert_eq!(
            identified[0].key, unidentified[0].key,
            "...so the root the user is standing on is still the same library"
        );
        let persisted = source
            .persisted_binding
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .clone()
            .expect("the recovered identity reaches per-source persistence");
        assert_eq!(persisted.0, None, "an identity probe does not rename the source");
        assert_eq!(persisted.1, format!("http://127.0.0.1:{port}"));
        assert_eq!(persisted.2, "machine-A");
    }

    /// A list from a server we are NO LONGER BOUND TO must not be served at all.
    ///
    /// The previous guard bumped the binding but never actually installed another
    /// server, so it could not reach this state (codex r14). Here B really is
    /// installed while A's probe is parked: A's fetch then succeeds and is
    /// perfectly self-consistent — A's keys, A's binding — and is still a report
    /// about a server this source has stopped being. Serving it puts A's
    /// libraries in the sidebar while every read behind them goes to B.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn a_list_from_a_server_we_no_longer_are_is_not_served() {
        let (arrived_tx, arrived_rx) = channel();
        let (release_tx, release_rx) = channel();
        let gate = (arrived_tx, Arc::new(Mutex::new(release_rx)));
        let (port_a, _hits_a) = spawn_mock_plex_with("machine-A", 1, Some(gate));
        let (port_b, hits_b) = spawn_mock_plex("machine-B");

        let mut lib = PlexLibrary::new("token".to_string(), "client".to_string());
        lib.set_server_manual("127.0.0.1".to_string(), port_a, false, Some("A".to_string()));
        let source = Arc::new(PlexSource::new("plex", "Plex", lib));

        let listing = tokio::spawn({
            let source = source.clone();
            async move { source.sections().await }
        });
        arrived_rx
            .recv_timeout(Duration::from_secs(10))
            .expect("the identity probe reached A");

        // While A's probe is parked, another task's failed read rediscovers and
        // REBINDS this source to B — exactly what `rediscover_bound` does on an
        // unpinned install (the real path needs plex.tv, so drive the two effects
        // it has: the new server, and the binding that voids A's keys).
        {
            let mut guard = source.lib.lock().await;
            source.binding.fetch_add(1, Ordering::SeqCst);
            guard.set_server(mock_server("machine-B", port_b));
        }
        release_tx.send(()).unwrap();

        let sections = listing.await.unwrap().expect("the source serves SOME list");
        // The TITLE is the proof: provenance and binding alone could be restamped
        // onto A's completed list by a broken retry, and both mocks answer the same
        // KEY. Only B's actual library body can come from B (codex r15).
        assert_eq!(
            sections[0].title, "machine-B Films",
            "the sidebar must show the libraries of the server this source is bound to NOW — \
             serving A's list here leaves the user browsing B's films under A's library name"
        );
        assert!(
            hits_b
                .lock()
                .unwrap()
                .iter()
                .any(|p| p.starts_with("/library/sections") && !p.contains("refresh")),
            "B must actually have been ASKED for its libraries, not merely credited with A's"
        );
        assert_eq!(sections[0].provenance.as_deref(), Some("machine-B"));
        assert_eq!(sections[0].binding, 1);
    }

    /// An identity probe answers for the server it was SENT to. If a rebind lands
    /// while it is in flight, that answer must be thrown away — never written onto
    /// the server that replaced it.
    ///
    /// The replacement here carries no machine identifier, which is a server
    /// discovery really produces (the reachability probe accepts an empty id
    /// outright). So the new server is unnamed, the "do we already know who we
    /// are?" test passes, and A's name is stamped onto B.
    ///
    /// That misnaming is worse than the ignorance it replaces. It PINS rediscovery
    /// to A — and a pinned install is the one case that does NOT void outstanding
    /// keys — so B's section "2" is later handed to the real A as a provable,
    /// unvoided key: the wrong-server scan, arrived at through the machinery built
    /// to forbid it (codex r19).
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn an_identity_answer_is_never_written_onto_the_server_that_replaced_it() {
        let (arrived_tx, arrived_rx) = channel();
        let (release_tx, release_rx) = channel();
        let gate = (arrived_tx, Arc::new(Mutex::new(release_rx)));
        // The probe SUCCEEDS this time — that is the hazard. A failed probe writes
        // nothing, so the only way to stamp the wrong server is to answer correctly
        // about the right one, too late.
        let (port_a, _hits_a) = spawn_mock_plex_with("machine-A", 0, Some(gate));
        let (port_b, _hits_b) = spawn_mock_plex("machine-B");

        let mut lib = PlexLibrary::new("token".to_string(), "client".to_string());
        lib.set_server_manual("127.0.0.1".to_string(), port_a, false, Some("A".to_string()));
        let source = Arc::new(PlexSource::new("plex", "Plex", lib));

        let listing = tokio::spawn({
            let source = source.clone();
            async move { source.sections().await }
        });
        arrived_rx
            .recv_timeout(Duration::from_secs(10))
            .expect("the identity probe reached A");

        // While A's probe is parked, an unpinned rediscovery installs B — and the
        // discovery record for B carried NO machineIdentifier, so this source can no
        // longer name the server it is bound to. Going through the real install is
        // the point: it is what bumps the binding, and the binding is the proof the
        // probe's answer no longer applies.
        {
            let mut unnamed_b = mock_server("machine-B", port_b);
            unnamed_b.machine_identifier = String::new();
            let mut guard = source.lib.lock().await;
            let (_, installed, binding) = source.install_under_lock(&mut guard, false, unnamed_b);
            assert!(installed);
            assert_eq!(binding, 1, "an unpinned install over a live server voids its keys");
        }
        release_tx.send(()).unwrap();

        let sections = listing.await.unwrap().expect("the source serves SOME list");

        // The source is bound to B and cannot name it. That is the honest state, and
        // it is a SAFE one: unnameable means unscannable, and the user's next refresh
        // probes B and recovers. Answering "machine-A" here would be the unsafe one.
        assert_eq!(
            rediscovery_pin(source.lib.lock().await.server_machine_id()),
            None,
            "A's probe answered about A — writing it here names B after the server it replaced, \
             and pins rediscovery to a machine this source is not talking to"
        );
        // The corruption is visible on the key itself: B's library, stamped with A's
        // name. A scan of it compares A against A, passes, and is free to travel to
        // the real A the moment rediscovery follows the pin.
        assert_eq!(
            sections[0].title, "machine-B Films",
            "the list must come from the server we are bound to NOW"
        );
        assert_eq!(
            sections[0].provenance, None,
            "a key from a server we cannot name is unprovable — stamping B's key with A's name is \
             exactly the wrong-server scan this subsystem exists to forbid"
        );
    }

    /// The keys a source hands out are stamped with the binding in force when it
    /// served them, so a caller holding an older list can be told its root is
    /// gone.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn sections_are_stamped_with_the_binding_that_issued_them() {
        let (port, _hits) = spawn_mock_plex("machine-A");
        let mut lib = PlexLibrary::new("token".to_string(), "client".to_string());
        lib.set_server_manual("127.0.0.1".to_string(), port, false, Some("A".to_string()));
        let source = PlexSource::new("plex", "Plex", lib);

        assert_eq!(source.sections().await.unwrap()[0].binding, 0);
        // A rebind (rediscover installing a server it cannot prove is the same
        // one) bumps this; the real path needs plex.tv discovery, so drive the
        // decision directly — `rebind_voids_keys` below owns whether it happens.
        source.binding.fetch_add(1, Ordering::SeqCst);
        let after = source.sections().await.unwrap();
        assert_eq!(
            after[0].binding, 1,
            "keys issued after a rebind must not be mistakable for the ones before it"
        );
    }

    /// A list served by a server another caller has just IDENTIFIED is provable.
    ///
    /// Two loads can run on a restored, unidentified server at once. One holds a
    /// clone that cannot name it; the other's probe succeeds and writes the id into
    /// shared state. If the first stamps its list from its own stale clone, every
    /// key on it is `provenance: None` — and Scan Library refuses the whole library
    /// list until something triggers another refresh. The binding check proves we
    /// are still bound to the same server, so the id that arrived while we were
    /// fetching describes the very list we fetched (codex r17).
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn a_list_is_provable_once_anyone_has_identified_the_server() {
        let (arrived_tx, arrived_rx) = channel();
        let (release_tx, release_rx) = channel();
        let gate = (arrived_tx, Arc::new(Mutex::new(release_rx)));
        // The probe parks, then fails: THIS caller will never learn who it is.
        let (port, _hits) = spawn_mock_plex_with("machine-A", 1, Some(gate));

        let mut lib = PlexLibrary::new("token".to_string(), "client".to_string());
        lib.set_server_manual("127.0.0.1".to_string(), port, false, Some("A".to_string()));
        let source = Arc::new(PlexSource::new("plex", "Plex", lib));

        let listing = tokio::spawn({
            let source = source.clone();
            async move { source.sections().await }
        });
        arrived_rx
            .recv_timeout(Duration::from_secs(10))
            .expect("the probe reached the server");

        // A concurrent caller identifies the server while this one is parked.
        source
            .lib
            .lock()
            .await
            .set_machine_identifier("machine-A".to_string());
        release_tx.send(()).unwrap();

        let sections = listing.await.unwrap().expect("the list still lands");
        assert_eq!(
            sections[0].provenance.as_deref(),
            Some("machine-A"),
            "someone identified this server while we were fetching from it — the keys \
             it served are provable, and refusing to scan them until another refresh \
             is a refusal we cannot justify"
        );
    }

    /// A server whose `/identity` never answers must not tax every click.
    ///
    /// The probe exists so a scan can name its server. It was retried on EVERY
    /// `ensure_ready` while the machine stayed unknown — and `ensure_ready` is the
    /// front door for every read: browse, search, detail, playback, watch-state.
    /// A server that blackholes `/identity` while serving library routes fine (a
    /// reverse proxy, a firewall rule) therefore charged the probe's five-second
    /// timeout to every single action, forever. Reads now attempt it once
    /// (codex r16).
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn a_read_probes_identity_at_most_once() {
        // 99 failures: this server will never identify itself.
        let (port, hits) = spawn_mock_plex_with("machine-A", 99, None);
        let mut lib = PlexLibrary::new("token".to_string(), "client".to_string());
        lib.set_server_manual("127.0.0.1".to_string(), port, false, Some("A".to_string()));
        let source = PlexSource::new("plex", "Plex", lib);

        for _ in 0..5 {
            let _ = source.items("2", "movie", None, 0, 10).await;
        }
        let probes = hits
            .lock()
            .unwrap()
            .iter()
            .filter(|p| p.starts_with("/identity"))
            .count();
        assert_eq!(
            probes, 1,
            "five reads must not pay for five identity probes — a server that never \
             answers this would make every click in the app wait for its timeout"
        );

        // The sections path still retries: it issues the keys a scan acts on, so
        // it must keep trying to learn whose keys they are — and the user's own
        // Refresh is that retry.
        let _ = source.sections().await;
        let probes = hits
            .lock()
            .unwrap()
            .iter()
            .filter(|p| p.starts_with("/identity"))
            .count();
        assert!(
            probes > 1,
            "a refresh must still try to identify the server, or a transient failure \
             could never recover and scans would stay refused forever"
        );
    }

    /// The PRODUCTION increment, not the predicate. `rebind_voids_keys` was
    /// unit-tested, but nothing exercised the one place that CALLS it: the real
    /// path reaches it only through plex.tv discovery. Delete the increment and
    /// every other test still passed — while an unpinned A→B install kept binding
    /// 0, so the frontend accepted B's colliding section key as A's library and
    /// showed B's content under A's title, durably (codex r15).
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn the_install_that_rebinds_us_is_the_one_that_voids_the_keys() {
        let source = PlexSource::new(
            "plex",
            "Plex",
            PlexLibrary::new("token".to_string(), "client".to_string()),
        );

        // First connect: nothing has been issued, so nothing is voided.
        {
            let mut guard = source.lib.lock().await;
            let (_, installed, binding) =
                source.install_under_lock(&mut guard, false, server_with_id("machine-A", "a.example"));
            assert!(installed);
            assert_eq!(binding, 0, "a first connect voids no keys — there are none");
        }

        // Rediscovery PINNED to our own machine: provably the same server at a new
        // address. The keys it issued are still its keys.
        {
            let mut guard = source.lib.lock().await;
            let (_, installed, binding) =
                source.install_under_lock(&mut guard, true, server_with_id("machine-A", "a2.example"));
            assert!(installed);
            assert_eq!(
                binding, 0,
                "a stale-address recovery on the SAME machine must not void its own keys (r6)"
            );
        }

        // An UNPINNED install over a server we already had: the machine was never
        // identified, so discovery was free to land anywhere on the account. Every
        // key we handed out may now name a different library.
        {
            let mut guard = source.lib.lock().await;
            guard.set_machine_identifier(String::new()); // unidentifiable again
            let (_, installed, binding) =
                source.install_under_lock(&mut guard, false, server_with_id("machine-B", "b.example"));
            assert!(installed);
            assert_eq!(
                binding, 1,
                "this install may have swapped the server underneath the keys on screen: void them"
            );
        }
    }

    #[test]
    fn only_an_unprovable_rebind_voids_outstanding_keys() {
        // Pinned: discovery was filtered to our own machine, so whatever it
        // installed IS us at a new address. The keys stand (this is the stale-
        // address recovery r6 kept working).
        assert!(!rebind_voids_keys(true, true));
        // Unpinned over an existing server: the machine was never identified, so
        // discovery was free to pick any server on the account. Every key we
        // already handed out may now name a different library.
        assert!(rebind_voids_keys(false, true));
        // First connect: nothing has been issued to void.
        assert!(!rebind_voids_keys(false, false));
    }

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

    /// The startup path restores a saved host/port through `set_server_manual`,
    /// which stores NO machine identifier. Pinning rediscovery on that empty id
    /// would match nothing and leave Plex unable to recover from a stale saved
    /// address — browsing and scanning dead until the user relinks (codex r6).
    #[test]
    fn an_unknown_machine_does_not_pin_rediscovery() {
        assert_eq!(rediscovery_pin(Some(String::new())), None, "empty = unknown");
        assert_eq!(rediscovery_pin(None), None);
        assert_eq!(
            rediscovery_pin(Some("machine-A".to_string())).as_deref(),
            Some("machine-A")
        );
        // And an unknown machine must not filter the candidate set away.
        let servers = vec![
            server_with_id("machine-A", "a.example"),
            server_with_id("machine-B", "b.example"),
        ];
        assert_eq!(
            same_machine_candidates(servers, rediscovery_pin(Some(String::new())).as_deref()).len(),
            2,
            "an unknown machine discovers freely"
        );
    }

    /// Two UNPINNED rediscoveries (first connect) racing on a multi-server
    /// account can choose different machines; the loser must not overwrite the
    /// winner, or ids already handed to the frontend would refer to the wrong
    /// server (codex r6).
    #[test]
    fn an_unpinned_rediscovery_does_not_clobber_an_installed_server() {
        assert!(
            !should_install(false, Some("machine-A")),
            "another task already installed a server: keep it"
        );
        assert!(
            should_install(false, None),
            "nothing installed yet: this call installs"
        );
        assert!(
            should_install(false, Some("")),
            "a manual endpoint has no machine: still nothing known"
        );
        assert!(
            should_install(true, Some("machine-A")),
            "a pinned rediscovery can only have chosen the same machine"
        );
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

    /// The scan must reach the server the KEY came from, not merely a server we
    /// can name: a source that drifted between the sections load and the scan
    /// would otherwise fire a server-local key at a stranger's same-numbered
    /// library and report success (codex r9).
    #[test]
    fn a_scan_must_go_to_the_server_its_key_came_from() {
        assert!(scan_target_ok(Some("machine-A"), Some("machine-A")));
        // Drifted since the sections were fetched — the key means nothing here.
        assert!(!scan_target_ok(Some("machine-A"), Some("machine-B")));
        // Either side unknown: we cannot prove the key belongs to this server.
        assert!(!scan_target_ok(None, Some("machine-A")));
        assert!(!scan_target_ok(Some("machine-A"), None));
        assert!(!scan_target_ok(Some(""), Some("")));
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
