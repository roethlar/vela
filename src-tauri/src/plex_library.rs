use quick_xml::events::Event;
use quick_xml::Reader;
use reqwest::Client;
use serde::Deserialize;
use std::time::Duration;
use url::Url;

#[derive(Debug, Deserialize, Clone)]
#[allow(dead_code)] // deserialized Plex XML fields; not all are read in code
pub struct PlexServer {
    #[serde(rename = "@name")]
    pub name: String,
    #[serde(rename = "@host")]
    pub host: String,
    #[serde(rename = "@port")]
    pub port: u16,
    #[serde(rename = "@scheme")]
    pub scheme: String,
    #[serde(rename = "@uri", default)]
    pub uri: String,
    #[serde(rename = "@local", default)]
    pub local: bool,
    #[serde(rename = "@relay", default)]
    pub relay: bool,
    #[serde(rename = "@machineIdentifier")]
    pub machine_identifier: String,
    #[serde(rename = "@version")]
    pub version: String,
}

#[derive(Debug, Deserialize)]
pub struct MediaContainer {
    #[serde(rename = "Directory", default)]
    pub directories: Vec<LibrarySection>,
}

#[derive(Debug, Deserialize)]
pub struct ItemsContainer {
    #[serde(rename = "Video", default)]
    pub videos: Vec<PlexVideo>,
    #[serde(rename = "Metadata", default)]
    pub metadata: Vec<PlexVideo>,
    #[serde(rename = "Directory", default)]
    pub directories: Vec<PlexDir>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct PlexDir {
    #[serde(rename = "@key")]
    pub key: String,
    #[serde(rename = "@ratingKey")]
    pub rating_key: Option<String>,
    #[serde(rename = "@title")]
    pub title: String,
    #[serde(rename = "@type", default)]
    pub media_type: Option<String>,
    #[serde(rename = "@thumb")]
    pub thumb: Option<String>,
    #[serde(rename = "@year")]
    pub year: Option<u32>,
    #[serde(rename = "@summary")]
    pub summary: Option<String>,
    // Shows arrive as Directory rows; without these the "date added" / "last
    // played" sorts rank all Plex shows as missing-timestamp (sorting review r2).
    #[serde(rename = "@addedAt")]
    pub added_at: Option<u64>,
    #[serde(rename = "@lastViewedAt")]
    pub last_viewed_at: Option<u64>,
    /// A season Directory row's parent (its show) — the info surface's
    /// show-navigation target.
    #[serde(rename = "@parentRatingKey")]
    pub parent_rating_key: Option<String>,
}

impl From<PlexDir> for PlexVideo {
    fn from(d: PlexDir) -> Self {
        PlexVideo {
            key: d.key.clone(),
            rating_key: d.rating_key.unwrap_or(d.key),
            title: d.title,
            title_sort: None,
            summary: d.summary,
            duration: None,
            view_offset: None,
            view_count: None,
            thumb: d.thumb,
            grandparent_thumb: None,
            art: None,
            added_at: d.added_at,
            last_viewed_at: d.last_viewed_at,
            updated_at: None,
            media: vec![],
            year: d.year,
            media_type: d.media_type.or_else(|| Some("directory".to_string())),
            index: None,
            parent_index: None,
            grandparent_title: None,
            parent_title: None,
            parent_rating_key: d.parent_rating_key,
            grandparent_rating_key: None,
            guids: vec![],
        }
    }
}

/// A row on the home screen (Continue Watching, Recently Added, …).
pub struct PlexHub {
    pub title: String,
    pub hub_identifier: String,
    pub hub_type: String,
    pub items: Vec<PlexVideo>,
}

/// Minimal metadata for one server-owned video playlist. Plex has emitted
/// these rows under both `Playlist` and `Metadata` XML element names across
/// server generations, so parsing is deliberately element-name tolerant.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlexPlaylist {
    pub rating_key: String,
    pub title: String,
    pub leaf_count: Option<usize>,
}

#[derive(Debug, Deserialize, Clone)]
#[allow(dead_code)] // deserialized Plex XML fields; not all are read in code
pub struct LibrarySection {
    #[serde(rename = "@key")]
    pub key: String,
    #[serde(rename = "@title")]
    pub title: String,
    #[serde(rename = "@type")]
    pub section_type: String,
    #[serde(rename = "@agent")]
    pub agent: Option<String>,
    #[serde(rename = "@scanner")]
    pub scanner: Option<String>,
}

#[derive(Debug, Deserialize, Clone, Default)]
#[allow(dead_code)] // deserialized Plex XML fields; not all are read in code
pub struct PlexVideo {
    #[serde(rename = "@key")]
    pub key: String,
    #[serde(rename = "@ratingKey")]
    pub rating_key: String,
    #[serde(rename = "@title")]
    pub title: String,
    #[serde(rename = "@titleSort")]
    pub title_sort: Option<String>,
    #[serde(rename = "@summary")]
    pub summary: Option<String>,
    #[serde(rename = "@duration")]
    pub duration: Option<u64>,
    #[serde(rename = "@viewOffset")]
    pub view_offset: Option<u64>,
    #[serde(rename = "@viewCount", default)]
    pub view_count: Option<u64>,
    #[serde(rename = "@thumb")]
    pub thumb: Option<String>,
    #[serde(rename = "@grandparentThumb")]
    pub grandparent_thumb: Option<String>,
    #[serde(rename = "@art")]
    pub art: Option<String>,
    #[serde(rename = "@addedAt")]
    pub added_at: Option<u64>,
    /// Unix seconds of the user's last watch activity on this item.
    #[serde(rename = "@lastViewedAt")]
    pub last_viewed_at: Option<u64>,
    #[serde(rename = "@updatedAt")]
    pub updated_at: Option<u64>,
    #[serde(rename = "Media", default)]
    pub media: Vec<PlexMedia>,
    #[serde(rename = "@year")]
    pub year: Option<u32>,
    #[serde(rename = "@type", default)]
    pub media_type: Option<String>,
    #[serde(rename = "@index")]
    pub index: Option<u32>,
    #[serde(rename = "@parentIndex")]
    pub parent_index: Option<u32>,
    #[serde(rename = "@grandparentTitle")]
    pub grandparent_title: Option<String>,
    #[serde(rename = "@parentTitle")]
    pub parent_title: Option<String>,
    #[serde(rename = "@parentRatingKey")]
    pub parent_rating_key: Option<String>,
    #[serde(rename = "@grandparentRatingKey")]
    pub grandparent_rating_key: Option<String>,
    /// `<Guid id="imdb://tt…"/>` children (present in section listings when
    /// requested with `includeGuids=1`); the cross-source dedup identity.
    #[serde(rename = "Guid", default)]
    pub guids: Vec<PlexGuid>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct PlexGuid {
    #[serde(rename = "@id")]
    pub id: String,
}

#[derive(Debug, Deserialize, Clone)]
#[allow(dead_code)] // deserialized Plex XML fields; not all are read in code
pub struct PlexMedia {
    #[serde(rename = "@id")]
    pub id: String,
    #[serde(rename = "@duration")]
    pub duration: Option<u64>,
    #[serde(rename = "@bitrate")]
    pub bitrate: Option<u32>,
    #[serde(rename = "@width")]
    pub width: Option<u32>,
    #[serde(rename = "@height")]
    pub height: Option<u32>,
    #[serde(rename = "@aspectRatio")]
    pub aspect_ratio: Option<f32>,
    #[serde(rename = "@videoCodec")]
    pub video_codec: Option<String>,
    #[serde(rename = "@audioCodec")]
    pub audio_codec: Option<String>,
    #[serde(rename = "@container")]
    pub container: Option<String>,
    #[serde(rename = "Part", default)]
    pub parts: Vec<PlexPart>,
}

#[derive(Debug, Deserialize, Clone)]
#[allow(dead_code)] // deserialized Plex XML fields; not all are read in code
pub struct PlexPart {
    #[serde(rename = "@id")]
    pub id: String,
    #[serde(rename = "@key")]
    pub key: String,
    #[serde(rename = "@duration")]
    pub duration: Option<u64>,
    #[serde(rename = "@file")]
    pub file: String,
    #[serde(rename = "@size")]
    pub size: Option<u64>,
    #[serde(rename = "@container")]
    pub container: Option<String>,
}

// --- Item detail (the "more info" surface) -------------------------------------
// A dedicated serde hierarchy for the per-item `/library/metadata/{rk}` response.
// Kept separate from the listing structs above so the hot listing/playback parse
// isn't widened; serde_xml_rs maps explicitly named attributes and repeated child
// elements to fields (the same mechanism `PlexVideo.media`/`.guids` already use).

/// Root wrapper: the metadata endpoint returns the item as a `Video` (movie/
/// episode) or a `Directory` (show/season) under `MediaContainer`.
#[derive(Debug, Deserialize, Default)]
pub struct DetailContainer {
    #[serde(rename = "Video", default)]
    pub videos: Vec<PlexDetail>,
    // Some Plex endpoints label item rows `Metadata` rather than `Video`; capture
    // both, matching `ItemsContainer`, so the detail parse can't miss the item.
    #[serde(rename = "Metadata", default)]
    pub metadata: Vec<PlexDetail>,
    #[serde(rename = "Directory", default)]
    pub directories: Vec<PlexDetail>,
}

#[derive(Debug, Deserialize, Default)]
#[serde(default)]
pub struct PlexDetail {
    #[serde(rename = "@key")]
    pub key: String,
    #[serde(rename = "@ratingKey")]
    pub rating_key: String,
    #[serde(rename = "@title")]
    pub title: String,
    #[serde(rename = "@summary")]
    pub summary: Option<String>,
    #[serde(rename = "@tagline")]
    pub tagline: Option<String>,
    #[serde(rename = "@year")]
    pub year: Option<u32>,
    #[serde(rename = "@duration")]
    pub duration: Option<u64>,
    #[serde(rename = "@viewOffset")]
    pub view_offset: Option<u64>,
    #[serde(rename = "@viewCount")]
    pub view_count: Option<u64>,
    #[serde(rename = "@type")]
    pub media_type: Option<String>,
    #[serde(rename = "@thumb")]
    pub thumb: Option<String>,
    #[serde(rename = "@grandparentThumb")]
    pub grandparent_thumb: Option<String>,
    #[serde(rename = "@art")]
    pub art: Option<String>,
    #[serde(rename = "@contentRating")]
    pub content_rating: Option<String>,
    #[serde(rename = "@rating")]
    pub rating: Option<f32>,
    #[serde(rename = "@audienceRating")]
    pub audience_rating: Option<f32>,
    #[serde(rename = "@studio")]
    pub studio: Option<String>,
    #[serde(rename = "@originallyAvailableAt")]
    pub originally_available_at: Option<String>,
    #[serde(rename = "@index")]
    pub index: Option<u32>,
    #[serde(rename = "@parentIndex")]
    pub parent_index: Option<u32>,
    #[serde(rename = "@grandparentTitle")]
    pub grandparent_title: Option<String>,
    #[serde(rename = "@parentTitle")]
    pub parent_title: Option<String>,
    #[serde(rename = "@parentRatingKey")]
    pub parent_rating_key: Option<String>,
    #[serde(rename = "@grandparentRatingKey")]
    pub grandparent_rating_key: Option<String>,
    #[serde(rename = "Genre", default)]
    pub genres: Vec<PlexTag>,
    #[serde(rename = "Director", default)]
    pub directors: Vec<PlexTag>,
    #[serde(rename = "Writer", default)]
    pub writers: Vec<PlexTag>,
    #[serde(rename = "Country", default)]
    pub countries: Vec<PlexTag>,
    #[serde(rename = "Role", default)]
    pub roles: Vec<PlexRole>,
    #[serde(rename = "Media", default)]
    pub media: Vec<PlexDetailMedia>,
}

/// A simple `tag=`-bearing child (`<Genre>`, `<Director>`, `<Writer>`, `<Country>`).
#[derive(Debug, Deserialize, Default, Clone)]
#[serde(default)]
pub struct PlexTag {
    #[serde(rename = "@tag")]
    pub tag: String,
    /// Server-local numeric tag id (captured as a string; digits-validated at
    /// mapping time). On Director/Writer it keys the person-filtered listing
    /// (`?director=<id>`); Genre/Country simply ignore it.
    #[serde(rename = "@id")]
    pub id: Option<String>,
}

/// A `<Role>` (cast) child: actor `tag`, character `role`, headshot `thumb`.
#[derive(Debug, Deserialize, Default, Clone)]
#[serde(default)]
pub struct PlexRole {
    #[serde(rename = "@tag")]
    pub tag: String,
    /// Server-local numeric tag id — keys the `?actor=<id>` filtered listing.
    #[serde(rename = "@id")]
    pub id: Option<String>,
    #[serde(rename = "@role")]
    pub role: Option<String>,
    #[serde(rename = "@thumb")]
    pub thumb: Option<String>,
}

#[derive(Debug, Deserialize, Default, Clone)]
#[serde(default)]
pub struct PlexDetailMedia {
    #[serde(rename = "@videoResolution")]
    pub video_resolution: Option<String>,
    #[serde(rename = "@width")]
    pub width: Option<u32>,
    #[serde(rename = "@height")]
    pub height: Option<u32>,
    #[serde(rename = "@videoCodec")]
    pub video_codec: Option<String>,
    #[serde(rename = "@audioCodec")]
    pub audio_codec: Option<String>,
    #[serde(rename = "@container")]
    pub container: Option<String>,
    #[serde(rename = "@videoDynamicRange")]
    pub video_dynamic_range: Option<String>,
    #[serde(rename = "Part", default)]
    pub parts: Vec<PlexDetailPart>,
}

#[derive(Debug, Deserialize, Default, Clone)]
#[serde(default)]
pub struct PlexDetailPart {
    #[serde(rename = "Stream", default)]
    pub streams: Vec<PlexStream>,
}

#[derive(Debug, Deserialize, Default, Clone)]
#[serde(default)]
pub struct PlexStream {
    #[serde(rename = "@streamType")]
    pub stream_type: Option<u8>,
    #[serde(rename = "@codec")]
    pub codec: Option<String>,
    #[serde(rename = "@language")]
    pub language: Option<String>,
    #[serde(rename = "@channels")]
    pub channels: Option<u32>,
    #[serde(rename = "@displayTitle")]
    pub display_title: Option<String>,
}

#[derive(Clone)]
pub struct PlexLibrary {
    client: Client,
    server: Option<PlexServer>,
    auth_token: String,
    client_identifier: String,
}

impl PlexLibrary {
    pub fn new(auth_token: String, client_identifier: String) -> Self {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(15))
            .build()
            .expect("failed to build reqwest client");
        Self {
            client,
            server: None,
            auth_token,
            client_identifier,
        }
    }

    pub async fn discover_servers(&self) -> Result<Vec<PlexServer>, Box<dyn std::error::Error>> {
        println!("Discovering Plex servers...");

        let response = self
            .client
            .get("https://plex.tv/api/v2/resources")
            .query(&[
                ("includeHttps", "1"),
                ("includeRelay", "1"),
                ("includeIPv6", "1"),
            ])
            .header("X-Plex-Token", &self.auth_token)
            .header("X-Plex-Client-Identifier", &self.client_identifier)
            .header("X-Plex-Product", "Vela")
            .header("X-Plex-Version", "1.0.0")
            .header("X-Plex-Platform", crate::platform_name())
            .header("X-Plex-Device", crate::platform_name())
            .header("X-Plex-Device-Name", "Vela")
            .header("Accept", "application/xml")
            .send()
            .await?;

        let status = response.status();
        let body = response.text().await?;

        println!("Server discovery response - Status: {}", status);
        if !status.is_success() {
            return Err(format!("Plex API error: Status {} - Body: {}", status, body).into());
        }

        let servers: Vec<PlexServer> = self.parse_resources_stream(&body)?;

        println!("Found {} servers", servers.len());
        for server in &servers {
            println!(
                "Server: {} at {} (local={}, relay={})",
                server.name, server.uri, server.local, server.relay
            );
        }

        Ok(servers)
    }

    pub async fn choose_reachable_server(
        &self,
        servers: &[PlexServer],
        allow_relay: bool,
    ) -> Option<PlexServer> {
        for server in ordered_server_candidates(servers, allow_relay) {
            if self.server_is_reachable(&server).await {
                return Some(server);
            }
        }
        None
    }

    /// Return one reachable connection for every distinct physical server.
    ///
    /// A Plex resource can advertise several connections for the same machine.
    /// Linking needs all reachable *machines* for its picker, but only one
    /// connection per machine. Identifier-less entries are deliberately rejected:
    /// a newly linked source must be pinned before it can issue server-local keys.
    pub async fn reachable_servers_by_machine(
        &self,
        servers: &[PlexServer],
        allow_relay: bool,
    ) -> Vec<PlexServer> {
        let mut reachable = Vec::new();
        let mut selected_machines = std::collections::HashSet::new();
        for server in ordered_server_candidates(servers, allow_relay) {
            if !link_candidate_needs_probe(&server, &selected_machines) {
                continue;
            }
            if self.server_is_reachable(&server).await {
                selected_machines.insert(server.machine_identifier.clone());
                reachable.push(server);
            }
        }
        reachable
    }

    async fn server_is_reachable(&self, server: &PlexServer) -> bool {
        let base = server_origin(server);
        let resp = match self
            .client
            .get(format!("{base}/identity"))
            .header("X-Plex-Token", &self.auth_token)
            .header("X-Plex-Client-Identifier", &self.client_identifier)
            .header("Accept", "application/xml")
            .timeout(Duration::from_secs(3))
            .send()
            .await
        {
            Ok(resp) => resp,
            Err(err) => {
                eprintln!("plex: probe failed for {base}: {err}");
                return false;
            }
        };
        if !resp.status().is_success() {
            eprintln!("plex: probe for {base} returned {}", resp.status());
            return false;
        }
        if server.machine_identifier.is_empty() {
            return true;
        }
        let body = match resp.text().await {
            Ok(body) => body,
            Err(err) => {
                eprintln!("plex: probe body read failed for {base}: {err}");
                return false;
            }
        };
        server_identity_matches(server, &body)
    }

    fn parse_resources_stream(
        &self,
        xml: &str,
    ) -> Result<Vec<PlexServer>, Box<dyn std::error::Error>> {
        let mut reader = Reader::from_str(xml);
        let mut buf = Vec::new();

        let mut out: Vec<PlexServer> = Vec::new();
        // Temp holders for current <resource>
        let mut cur_name: Option<String> = None;
        let mut cur_id: Option<String> = None;
        let mut cur_ver: Option<String> = None;
        let mut cur_provides_server = false;
        let mut cur_public_addr: Option<String> = None;
        let mut pushed = false;

        loop {
            match reader.read_event_into(&mut buf) {
                Ok(Event::Start(e)) => {
                    let name = e.name().as_ref().to_owned();
                    if name.as_slice() == b"resource" || name.as_slice() == b"Resource" {
                        // reset
                        cur_name = None;
                        cur_id = None;
                        cur_ver = None;
                        cur_provides_server = false;
                        cur_public_addr = None;
                        pushed = false;
                        for attr in e.attributes().flatten() {
                            match attr.key.as_ref() {
                                b"name" => {
                                    cur_name =
                                        Some(String::from_utf8_lossy(&attr.value).into_owned())
                                }
                                b"clientIdentifier" => {
                                    cur_id = Some(String::from_utf8_lossy(&attr.value).into_owned())
                                }
                                b"productVersion" => {
                                    cur_ver =
                                        Some(String::from_utf8_lossy(&attr.value).into_owned())
                                }
                                b"provides" => {
                                    let v = String::from_utf8_lossy(&attr.value);
                                    if v.contains("server") {
                                        cur_provides_server = true;
                                    }
                                }
                                b"publicAddress" => {
                                    cur_public_addr =
                                        Some(String::from_utf8_lossy(&attr.value).into_owned())
                                }
                                _ => {}
                            }
                        }
                    } else if name.as_slice() == b"connection" || name.as_slice() == b"Connection" {
                        if !cur_provides_server { /* skip non-server resources */
                        } else {
                            if let (Some(name), Some(id)) = (cur_name.as_deref(), cur_id.as_deref())
                            {
                                if let Some(server) = server_from_connection_attrs(
                                    &e,
                                    name,
                                    id,
                                    cur_ver.as_deref().unwrap_or_default(),
                                ) {
                                    out.push(server);
                                    pushed = true;
                                }
                            }
                        }
                    }
                }
                Ok(Event::Empty(e)) => {
                    let name = e.name().as_ref().to_owned();
                    if (name.as_slice() == b"connection" || name.as_slice() == b"Connection")
                        && cur_provides_server
                    {
                        if let (Some(name), Some(id)) = (cur_name.as_deref(), cur_id.as_deref()) {
                            if let Some(server) = server_from_connection_attrs(
                                &e,
                                name,
                                id,
                                cur_ver.as_deref().unwrap_or_default(),
                            ) {
                                out.push(server);
                                pushed = true;
                            }
                        }
                    }
                }
                Ok(Event::End(e)) => {
                    let name = e.name().as_ref().to_owned();
                    if name.as_slice() == b"resource" || name.as_slice() == b"Resource" {
                        // If no connection child parsed, fallback to publicAddress if available
                        if !pushed {
                            if let (Some(name), Some(id), Some(pa)) =
                                (cur_name.clone(), cur_id.clone(), cur_public_addr.clone())
                            {
                                let scheme = "https".to_string();
                                let host = pa;
                                let port = 32400u16; // default PMS port; may be different
                                let uri = format_server_origin(&scheme, &host, port);
                                out.push(PlexServer {
                                    name,
                                    host,
                                    port,
                                    scheme,
                                    uri,
                                    local: false,
                                    relay: false,
                                    machine_identifier: id,
                                    version: cur_ver.clone().unwrap_or_default(),
                                });
                            }
                        }
                    }
                }
                Ok(Event::Eof) => break,
                Ok(_) => {}
                Err(e) => return Err(format!("XML parse error: {}", e).into()),
            }
            buf.clear();
        }
        Ok(out)
    }

    pub fn set_server(&mut self, server: PlexServer) {
        self.server = Some(server);
    }

    pub fn set_server_manual(
        &mut self,
        host: String,
        port: u16,
        https: bool,
        name: Option<String>,
    ) {
        let scheme = if https {
            "https".to_string()
        } else {
            "http".to_string()
        };
        let server = PlexServer {
            name: name.unwrap_or_else(|| host.clone()),
            host: host.clone(),
            port,
            uri: format_server_origin(&scheme, &host, port),
            scheme,
            local: false,
            relay: false,
            machine_identifier: String::new(),
            version: String::new(),
        };
        self.server = Some(server);
    }

    /// Ask the server to rescan one library section for new files. `path`
    /// must be built by `source::plex::scan_path` (validated and unit-tested
    /// there) — never hand-format it here. A non-owner token gets a 401/403,
    /// surfaced by `error_for_status`.
    pub async fn request_library_scan(&self, path: &str) -> Result<(), Box<dyn std::error::Error>> {
        let base = self.server_base().ok_or("No server selected")?;
        let url = format!("{base}{path}");
        self.client
            .get(&url)
            .header("X-Plex-Token", &self.auth_token)
            .header("X-Plex-Client-Identifier", &self.client_identifier)
            .send()
            .await?
            .error_for_status()?;
        Ok(())
    }

    /// Top-level home hubs (Continue Watching, Recently Added, On Deck, …).
    pub async fn get_hubs(&self) -> Result<Vec<PlexHub>, Box<dyn std::error::Error>> {
        let base = self.server_base().ok_or("No server selected")?;
        let url = format!("{base}/hubs?count=24");
        let resp = self
            .client
            .get(&url)
            .header("X-Plex-Token", &self.auth_token)
            .header("X-Plex-Client-Identifier", &self.client_identifier)
            .header("Accept", "application/xml")
            .send()
            .await?
            .error_for_status()?;
        let body = resp.text().await?;
        // Stream-parse the nested <Hub><Video/>…</Hub> shape so repeated mixed
        // item elements retain their server order.
        let mut reader = quick_xml::Reader::from_str(&body);
        let mut buf = Vec::new();
        let mut out: Vec<PlexHub> = Vec::new();
        let mut cur: Option<PlexHub> = None;
        loop {
            match reader.read_event_into(&mut buf) {
                Ok(Event::Start(e)) | Ok(Event::Empty(e)) => match e.name().as_ref() {
                    b"Hub" => {
                        if let Some(h) = cur.take() {
                            if !h.items.is_empty() {
                                out.push(h);
                            }
                        }
                        cur = Some(hub_from_attrs(&e));
                    }
                    b"Video" | b"Directory" | b"Metadata" => {
                        if let Some(h) = cur.as_mut() {
                            h.items.push(video_from_attrs(&e));
                        }
                    }
                    _ => {}
                },
                Ok(Event::End(e)) => {
                    if e.name().as_ref() == b"Hub" {
                        if let Some(h) = cur.take() {
                            if !h.items.is_empty() {
                                out.push(h);
                            }
                        }
                    }
                }
                Ok(Event::Eof) => break,
                Ok(_) => {}
                Err(_) => break,
            }
            buf.clear();
        }
        if let Some(h) = cur.take() {
            if !h.items.is_empty() {
                out.push(h);
            }
        }
        Ok(out)
    }

    /// `/library/onDeck` — the server's next-up/in-progress list. Vela builds
    /// its own On Deck hub from this endpoint because the `/hubs` On Deck hub
    /// is server-controlled and often absent (decision 2026-07-04: On Deck
    /// folds into the Continue Watching flow).
    pub async fn get_on_deck(&self) -> Result<Vec<PlexVideo>, Box<dyn std::error::Error>> {
        let base = self.server_base().ok_or("No server selected")?;
        let url = format!("{base}/library/onDeck");
        let resp = self
            .client
            .get(&url)
            .header("X-Plex-Token", &self.auth_token)
            .header("X-Plex-Client-Identifier", &self.client_identifier)
            .header("Accept", "application/xml")
            .send()
            .await?
            .error_for_status()?;
        let body = resp.text().await?;
        Ok(videos_from_xml(&body))
    }

    /// Video playlists visible to the current Plex user. `type=15` is Plex's
    /// playlist metadata type; `playlistType=video` excludes music playlists
    /// that Vela cannot route to its video player.
    pub async fn get_video_playlists(
        &self,
        start: usize,
        size: usize,
    ) -> Result<Vec<PlexPlaylist>, Box<dyn std::error::Error>> {
        let base = self.server_base().ok_or("No server selected")?;
        let query = vec![
            ("type", "15".to_string()),
            ("playlistType", "video".to_string()),
            ("X-Plex-Container-Start", start.to_string()),
            ("X-Plex-Container-Size", size.to_string()),
        ];
        let response = self
            .client
            .get(format!("{base}/playlists"))
            .query(&query)
            .header("X-Plex-Token", &self.auth_token)
            .header("X-Plex-Client-Identifier", &self.client_identifier)
            .header("Accept", "application/xml")
            .send()
            .await?
            .error_for_status()?;
        let body = response.text().await?;
        playlists_from_xml(&body).map_err(|error| {
            std::io::Error::new(std::io::ErrorKind::InvalidData, error).into()
        })
    }

    /// One page of a Plex playlist, preserving its server order and duplicate
    /// entries through the same mixed-element parser used by library listings.
    pub async fn get_playlist_items(
        &self,
        playlist_id: &str,
        start: usize,
        size: usize,
    ) -> Result<Vec<PlexVideo>, Box<dyn std::error::Error>> {
        let base = self.server_base().ok_or("No server selected")?;
        self.get_items(
            &format!("{base}/playlists/{playlist_id}/items"),
            &[
                ("X-Plex-Container-Start".to_string(), start.to_string()),
                ("X-Plex-Container-Size".to_string(), size.to_string()),
            ],
        )
        .await
    }

    /// Search across libraries. Returns playable/browsable results (movies,
    /// shows, seasons, episodes), de-duplicated, in the server's relevance order.
    pub async fn search(&self, query: &str) -> Result<Vec<PlexVideo>, Box<dyn std::error::Error>> {
        let base = self.server_base().ok_or("No server selected")?;
        let q: String = url::form_urlencoded::byte_serialize(query.as_bytes()).collect();
        let url = format!("{base}/hubs/search?query={q}&limit=30");
        let resp = self
            .client
            .get(&url)
            .header("X-Plex-Token", &self.auth_token)
            .header("X-Plex-Client-Identifier", &self.client_identifier)
            .header("Accept", "application/xml")
            .send()
            .await?
            .error_for_status()?;
        let body = resp.text().await?;
        let mut reader = quick_xml::Reader::from_str(&body);
        let mut buf = Vec::new();
        let mut items: Vec<PlexVideo> = Vec::new();
        let mut seen = std::collections::HashSet::new();
        loop {
            match reader.read_event_into(&mut buf) {
                Ok(Event::Start(e)) | Ok(Event::Empty(e)) => {
                    if matches!(e.name().as_ref(), b"Video" | b"Directory" | b"Metadata") {
                        let v = video_from_attrs(&e);
                        let kind = v.media_type.as_deref().unwrap_or("");
                        let playable = matches!(kind, "movie" | "show" | "season" | "episode");
                        if playable && !v.rating_key.is_empty() && seen.insert(v.rating_key.clone())
                        {
                            items.push(v);
                        }
                    }
                }
                Ok(Event::Eof) => break,
                Ok(_) => {}
                Err(_) => break,
            }
            buf.clear();
        }
        Ok(items)
    }

    pub async fn get_library_sections(
        &self,
    ) -> Result<Vec<LibrarySection>, Box<dyn std::error::Error>> {
        let base = self.server_base().ok_or("No server selected")?;
        let url = format!("{base}/library/sections");

        let response = self
            .client
            .get(&url)
            .header("X-Plex-Token", &self.auth_token)
            .header("X-Plex-Client-Identifier", &self.client_identifier)
            .header("Accept", "application/xml")
            .send()
            .await?
            .error_for_status()?;

        let body = response.text().await?;
        let container: MediaContainer = serde_xml_rs::from_str(&body)?;
        Ok(container.directories)
    }

    /// Fetch a list of items, preserving the server's XML ordering (serde alone
    /// doesn't guarantee order across Video/Metadata/Directory element types).
    async fn get_items(
        &self,
        base_url: &str,
        params: &[(String, String)],
    ) -> Result<Vec<PlexVideo>, Box<dyn std::error::Error>> {
        let mut url = Url::parse(base_url)?;
        url.query_pairs_mut().extend_pairs(params);

        let response = self
            .client
            .get(url)
            .header("X-Plex-Token", &self.auth_token)
            .header("X-Plex-Client-Identifier", &self.client_identifier)
            .header("Accept", "application/xml")
            .send()
            .await?
            .error_for_status()?;

        let body = response.text().await?;

        // Pass 1: capture the order of top-level entries as they appear in XML
        let mut order_keys: Vec<String> = Vec::new();
        {
            let mut rdr = quick_xml::Reader::from_str(&body);
            // 0.41 moved the reader's knobs behind `config_mut()`. This one is not
            // load-bearing — the loop below reads only Start/Empty events and pulls
            // ATTRIBUTES off them, never a text node — but keep it, so the parse is
            // configured exactly as it was rather than silently differently.
            rdr.config_mut().trim_text(true);
            let mut buf = Vec::new();
            loop {
                match rdr.read_event_into(&mut buf) {
                    Ok(quick_xml::events::Event::Empty(e))
                    | Ok(quick_xml::events::Event::Start(e)) => {
                        let name = e.name().as_ref().to_owned();
                        if matches!(name.as_slice(), b"Video" | b"Metadata" | b"Directory") {
                            let mut rk: Option<String> = None;
                            let mut k: Option<String> = None;
                            for a in e.attributes().flatten() {
                                match a.key.as_ref() {
                                    b"ratingKey" => {
                                        rk = Some(String::from_utf8_lossy(&a.value).into_owned())
                                    }
                                    b"key" => {
                                        k = Some(String::from_utf8_lossy(&a.value).into_owned())
                                    }
                                    _ => {}
                                }
                            }
                            // Prefer ratingKey; fall back to key.
                            if let Some(id) = rk.or(k) {
                                order_keys.push(id);
                            }
                        }
                    }
                    Ok(quick_xml::events::Event::Eof) => break,
                    Ok(_) => {}
                    Err(_) => break,
                }
                buf.clear();
            }
        }

        // Pass 2: parse into structs
        let container: ItemsContainer = serde_xml_rs::from_str(&body)?;

        let mut videos = container.videos;
        let mut metas = container.metadata;
        let mut dirs: Vec<PlexVideo> = container.directories.into_iter().map(Into::into).collect();

        // Build lookups by rating_key and by key
        let mut out: Vec<PlexVideo> = Vec::with_capacity(videos.len() + metas.len() + dirs.len());
        for id in order_keys {
            // search videos
            if let Some(pos) = videos
                .iter()
                .position(|v| v.rating_key == id || v.key == id)
            {
                out.push(videos.remove(pos));
                continue;
            }
            if let Some(pos) = metas.iter().position(|v| v.rating_key == id || v.key == id) {
                out.push(metas.remove(pos));
                continue;
            }
            if let Some(pos) = dirs.iter().position(|v| v.rating_key == id || v.key == id) {
                out.push(dirs.remove(pos));
                continue;
            }
        }

        // Append any remaining items (shouldn't happen if IDs matched, but safe fallback)
        out.extend(videos);
        out.extend(metas);
        out.extend(dirs);
        Ok(out)
    }

    pub fn poster_transcode_url(
        &self,
        thumb_path: &str,
        width: u32,
        height: u32,
    ) -> Option<String> {
        let base = self.server_base()?;
        // Pass the thumb to the transcoder as the SERVER-RELATIVE path (don't
        // prepend the external origin). Plex then resolves it against itself
        // locally. Prepending the public origin (plex.direct / IPv6 / HTTPS) made
        // the server try to fetch its own public URL through the transcoder, which
        // 500s; the relative form is what the official clients use and returns a
        // properly sized image. A thumb that's already absolute is passed as-is.
        let encoded: String = url::form_urlencoded::byte_serialize(thumb_path.as_bytes()).collect();
        let url = format!(
            "{}/photo/:/transcode?width={}&height={}&minSize=1&upscale=1&url={}&X-Plex-Token={}",
            base, width, height, encoded, self.auth_token
        );
        Some(url)
    }

    /// Mark an item watched (`scrobble`) or unwatched (`unscrobble`) on the server.
    pub async fn set_played(
        &self,
        rating_key: &str,
        played: bool,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let base = self.server_base().ok_or("No server selected")?;
        let action = if played { "scrobble" } else { "unscrobble" };
        let url =
            format!("{base}/:/{action}?identifier=com.plexapp.plugins.library&key={rating_key}");
        self.client
            .get(&url)
            .header("X-Plex-Token", &self.auth_token)
            .header("X-Plex-Client-Identifier", &self.client_identifier)
            .header("Accept", "application/xml")
            .send()
            .await?
            .error_for_status()?;
        Ok(())
    }

    /// Fetch the full metadata record for one item (the detail / info surface):
    /// `/library/metadata/{rk}`, which carries cast/crew/genre/media-streams that
    /// the section listing omits. Parsed with serde into [`PlexDetail`].
    pub async fn get_item_detail(
        &self,
        rating_key: &str,
    ) -> Result<PlexDetail, Box<dyn std::error::Error>> {
        let base = self.server_base().ok_or("No server selected")?;
        let url = format!("{base}/library/metadata/{rating_key}");
        let body = self
            .client
            .get(&url)
            .header("X-Plex-Token", &self.auth_token)
            .header("X-Plex-Client-Identifier", &self.client_identifier)
            .header("Accept", "application/xml")
            .send()
            .await?
            .error_for_status()?
            .text()
            .await?;
        let container: DetailContainer = serde_xml_rs::from_str(&body)?;
        container
            .videos
            .into_iter()
            .chain(container.metadata)
            .chain(container.directories)
            .next()
            .ok_or_else(|| "item not found".into())
    }

    /// Remove an item from the server's Continue Watching hub (the same
    /// action Plex Web's "Remove from Continue Watching" performs).
    pub async fn remove_from_continue_watching(
        &self,
        rating_key: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let base = self.server_base().ok_or("No server selected")?;
        let url = format!("{base}/actions/removeFromContinueWatching");
        self.client
            .put(&url)
            .query(&[("ratingKey", rating_key)])
            .header("X-Plex-Token", &self.auth_token)
            .header("X-Plex-Client-Identifier", &self.client_identifier)
            .header("Accept", "application/xml")
            .send()
            .await?
            .error_for_status()?;
        Ok(())
    }

    pub async fn fetch_children(
        &self,
        parent_rating_key: &str,
        type_filter: Option<&str>,
        start: usize,
        size: usize,
    ) -> Result<Vec<PlexVideo>, Box<dyn std::error::Error>> {
        let base = self.server_base().ok_or("No server selected")?;
        let base_url = format!("{base}/library/metadata/{parent_rating_key}/children");

        let mut params = vec![
            ("X-Plex-Container-Start".to_string(), start.to_string()),
            ("X-Plex-Container-Size".to_string(), size.to_string()),
        ];

        if let Some(t) = type_filter {
            params.push(("type".to_string(), t.to_string()));
        }

        self.get_items(&base_url, &params).await
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn get_section_content_with_type_alpha_sorted(
        &self,
        section_key: &str,
        type_filter: &str,
        first_char: Option<char>,
        sort: Option<&str>,
        start: usize,
        size: usize,
    ) -> Result<Vec<PlexVideo>, Box<dyn std::error::Error>> {
        let base = self.server_base().ok_or("No server selected")?;
        let base_url = format!("{base}/library/sections/{section_key}/all");

        let mut params = vec![
            ("includeGuids".to_string(), "1".to_string()),
            ("type".to_string(), type_filter.to_string()),
            ("X-Plex-Container-Start".to_string(), start.to_string()),
            ("X-Plex-Container-Size".to_string(), size.to_string()),
        ];

        let first_char_str = first_char.map(|c| c.to_string());
        if let Some(c) = &first_char_str {
            params.push(("firstCharacter".to_string(), c.clone()));
            params.push(("titleStartsWith".to_string(), c.clone()));
        }

        if let Some(s) = sort {
            params.push(("sort".to_string(), s.to_string()));
        }

        self.get_items(&base_url, &params).await
    }

    /// One page of a section filtered by a person tag id (`actor=` /
    /// `director=` / `writer=`), explicit item type like the other listing
    /// fetches. The URL/params come from the pure `person_filter_query` so
    /// the construction is unit-testable.
    pub async fn get_section_person_filtered(
        &self,
        section_key: &str,
        filter: &str,
        tag_id: &str,
        type_filter: &str,
        start: usize,
        size: usize,
    ) -> Result<Vec<PlexVideo>, Box<dyn std::error::Error>> {
        let base = self.server_base().ok_or("No server selected")?;
        let (url, params) =
            person_filter_query(&base, section_key, filter, tag_id, type_filter, start, size);
        self.get_items(&url, &params).await
    }

    pub async fn get_section_content_with_type_alpha(
        &self,
        section_key: &str,
        type_filter: &str,
        first_char: Option<char>,
        start: usize,
        size: usize,
    ) -> Result<Vec<PlexVideo>, Box<dyn std::error::Error>> {
        let base = self.server_base().ok_or("No server selected")?;
        let base_url = format!("{base}/library/sections/{section_key}/all");

        let mut params = vec![
            ("includeGuids".to_string(), "1".to_string()),
            ("type".to_string(), type_filter.to_string()),
            ("X-Plex-Container-Start".to_string(), start.to_string()),
            ("X-Plex-Container-Size".to_string(), size.to_string()),
        ];

        let first_char_str = first_char.map(|c| c.to_string());
        if let Some(c) = &first_char_str {
            params.push(("firstCharacter".to_string(), c.clone()));
        }

        self.get_items(&base_url, &params).await
    }

    pub async fn get_part_url_for_rating_key(
        &self,
        rating_key: &str,
    ) -> Result<Option<String>, Box<dyn std::error::Error>> {
        let base = match self.server_base() {
            Some(base) => base,
            None => return Ok(None),
        };
        let url = format!("{base}/library/metadata/{rating_key}?includeAllLeaves=1");
        let resp = self
            .client
            .get(&url)
            .header("X-Plex-Token", &self.auth_token)
            .header("X-Plex-Client-Identifier", &self.client_identifier)
            .header("Accept", "application/xml")
            .send()
            .await?
            .error_for_status()?;
        let body = resp.text().await?;
        let mut reader = quick_xml::Reader::from_str(&body);
        let mut buf = Vec::new();
        // Score each Media version and pick the best part: prefer HDR, then higher
        // resolution, then higher bitrate. (height, bitrate, is_hdr) of the current Media.
        let mut cur_hdr = false;
        let mut cur_height: u32 = 0;
        let mut cur_bitrate: u32 = 0;
        // All parts of the current Media (split-file media has more than one).
        let mut cur_parts: Vec<String> = Vec::new();
        // (is_hdr, height, bitrate, parts) per Media version.
        let mut candidates: Vec<(bool, u32, u32, Vec<String>)> = Vec::new();
        loop {
            match reader.read_event_into(&mut buf) {
                Ok(Event::Start(e)) | Ok(Event::Empty(e)) => {
                    let name = e.name().as_ref().to_owned();
                    if name.as_slice() == b"Media" || name.as_slice() == b"media" {
                        // Flush the previous Media's parts before starting a new one.
                        if !cur_parts.is_empty() {
                            candidates.push((
                                cur_hdr,
                                cur_height,
                                cur_bitrate,
                                std::mem::take(&mut cur_parts),
                            ));
                        }
                        cur_hdr = false;
                        cur_height = 0;
                        cur_bitrate = 0;
                        for a in e.attributes().flatten() {
                            match a.key.as_ref() {
                                b"height" => cur_height = av(&a).parse().unwrap_or(0),
                                b"bitrate" => cur_bitrate = av(&a).parse().unwrap_or(0),
                                b"videoDynamicRange" | b"videoProfile" => {
                                    let v = av(&a).to_ascii_lowercase();
                                    if v.contains("hdr")
                                        || v.contains("dolby")
                                        || v.contains("dovi")
                                        || v.contains("hlg")
                                        || v.contains("pq")
                                    {
                                        cur_hdr = true;
                                    }
                                }
                                _ => {}
                            }
                        }
                    } else if name.as_slice() == b"Part" || name.as_slice() == b"part" {
                        for a in e.attributes().flatten() {
                            if a.key.as_ref() == b"key" {
                                cur_parts.push(part_url(&base, &av(&a)));
                                break;
                            }
                        }
                    }
                }
                Ok(Event::Eof) => {
                    if !cur_parts.is_empty() {
                        candidates.push((
                            cur_hdr,
                            cur_height,
                            cur_bitrate,
                            std::mem::take(&mut cur_parts),
                        ));
                    }
                    break;
                }
                Ok(_) => {}
                Err(e) => {
                    eprintln!("Metadata XML parse error: {}", e);
                    break;
                }
            }
            buf.clear();
        }
        let best = candidates
            .into_iter()
            .max_by_key(|(hdr, height, bitrate, _)| (*hdr, *height, *bitrate))
            .map(|(_, _, _, parts)| parts);
        Ok(best.map(|parts| {
            if parts.len() == 1 {
                parts.into_iter().next().unwrap_or_default()
            } else {
                // Split-file media: concatenate the parts with an mpv EDL so the
                // whole title plays, not just the first segment. Each URL is
                // length-quoted (%N%) since the URLs contain ; & ? characters.
                let mut edl = String::from("edl://");
                for (i, p) in parts.iter().enumerate() {
                    if i > 0 {
                        edl.push(';');
                    }
                    edl.push_str(&format!("%{}%{}", p.len(), p));
                }
                edl
            }
        }))
    }

    pub fn server_base(&self) -> Option<String> {
        let s = self.server.as_ref()?;
        Some(server_origin(s))
    }

    /// Which physical server this handle is pointed at. Section keys are
    /// server-LOCAL numeric ids, so any caller that reuses a key across a
    /// rediscover must check this first: discovery picks the first reachable
    /// server on the account, which need not be the one the key came from.
    pub fn server_machine_id(&self) -> Option<String> {
        Some(self.server.as_ref()?.machine_identifier.clone())
    }

    /// Ask the installed server who it is. A server restored from config
    /// (`set_server_manual`) carries NO machine identifier, which leaves this
    /// source unable to pin rediscovery — and an unpinned rediscovery can
    /// silently repoint it at another account server, under section keys that
    /// only mean anything on the original (codex r7). One `/identity` call at
    /// first contact removes that whole class.
    pub async fn fetch_machine_identifier(&self) -> Result<String, String> {
        let base = self.server_base().ok_or("no server selected")?;
        let resp = self
            .client
            .get(format!("{base}/identity"))
            .header("X-Plex-Token", &self.auth_token)
            .header("X-Plex-Client-Identifier", &self.client_identifier)
            .header("Accept", "application/xml")
            .timeout(Duration::from_secs(5))
            .send()
            .await
            .map_err(|e| e.to_string())?;
        let body = resp.text().await.map_err(|e| e.to_string())?;
        identity_machine_identifier(&body).ok_or_else(|| "no machine identifier".to_string())
    }

    pub fn set_machine_identifier(&mut self, id: String) {
        if let Some(s) = self.server.as_mut() {
            s.machine_identifier = id;
        }
    }

    pub fn auth_token_clone(&self) -> String {
        self.auth_token.clone()
    }
    pub fn client_identifier_clone(&self) -> String {
        self.client_identifier.clone()
    }

    pub async fn get_resume_offset_ms(
        &self,
        rating_key: &str,
    ) -> Result<Option<u64>, Box<dyn std::error::Error>> {
        let base = match self.server_base() {
            Some(base) => base,
            None => return Ok(None),
        };
        let url = format!("{base}/library/metadata/{rating_key}?includeAllLeaves=1");
        let resp = self
            .client
            .get(&url)
            .header("X-Plex-Token", &self.auth_token)
            .header("X-Plex-Client-Identifier", &self.client_identifier)
            .header("Accept", "application/xml")
            .send()
            .await?
            .error_for_status()?;
        let body = resp.text().await?;
        let mut reader = quick_xml::Reader::from_str(&body);
        let mut buf = Vec::new();
        let mut candidate: Option<u64> = None;
        loop {
            match reader.read_event_into(&mut buf) {
                Ok(quick_xml::events::Event::Empty(e)) | Ok(quick_xml::events::Event::Start(e)) => {
                    let name = e.name().as_ref().to_owned();
                    if name.as_slice() == b"Video" || name.as_slice() == b"Metadata" {
                        let mut rk: Option<String> = None;
                        let mut vo: Option<u64> = None;
                        for a in e.attributes().flatten() {
                            match a.key.as_ref() {
                                b"ratingKey" => {
                                    rk = Some(String::from_utf8_lossy(&a.value).into_owned())
                                }
                                b"viewOffset" => {
                                    if let Ok(v) = String::from_utf8(a.value.to_vec())
                                        .unwrap_or_default()
                                        .parse::<u64>()
                                    {
                                        vo = Some(v);
                                    }
                                }
                                _ => {}
                            }
                        }
                        if let (Some(rk_val), Some(v)) = (rk, vo) {
                            if rk_val == rating_key {
                                return Ok(Some(v));
                            }
                            if candidate.is_none() {
                                candidate = Some(v);
                            }
                        }
                    }
                }
                Ok(quick_xml::events::Event::Eof) => break,
                Ok(_) => {}
                Err(_) => break,
            }
            buf.clear();
        }
        Ok(candidate)
    }
}

fn server_from_connection_attrs(
    e: &quick_xml::events::BytesStart,
    name: &str,
    machine_identifier: &str,
    version: &str,
) -> Option<PlexServer> {
    let mut uri: Option<String> = None;
    let mut local = false;
    let mut relay = false;

    for attr in e.attributes().flatten() {
        match attr.key.as_ref() {
            b"uri" => uri = Some(av(&attr)),
            b"local" => local = attr_bool(&av(&attr)),
            b"relay" => relay = attr_bool(&av(&attr)),
            _ => {}
        }
    }

    let url = Url::parse(uri.as_deref()?).ok()?;
    let scheme = url.scheme().to_string();
    let host = url.host_str().unwrap_or("localhost").to_string();
    let port = url.port_or_known_default().unwrap_or(32400);
    Some(PlexServer {
        name: name.to_string(),
        host,
        port,
        scheme,
        uri: origin_from_url(&url),
        local,
        relay,
        machine_identifier: machine_identifier.to_string(),
        version: version.to_string(),
    })
}

fn ordered_server_candidates(servers: &[PlexServer], allow_relay: bool) -> Vec<PlexServer> {
    let mut indexed: Vec<(usize, u8, PlexServer)> = servers
        .iter()
        .enumerate()
        .filter_map(|(index, server)| {
            server_candidate_priority(server, allow_relay)
                .map(|priority| (index, priority, server.clone()))
        })
        .collect();
    indexed.sort_by_key(|(index, priority, _)| (*priority, *index));
    indexed.into_iter().map(|(_, _, server)| server).collect()
}

fn link_candidate_needs_probe(
    server: &PlexServer,
    selected_machines: &std::collections::HashSet<String>,
) -> bool {
    !server.machine_identifier.is_empty()
        && !selected_machines.contains(&server.machine_identifier)
}

fn server_identity_matches(server: &PlexServer, identity_xml: &str) -> bool {
    identity_machine_identifier(identity_xml).as_deref()
        == Some(server.machine_identifier.as_str())
}

fn server_candidate_priority(server: &PlexServer, allow_relay: bool) -> Option<u8> {
    if server.scheme != "https" {
        return None;
    }
    if server.relay {
        return allow_relay.then_some(40);
    }
    let plex_direct = server.host.contains(".plex.direct");
    Some(match (plex_direct, server.local) {
        (true, true) => 0,
        (true, false) => 10,
        (false, true) => 20,
        (false, false) => 30,
    })
}

fn attr_bool(value: &str) -> bool {
    matches!(
        value.to_ascii_lowercase().as_str(),
        "1" | "true" | "yes" | "y"
    )
}

fn origin_from_url(url: &Url) -> String {
    let mut origin = url.clone();
    origin.set_path("");
    origin.set_query(None);
    origin.set_fragment(None);
    origin.as_str().trim_end_matches('/').to_string()
}

fn server_origin(server: &PlexServer) -> String {
    if server.uri.is_empty() {
        format_server_origin(&server.scheme, &server.host, server.port)
    } else {
        server.uri.clone()
    }
}

fn format_server_origin(scheme: &str, host: &str, port: u16) -> String {
    let host = if host.contains(':') && !host.starts_with('[') {
        format!("[{host}]")
    } else {
        host.to_string()
    };
    format!("{scheme}://{host}:{port}")
}

fn identity_machine_identifier(xml: &str) -> Option<String> {
    let mut reader = quick_xml::Reader::from_str(xml);
    let mut buf = Vec::new();
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) | Ok(Event::Empty(e)) => {
                for attr in e.attributes().flatten() {
                    if attr.key.as_ref() == b"machineIdentifier" {
                        return Some(av(&attr));
                    }
                }
            }
            Ok(Event::Eof) => return None,
            Ok(_) => {}
            Err(_) => return None,
        }
        buf.clear();
    }
}

/// Decode an attribute value, unescaping XML entities (e.g. `&amp;` -> `&`).
///
/// quick-xml 0.41 deprecated `unescape_value` in favour of `normalized_value`. It is not a
/// behaviour change: the deprecated method's whole body is
/// `normalized_value_with(XmlVersion::Implicit1_0, ..)`, so passing `Implicit1_0` here is
/// byte-for-byte what it already did. Spelling it out rather than taking the new default,
/// because the new default is a different XML version and this parses whatever Plex sends.
fn av(a: &quick_xml::events::attributes::Attribute) -> String {
    a.normalized_value(quick_xml::XmlVersion::Implicit1_0)
        .map(|c| c.into_owned())
        .unwrap_or_else(|_| String::from_utf8_lossy(&a.value).into_owned())
}

fn hub_from_attrs(e: &quick_xml::events::BytesStart) -> PlexHub {
    let mut hub = PlexHub {
        title: String::new(),
        hub_identifier: String::new(),
        hub_type: String::new(),
        items: Vec::new(),
    };
    for a in e.attributes().flatten() {
        match a.key.as_ref() {
            b"title" => hub.title = av(&a),
            b"hubIdentifier" => hub.hub_identifier = av(&a),
            b"type" => hub.hub_type = av(&a),
            _ => {}
        }
    }
    hub
}

/// Collect every item element from a flat MediaContainer body (no Hub
/// grouping), preserving server order.
fn videos_from_xml(body: &str) -> Vec<PlexVideo> {
    let mut reader = quick_xml::Reader::from_str(body);
    let mut buf = Vec::new();
    let mut out = Vec::new();
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) | Ok(Event::Empty(e)) => {
                if matches!(e.name().as_ref(), b"Video" | b"Directory" | b"Metadata") {
                    out.push(video_from_attrs(&e));
                }
            }
            Ok(Event::Eof) => break,
            Ok(_) => {}
            Err(_) => break,
        }
        buf.clear();
    }
    out
}

fn playlists_from_xml(body: &str) -> Result<Vec<PlexPlaylist>, String> {
    let mut reader = quick_xml::Reader::from_str(body);
    let mut buf = Vec::new();
    let mut out = Vec::new();
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(quick_xml::events::Event::Empty(e))
            | Ok(quick_xml::events::Event::Start(e)) => {
                let tag = e.name().as_ref().to_owned();
                if !matches!(tag.as_slice(), b"Playlist" | b"Metadata" | b"Directory") {
                    buf.clear();
                    continue;
                }
                let mut rating_key = None;
                let mut title = None;
                let mut media_type = None;
                let mut playlist_type = None;
                let mut leaf_count = None;
                for attr in e.attributes() {
                    let attr = attr.map_err(|error| error.to_string())?;
                    match attr.key.as_ref() {
                        b"ratingKey" => rating_key = Some(av(&attr)),
                        b"title" => title = Some(av(&attr)),
                        b"type" => media_type = Some(av(&attr)),
                        b"playlistType" => playlist_type = Some(av(&attr)),
                        b"leafCount" => leaf_count = av(&attr).parse().ok(),
                        _ => {}
                    }
                }
                let playlist_element = tag.as_slice() == b"Playlist";
                if (playlist_element || media_type.as_deref() == Some("playlist"))
                    && playlist_type.as_deref().is_none_or(|kind| kind == "video")
                {
                    if let (Some(rating_key), Some(title)) = (rating_key, title) {
                        if !rating_key.is_empty() && !title.is_empty() {
                            out.push(PlexPlaylist {
                                rating_key,
                                title,
                                leaf_count,
                            });
                        }
                    }
                }
            }
            Ok(quick_xml::events::Event::Eof) => break,
            Ok(_) => {}
            Err(error) => return Err(error.to_string()),
        }
        buf.clear();
    }
    Ok(out)
}

fn video_from_attrs(e: &quick_xml::events::BytesStart) -> PlexVideo {
    let mut v = PlexVideo {
        key: String::new(),
        rating_key: String::new(),
        title: String::new(),
        title_sort: None,
        summary: None,
        duration: None,
        view_offset: None,
        view_count: None,
        thumb: None,
        grandparent_thumb: None,
        art: None,
        added_at: None,
        last_viewed_at: None,
        updated_at: None,
        media: vec![],
        year: None,
        media_type: None,
        index: None,
        parent_index: None,
        grandparent_title: None,
        parent_title: None,
        parent_rating_key: None,
        grandparent_rating_key: None,
        guids: vec![],
    };
    for a in e.attributes().flatten() {
        match a.key.as_ref() {
            b"key" => v.key = av(&a),
            b"ratingKey" => v.rating_key = av(&a),
            b"title" => v.title = av(&a),
            b"titleSort" => v.title_sort = Some(av(&a)),
            b"summary" => v.summary = Some(av(&a)),
            b"duration" => v.duration = av(&a).parse().ok(),
            b"viewOffset" => v.view_offset = av(&a).parse().ok(),
            b"viewCount" => v.view_count = av(&a).parse().ok(),
            b"lastViewedAt" => v.last_viewed_at = av(&a).parse().ok(),
            b"thumb" => v.thumb = Some(av(&a)),
            b"grandparentThumb" => v.grandparent_thumb = Some(av(&a)),
            b"art" => v.art = Some(av(&a)),
            b"year" => v.year = av(&a).parse().ok(),
            b"type" => v.media_type = Some(av(&a)),
            b"index" => v.index = av(&a).parse().ok(),
            b"parentIndex" => v.parent_index = av(&a).parse().ok(),
            b"grandparentTitle" => v.grandparent_title = Some(av(&a)),
            b"parentTitle" => v.parent_title = Some(av(&a)),
            b"parentRatingKey" => v.parent_rating_key = Some(av(&a)),
            b"grandparentRatingKey" => v.grandparent_rating_key = Some(av(&a)),
            _ => {}
        }
    }
    if v.rating_key.is_empty() {
        v.rating_key = v.key.clone();
    }
    v
}

/// The URL + query for a person-filtered section listing: `filter` is the
/// Plex param name (`actor`/`director`/`writer` — validated by the caller),
/// `tag_id` the digits-validated server-local tag id, `type_filter` the
/// explicit item type ("1" movies / "2" shows), plus the standard paging.
fn person_filter_query(
    base: &str,
    section_key: &str,
    filter: &str,
    tag_id: &str,
    type_filter: &str,
    start: usize,
    size: usize,
) -> (String, Vec<(String, String)>) {
    (
        format!("{base}/library/sections/{section_key}/all"),
        vec![
            ("includeGuids".to_string(), "1".to_string()),
            ("type".to_string(), type_filter.to_string()),
            (filter.to_string(), tag_id.to_string()),
            ("X-Plex-Container-Start".to_string(), start.to_string()),
            ("X-Plex-Container-Size".to_string(), size.to_string()),
        ],
    )
}

/// Build the playable URL for a Plex part key. An absolute part URL is reduced
/// to its path+query and re-rooted onto `base` (our chosen connection origin).
/// Auth deliberately travels as the `X-Plex-Token` HEADER — threaded to mpv by
/// the playback layer — never as a query parameter: mpv renders `${path}` in
/// its title bar, stats overlay, and playlist, so a token-bearing URL would
/// surface the credential there. (See `.agents/decisions.md`, 2026-07-03.)
fn part_url(base: &str, part_key: &str) -> String {
    let mut rel_path = part_key.to_string();
    if let Ok(u) = url::Url::parse(part_key) {
        rel_path = u.path().to_string();
        if let Some(q) = u.query() {
            rel_path.push('?');
            rel_path.push_str(q);
        }
    }
    let mut full = if rel_path.starts_with('/') {
        format!("{base}{rel_path}")
    } else {
        format!("{base}/{rel_path}")
    };
    if full.contains('?') {
        full.push('&');
    } else {
        full.push('?');
    }
    full.push_str("download=1");
    full
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn library_sections_parse_scalar_attributes() {
        let xml = r#"<MediaContainer size="2">
  <Directory key="1" title="Movies" type="movie"
             agent="tv.plex.agents.movie" scanner="Plex Movie"/>
  <Directory key="2" title="Shows" type="show"/>
</MediaContainer>"#;

        let container: MediaContainer = serde_xml_rs::from_str(xml).expect("section parse");
        assert_eq!(container.directories.len(), 2);

        let movies = &container.directories[0];
        assert_eq!(movies.key, "1");
        assert_eq!(movies.title, "Movies");
        assert_eq!(movies.section_type, "movie");
        assert_eq!(movies.agent.as_deref(), Some("tv.plex.agents.movie"));
        assert_eq!(movies.scanner.as_deref(), Some("Plex Movie"));

        let shows = &container.directories[1];
        assert_eq!(shows.key, "2");
        assert_eq!(shows.title, "Shows");
        assert_eq!(shows.section_type, "show");
        assert_eq!(shows.agent, None);
        assert_eq!(shows.scanner, None);
    }

    #[test]
    fn detail_parse_captures_full_metadata() {
        // A representative `/library/metadata/{rk}` movie response. The parse must
        // capture the scalar attrs AND every child collection (genre/director/
        // writer/country/role/media/part/stream) — the fields the section listing
        // omits and the detail view exists to show.
        let xml = r#"<MediaContainer size="1">
  <Video ratingKey="12345" key="/library/metadata/12345" type="movie"
         title="Blade Runner 2049" contentRating="R" rating="8.0"
         audienceRating="8.8" studio="Warner Bros." year="2017"
         tagline="There is an order to things." originallyAvailableAt="2017-10-06"
         summary="A young blade runner uncovers a secret." duration="9754000"
         viewCount="1" viewOffset="120000" thumb="/library/metadata/12345/thumb/1"
         art="/library/metadata/12345/art/1">
    <Media id="1" videoResolution="4k" width="3840" height="2160" videoCodec="hevc"
           audioCodec="truehd" container="mkv" videoDynamicRange="Dolby Vision">
      <Part id="2" key="/library/parts/2/file.mkv" file="/data/br2049.mkv" container="mkv">
        <Stream streamType="2" codec="truehd" language="English" channels="8" displayTitle="TrueHD 7.1"/>
        <Stream streamType="3" codec="srt" language="English" displayTitle="English (SRT)"/>
      </Part>
    </Media>
    <Genre tag="Science Fiction"/>
    <Genre tag="Drama"/>
    <Director tag="Denis Villeneuve"/>
    <Writer tag="Hampton Fancher"/>
    <Writer tag="Michael Green"/>
    <Country tag="United States"/>
    <Role tag="Ryan Gosling" role="K" thumb="/library/metadata/12345/role/1"/>
    <Role tag="Harrison Ford" role="Rick Deckard" thumb="/library/metadata/12345/role/2"/>
  </Video>
</MediaContainer>"#;

        let container: DetailContainer = serde_xml_rs::from_str(xml).expect("detail parse");
        let d = container.videos.into_iter().next().expect("one Video");

        // Scalar attributes.
        assert_eq!(d.key, "/library/metadata/12345");
        assert_eq!(d.rating_key, "12345");
        assert_eq!(d.title, "Blade Runner 2049");
        assert_eq!(d.media_type.as_deref(), Some("movie"));
        assert_eq!(
            d.summary.as_deref(),
            Some("A young blade runner uncovers a secret.")
        );
        assert_eq!(d.content_rating.as_deref(), Some("R"));
        assert_eq!(d.rating, Some(8.0));
        assert_eq!(d.audience_rating, Some(8.8));
        assert_eq!(d.studio.as_deref(), Some("Warner Bros."));
        assert_eq!(d.tagline.as_deref(), Some("There is an order to things."));
        assert_eq!(d.originally_available_at.as_deref(), Some("2017-10-06"));
        assert_eq!(d.year, Some(2017));
        assert_eq!(d.duration, Some(9_754_000));
        assert_eq!(d.view_count, Some(1));
        assert_eq!(d.view_offset, Some(120_000));
        assert_eq!(d.thumb.as_deref(), Some("/library/metadata/12345/thumb/1"));
        assert_eq!(d.art.as_deref(), Some("/library/metadata/12345/art/1"));

        // Child collections.
        let genres: Vec<_> = d.genres.iter().map(|t| t.tag.as_str()).collect();
        assert_eq!(genres, ["Science Fiction", "Drama"]);
        assert_eq!(
            d.directors
                .iter()
                .map(|t| t.tag.as_str())
                .collect::<Vec<_>>(),
            ["Denis Villeneuve"]
        );
        assert_eq!(
            d.writers.iter().map(|t| t.tag.as_str()).collect::<Vec<_>>(),
            ["Hampton Fancher", "Michael Green"]
        );
        assert_eq!(
            d.countries
                .iter()
                .map(|t| t.tag.as_str())
                .collect::<Vec<_>>(),
            ["United States"]
        );

        // Cast: name + character + headshot.
        assert_eq!(d.roles.len(), 2);
        assert_eq!(d.roles[0].tag, "Ryan Gosling");
        assert_eq!(d.roles[0].role.as_deref(), Some("K"));
        assert_eq!(
            d.roles[0].thumb.as_deref(),
            Some("/library/metadata/12345/role/1")
        );
        assert_eq!(d.roles[1].tag, "Harrison Ford");
        assert_eq!(d.roles[1].role.as_deref(), Some("Rick Deckard"));

        // Media / Part / Stream.
        assert_eq!(d.media.len(), 1);
        let m = &d.media[0];
        assert_eq!(m.video_resolution.as_deref(), Some("4k"));
        assert_eq!(m.width, Some(3840));
        assert_eq!(m.height, Some(2160));
        assert_eq!(m.video_codec.as_deref(), Some("hevc"));
        assert_eq!(m.audio_codec.as_deref(), Some("truehd"));
        assert_eq!(m.container.as_deref(), Some("mkv"));
        assert_eq!(m.video_dynamic_range.as_deref(), Some("Dolby Vision"));
        assert_eq!(m.parts.len(), 1);
        let streams = &m.parts[0].streams;
        assert_eq!(streams.len(), 2);
        assert_eq!(streams[0].stream_type, Some(2));
        assert_eq!(streams[0].codec.as_deref(), Some("truehd"));
        assert_eq!(streams[0].language.as_deref(), Some("English"));
        assert_eq!(streams[0].channels, Some(8));
        assert_eq!(streams[0].display_title.as_deref(), Some("TrueHD 7.1"));
        assert_eq!(streams[1].stream_type, Some(3));
        assert_eq!(streams[1].language.as_deref(), Some("English"));
        assert_eq!(streams[1].display_title.as_deref(), Some("English (SRT)"));
    }

    #[test]
    fn plexdir_carries_added_and_last_viewed_into_video() {
        // Plex shows deserialize as Directory rows; the conversion must keep
        // addedAt / lastViewedAt or the date-added / last-played sorts rank all
        // Plex shows as missing-timestamp (sorting review r2).
        let d = PlexDir {
            key: "/k".into(),
            rating_key: Some("rk".into()),
            title: "Some Show".into(),
            media_type: Some("show".into()),
            thumb: None,
            year: Some(2020),
            summary: None,
            added_at: Some(1_700_000_000),
            last_viewed_at: Some(1_751_000_000),
            parent_rating_key: None,
        };
        let v: PlexVideo = d.into();
        assert_eq!(v.added_at, Some(1_700_000_000));
        assert_eq!(v.last_viewed_at, Some(1_751_000_000));
    }

    /// The pin `PlexSource::rediscover` derives comes from here: if this stopped
    /// reflecting the installed server, rediscovery would silently go unpinned
    /// and could repoint the source at another machine (codex r5).
    #[test]
    fn server_machine_id_reflects_the_installed_server() {
        let mut lib = PlexLibrary::new("tok".into(), "dev".into());
        assert_eq!(lib.server_machine_id(), None, "no server installed yet");
        lib.set_server(server("alpha", "https", "a.example", false, false));
        assert_eq!(lib.server_machine_id().as_deref(), Some("alpha-id"));
        lib.set_server(server("beta", "https", "b.example", false, false));
        assert_eq!(lib.server_machine_id().as_deref(), Some("beta-id"));
    }

    fn server(name: &str, scheme: &str, host: &str, local: bool, relay: bool) -> PlexServer {
        PlexServer {
            name: name.to_string(),
            host: host.to_string(),
            port: 32400,
            scheme: scheme.to_string(),
            uri: format_server_origin(scheme, host, 32400),
            local,
            relay,
            machine_identifier: format!("{name}-id"),
            version: "1.0".to_string(),
        }
    }

    #[test]
    fn part_urls_carry_no_token_and_keep_download_flag() {
        // Server-relative part key, no existing query.
        assert_eq!(
            part_url("https://plex.example:32400", "/library/parts/42/file.mkv"),
            "https://plex.example:32400/library/parts/42/file.mkv?download=1"
        );
        // A part key that already has a query keeps it and appends with '&'.
        assert_eq!(
            part_url(
                "https://plex.example:32400",
                "/library/parts/42/file.mkv?x=1"
            ),
            "https://plex.example:32400/library/parts/42/file.mkv?x=1&download=1"
        );
        // An absolute part URL is re-rooted onto our chosen server origin.
        let u = part_url(
            "https://plex.example:32400",
            "https://other.host:12345/library/parts/7/file.mkv?y=2",
        );
        assert_eq!(
            u,
            "https://plex.example:32400/library/parts/7/file.mkv?y=2&download=1"
        );
        // The credential must never ride in the URL; it travels as a header.
        assert!(!u.contains("X-Plex-Token"));
    }

    #[test]
    fn video_attrs_capture_series_poster_and_backdrop_art() {
        let mut e = quick_xml::events::BytesStart::new("Video");
        e.push_attribute(("ratingKey", "42"));
        e.push_attribute(("title", "Fallen Angel"));
        e.push_attribute(("thumb", "/library/metadata/42/thumb/1"));
        e.push_attribute(("grandparentThumb", "/library/metadata/7/thumb/9"));
        e.push_attribute(("art", "/library/metadata/7/art/9"));

        let v = video_from_attrs(&e);
        assert_eq!(v.thumb.as_deref(), Some("/library/metadata/42/thumb/1"));
        assert_eq!(
            v.grandparent_thumb.as_deref(),
            Some("/library/metadata/7/thumb/9")
        );
        assert_eq!(v.art.as_deref(), Some("/library/metadata/7/art/9"));
    }

    #[test]
    fn playlist_xml_accepts_legacy_and_metadata_rows_but_only_video() {
        let xml = r#"
          <MediaContainer size="4">
            <Playlist ratingKey="11" key="/playlists/11/items"
                      title="Films &amp; Shorts" type="playlist"
                      playlistType="video" leafCount="3" />
            <Metadata ratingKey="12" key="/playlists/12/items"
                      title="Smart Picks" type="playlist"
                      playlistType="video" leafCount="8" />
            <Metadata ratingKey="13" key="/playlists/13/items"
                      title="Songs" type="playlist"
                      playlistType="audio" leafCount="20" />
            <Directory ratingKey="14" title="Not a playlist" type="collection" />
          </MediaContainer>
        "#;
        assert_eq!(
            playlists_from_xml(xml).unwrap(),
            vec![
                PlexPlaylist {
                    rating_key: "11".to_string(),
                    title: "Films & Shorts".to_string(),
                    leaf_count: Some(3),
                },
                PlexPlaylist {
                    rating_key: "12".to_string(),
                    title: "Smart Picks".to_string(),
                    leaf_count: Some(8),
                },
            ]
        );
        assert!(playlists_from_xml("<MediaContainer><Playlist").is_err());
    }

    #[test]
    fn person_filter_query_builds_filtered_paged_url() {
        let (url, params) = person_filter_query(
            "https://plex.example:32400",
            "3",
            "director",
            "456",
            "1",
            60,
            200,
        );
        assert_eq!(url, "https://plex.example:32400/library/sections/3/all");
        assert!(params.contains(&("director".to_string(), "456".to_string())));
        assert!(params.contains(&("type".to_string(), "1".to_string())));
        assert!(params.contains(&("X-Plex-Container-Start".to_string(), "60".to_string())));
        assert!(params.contains(&("X-Plex-Container-Size".to_string(), "200".to_string())));
        // The credential never rides in the query; it travels as a header.
        assert!(!params.iter().any(|(k, _)| k.contains("Token")));
    }

    #[test]
    fn detail_parse_captures_person_tag_ids() {
        let xml = r#"
            <MediaContainer size="1">
              <Video ratingKey="42" key="/library/metadata/42" title="A Movie" type="movie">
                <Role tag="Actor One" role="Hero" id="123" />
                <Role tag="No Id" role="Extra" />
                <Director tag="Dir One" id="456" />
                <Writer tag="Writer One" id="789" />
              </Video>
            </MediaContainer>
        "#;
        let c: DetailContainer = serde_xml_rs::from_str(xml).expect("parse");
        let d = &c.videos[0];
        assert_eq!(d.roles[0].id.as_deref(), Some("123"));
        assert_eq!(d.roles[1].id, None);
        assert_eq!(d.directors[0].id.as_deref(), Some("456"));
        assert_eq!(d.writers[0].id.as_deref(), Some("789"));
    }

    #[test]
    fn detail_container_parses_metadata_and_directory_parent_attributes() {
        let xml = r#"
            <MediaContainer size="2">
              <Metadata ratingKey="202" key="/library/metadata/202" title="Next Up"
                        type="episode" grandparentThumb="/library/metadata/100/thumb/1"
                        index="2" parentIndex="1" grandparentTitle="The Show"
                        parentTitle="Season 1" parentRatingKey="150"
                        grandparentRatingKey="100" />
              <Directory ratingKey="100" key="/library/metadata/100"
                         title="The Show" type="show" />
            </MediaContainer>
        "#;

        let container: DetailContainer = serde_xml_rs::from_str(xml).expect("detail parse");
        assert_eq!(container.metadata.len(), 1);
        assert_eq!(container.directories.len(), 1);

        let episode = &container.metadata[0];
        assert_eq!(episode.rating_key, "202");
        assert_eq!(episode.key, "/library/metadata/202");
        assert_eq!(episode.title, "Next Up");
        assert_eq!(episode.media_type.as_deref(), Some("episode"));
        assert_eq!(
            episode.grandparent_thumb.as_deref(),
            Some("/library/metadata/100/thumb/1")
        );
        assert_eq!(episode.index, Some(2));
        assert_eq!(episode.parent_index, Some(1));
        assert_eq!(episode.grandparent_title.as_deref(), Some("The Show"));
        assert_eq!(episode.parent_title.as_deref(), Some("Season 1"));
        assert_eq!(episode.parent_rating_key.as_deref(), Some("150"));
        assert_eq!(episode.grandparent_rating_key.as_deref(), Some("100"));

        let show = &container.directories[0];
        assert_eq!(show.rating_key, "100");
        assert_eq!(show.key, "/library/metadata/100");
        assert_eq!(show.title, "The Show");
        assert_eq!(show.media_type.as_deref(), Some("show"));
    }

    #[test]
    fn episode_and_season_rows_carry_parent_keys() {
        let xml = r#"
            <MediaContainer size="3">
              <Video ratingKey="202" key="/library/metadata/202" title="Next Up"
                     titleSort="Next Up, The" summary="An episode summary"
                     duration="2700000" viewOffset="1200000" viewCount="1"
                     thumb="/library/metadata/202/thumb/1"
                     grandparentThumb="/library/metadata/100/thumb/1"
                     art="/library/metadata/100/art/1" addedAt="1751000000"
                     lastViewedAt="1751500000" updatedAt="1751600000" year="2025"
                     type="episode" index="2" parentIndex="1"
                     grandparentTitle="The Show" parentTitle="Season 1"
                     parentRatingKey="150" grandparentRatingKey="100">
                <Guid id="imdb://tt1234567" />
                <Guid id="tmdb://7654321" />
                <Media id="301" duration="2700000" bitrate="12000" width="3840"
                       height="2160" aspectRatio="1.78" videoCodec="hevc"
                       audioCodec="eac3" container="mkv">
                  <Part id="401" key="/library/parts/401/file.mkv" duration="2700000"
                        file="/data/episode.mkv" size="987654321" container="mkv" />
                </Media>
              </Video>
              <Metadata ratingKey="303" key="/library/metadata/303"
                        title="Metadata Row" type="movie" />
              <Directory ratingKey="150" key="/library/metadata/150/children"
                     title="Season 1" type="season" thumb="/library/metadata/150/thumb/1"
                     year="2025" summary="The first season" addedAt="1750000000"
                     lastViewedAt="1751400000" parentRatingKey="100" />
            </MediaContainer>
        "#;
        // Attribute path (hubs / on-deck / streamed listings).
        let items = videos_from_xml(xml);
        assert_eq!(items[0].parent_rating_key.as_deref(), Some("150"));
        assert_eq!(items[0].grandparent_rating_key.as_deref(), Some("100"));
        assert_eq!(items[2].parent_rating_key.as_deref(), Some("100"));

        // Serde path (the get_items listing parse) + the Directory→Video map.
        let c: ItemsContainer = serde_xml_rs::from_str(xml).expect("parse");
        assert_eq!(c.videos.len(), 1);
        assert_eq!(c.metadata.len(), 1);
        assert_eq!(c.directories.len(), 1);

        let episode = &c.videos[0];
        assert_eq!(episode.key, "/library/metadata/202");
        assert_eq!(episode.rating_key, "202");
        assert_eq!(episode.title, "Next Up");
        assert_eq!(episode.title_sort.as_deref(), Some("Next Up, The"));
        assert_eq!(episode.summary.as_deref(), Some("An episode summary"));
        assert_eq!(episode.duration, Some(2_700_000));
        assert_eq!(episode.view_offset, Some(1_200_000));
        assert_eq!(episode.view_count, Some(1));
        assert_eq!(
            episode.thumb.as_deref(),
            Some("/library/metadata/202/thumb/1")
        );
        assert_eq!(
            episode.grandparent_thumb.as_deref(),
            Some("/library/metadata/100/thumb/1")
        );
        assert_eq!(episode.art.as_deref(), Some("/library/metadata/100/art/1"));
        assert_eq!(episode.added_at, Some(1_751_000_000));
        assert_eq!(episode.last_viewed_at, Some(1_751_500_000));
        assert_eq!(episode.updated_at, Some(1_751_600_000));
        assert_eq!(episode.year, Some(2025));
        assert_eq!(episode.media_type.as_deref(), Some("episode"));
        assert_eq!(episode.index, Some(2));
        assert_eq!(episode.parent_index, Some(1));
        assert_eq!(episode.grandparent_title.as_deref(), Some("The Show"));
        assert_eq!(episode.parent_title.as_deref(), Some("Season 1"));
        assert_eq!(episode.parent_rating_key.as_deref(), Some("150"));
        assert_eq!(episode.grandparent_rating_key.as_deref(), Some("100"));
        assert_eq!(
            episode
                .guids
                .iter()
                .map(|g| g.id.as_str())
                .collect::<Vec<_>>(),
            ["imdb://tt1234567", "tmdb://7654321"]
        );

        assert_eq!(episode.media.len(), 1);
        let media = &episode.media[0];
        assert_eq!(media.id, "301");
        assert_eq!(media.duration, Some(2_700_000));
        assert_eq!(media.bitrate, Some(12_000));
        assert_eq!(media.width, Some(3840));
        assert_eq!(media.height, Some(2160));
        assert_eq!(media.aspect_ratio, Some(1.78));
        assert_eq!(media.video_codec.as_deref(), Some("hevc"));
        assert_eq!(media.audio_codec.as_deref(), Some("eac3"));
        assert_eq!(media.container.as_deref(), Some("mkv"));
        assert_eq!(media.parts.len(), 1);
        let part = &media.parts[0];
        assert_eq!(part.id, "401");
        assert_eq!(part.key, "/library/parts/401/file.mkv");
        assert_eq!(part.duration, Some(2_700_000));
        assert_eq!(part.file, "/data/episode.mkv");
        assert_eq!(part.size, Some(987_654_321));
        assert_eq!(part.container.as_deref(), Some("mkv"));

        assert_eq!(c.metadata[0].rating_key, "303");
        assert_eq!(c.metadata[0].title, "Metadata Row");
        assert_eq!(c.metadata[0].media_type.as_deref(), Some("movie"));

        let season: PlexVideo = c.directories[0].clone().into();
        assert_eq!(season.key, "/library/metadata/150/children");
        assert_eq!(season.rating_key, "150");
        assert_eq!(season.title, "Season 1");
        assert_eq!(season.media_type.as_deref(), Some("season"));
        assert_eq!(
            season.thumb.as_deref(),
            Some("/library/metadata/150/thumb/1")
        );
        assert_eq!(season.year, Some(2025));
        assert_eq!(season.summary.as_deref(), Some("The first season"));
        assert_eq!(season.added_at, Some(1_750_000_000));
        assert_eq!(season.last_viewed_at, Some(1_751_400_000));
        assert_eq!(season.parent_rating_key.as_deref(), Some("100"));
        assert_eq!(season.grandparent_rating_key, None);
    }

    #[test]
    fn on_deck_body_parses_items_with_last_viewed_stamp() {
        let xml = r#"
            <MediaContainer size="2">
              <Video ratingKey="101" key="/library/metadata/101" title="Blood and Bone"
                     type="movie" viewOffset="1200000" lastViewedAt="1751500000" />
              <Video ratingKey="202" key="/library/metadata/202" title="Next Up"
                     type="episode" grandparentTitle="Show" index="3" parentIndex="1" />
            </MediaContainer>
        "#;
        let items = videos_from_xml(xml);
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].rating_key, "101");
        assert_eq!(items[0].last_viewed_at, Some(1_751_500_000));
        assert_eq!(items[0].view_offset, Some(1_200_000));
        assert_eq!(items[1].media_type.as_deref(), Some("episode"));
        assert_eq!(items[1].last_viewed_at, None);
    }

    #[test]
    fn resource_parser_preserves_connection_metadata_and_origin() {
        let lib = PlexLibrary::new("token".to_string(), "client".to_string());
        let xml = r#"
            <MediaContainer>
              <Resource name="Home" clientIdentifier="machine-1" productVersion="1.2.3" provides="server">
                <Connection uri="https://192-168-1-2.hash.plex.direct:32400/path?x=1&amp;y=2" local="1" relay="0" />
                <Connection uri="https://remote.hash.plex.direct:32400" local="0" relay="0"></Connection>
                <Connection uri="https://relay.example:443" local="0" relay="1" />
                <Connection uri="http://[2001:db8::1]:32400" local="1" relay="0" />
              </Resource>
              <Resource name="Player" clientIdentifier="client-1" provides="client">
                <Connection uri="https://ignored.example:32400" local="0" relay="0" />
              </Resource>
            </MediaContainer>
        "#;

        let servers = lib.parse_resources_stream(xml).expect("parse resources");

        assert_eq!(servers.len(), 4);
        assert_eq!(servers[0].machine_identifier, "machine-1");
        assert_eq!(servers[0].uri, "https://192-168-1-2.hash.plex.direct:32400");
        assert!(servers[0].local);
        assert!(!servers[0].relay);
        assert_eq!(servers[1].uri, "https://remote.hash.plex.direct:32400");
        assert!(servers[2].relay);
        assert_eq!(servers[3].uri, "http://[2001:db8::1]:32400");
    }

    #[test]
    fn ordered_candidates_require_https_and_exclude_relay_by_default() {
        let servers = vec![
            server("http-local", "http", "192.168.1.2", true, false),
            server("relay", "https", "relay.example", false, true),
            server("remote", "https", "remote.hash.plex.direct", false, false),
            server(
                "local",
                "https",
                "192-168-1-2.hash.plex.direct",
                true,
                false,
            ),
            server("custom", "https", "plex.example.com", false, false),
        ];

        let ordered = ordered_server_candidates(&servers, false);
        let names: Vec<&str> = ordered.iter().map(|s| s.name.as_str()).collect();

        assert_eq!(names, vec!["local", "remote", "custom"]);
    }

    #[test]
    fn ordered_candidates_can_include_relay_last_when_allowed() {
        let servers = vec![
            server("relay", "https", "relay.example", false, true),
            server("remote", "https", "remote.hash.plex.direct", false, false),
        ];

        let ordered = ordered_server_candidates(&servers, true);
        let names: Vec<&str> = ordered.iter().map(|s| s.name.as_str()).collect();

        assert_eq!(names, vec!["remote", "relay"]);
    }

    #[test]
    fn manual_ipv6_server_base_is_bracketed() {
        let mut lib = PlexLibrary::new("token".to_string(), "client".to_string());
        lib.set_server_manual(
            "2001:db8::1".to_string(),
            32400,
            true,
            Some("server".to_string()),
        );

        assert_eq!(
            lib.server_base().as_deref(),
            Some("https://[2001:db8::1]:32400")
        );
    }

    #[test]
    fn identity_machine_identifier_is_extracted() {
        let xml = r#"<MediaContainer size="0" machineIdentifier="machine-1" />"#;

        assert_eq!(
            identity_machine_identifier(xml).as_deref(),
            Some("machine-1")
        );
    }

    #[test]
    fn link_candidates_require_an_unselected_machine_identifier() {
        let selected = std::collections::HashSet::from(["alpha-id".to_string()]);
        let alpha = server("alpha", "https", "alpha.example", false, false);
        let beta = server("beta", "https", "beta.example", false, false);
        let mut unknown = server("unknown", "https", "unknown.example", false, false);
        unknown.machine_identifier.clear();

        assert!(!link_candidate_needs_probe(&alpha, &selected));
        assert!(link_candidate_needs_probe(&beta, &selected));
        assert!(!link_candidate_needs_probe(&unknown, &selected));
    }

    #[test]
    fn reachable_server_identity_must_match_the_discovery_machine() {
        let alpha = server("alpha", "https", "alpha.example", false, false);
        let matching = r#"<MediaContainer machineIdentifier="alpha-id" />"#;
        let other = r#"<MediaContainer machineIdentifier="other-id" />"#;

        assert!(server_identity_matches(&alpha, matching));
        assert!(!server_identity_matches(&alpha, other));
        assert!(!server_identity_matches(&alpha, "<MediaContainer />"));
    }
}
