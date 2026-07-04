use quick_xml::events::Event;
use quick_xml::Reader;
use reqwest::Client;
use serde::Deserialize;
use std::time::Duration;
use url::Url;

#[derive(Debug, Deserialize, Clone)]
#[allow(dead_code)] // deserialized Plex XML fields; not all are read in code
pub struct PlexServer {
    pub name: String,
    pub host: String,
    pub port: u16,
    pub scheme: String,
    #[serde(default)]
    pub uri: String,
    #[serde(default)]
    pub local: bool,
    #[serde(default)]
    pub relay: bool,
    #[serde(rename = "machineIdentifier")]
    pub machine_identifier: String,
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
    pub key: String,
    #[serde(rename = "ratingKey")]
    pub rating_key: Option<String>,
    pub title: String,
    #[serde(rename = "type", default)]
    pub media_type: Option<String>,
    #[serde(rename = "thumb")]
    pub thumb: Option<String>,
    pub year: Option<u32>,
    pub summary: Option<String>,
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
            added_at: None,
            updated_at: None,
            media: vec![],
            year: d.year,
            media_type: d.media_type.or_else(|| Some("directory".to_string())),
            index: None,
            parent_index: None,
            grandparent_title: None,
            parent_title: None,
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

#[derive(Debug, Deserialize, Clone)]
#[allow(dead_code)] // deserialized Plex XML fields; not all are read in code
pub struct LibrarySection {
    pub key: String,
    pub title: String,
    #[serde(rename = "type")]
    pub section_type: String,
    pub agent: Option<String>,
    pub scanner: Option<String>,
}

#[derive(Debug, Deserialize, Clone)]
#[allow(dead_code)] // deserialized Plex XML fields; not all are read in code
pub struct PlexVideo {
    pub key: String,
    #[serde(rename = "ratingKey")]
    pub rating_key: String,
    pub title: String,
    #[serde(rename = "titleSort")]
    pub title_sort: Option<String>,
    pub summary: Option<String>,
    pub duration: Option<u64>,
    #[serde(rename = "viewOffset")]
    pub view_offset: Option<u64>,
    #[serde(rename = "viewCount", default)]
    pub view_count: Option<u64>,
    #[serde(rename = "thumb")]
    pub thumb: Option<String>,
    #[serde(rename = "grandparentThumb")]
    pub grandparent_thumb: Option<String>,
    #[serde(rename = "art")]
    pub art: Option<String>,
    #[serde(rename = "addedAt")]
    pub added_at: Option<u64>,
    #[serde(rename = "updatedAt")]
    pub updated_at: Option<u64>,
    #[serde(rename = "Media", default)]
    pub media: Vec<PlexMedia>,
    pub year: Option<u32>,
    #[serde(rename = "type", default)]
    pub media_type: Option<String>,
    #[serde(rename = "index")]
    pub index: Option<u32>,
    #[serde(rename = "parentIndex")]
    pub parent_index: Option<u32>,
    #[serde(rename = "grandparentTitle")]
    pub grandparent_title: Option<String>,
    #[serde(rename = "parentTitle")]
    pub parent_title: Option<String>,
}

#[derive(Debug, Deserialize, Clone)]
#[allow(dead_code)] // deserialized Plex XML fields; not all are read in code
pub struct PlexMedia {
    pub id: String,
    pub duration: Option<u64>,
    #[serde(rename = "bitrate")]
    pub bitrate: Option<u32>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    #[serde(rename = "aspectRatio")]
    pub aspect_ratio: Option<f32>,
    #[serde(rename = "videoCodec")]
    pub video_codec: Option<String>,
    #[serde(rename = "audioCodec")]
    pub audio_codec: Option<String>,
    #[serde(rename = "container")]
    pub container: Option<String>,
    #[serde(rename = "Part", default)]
    pub parts: Vec<PlexPart>,
}

#[derive(Debug, Deserialize, Clone)]
#[allow(dead_code)] // deserialized Plex XML fields; not all are read in code
pub struct PlexPart {
    pub id: String,
    pub key: String,
    pub duration: Option<u64>,
    pub file: String,
    pub size: Option<u64>,
    pub container: Option<String>,
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
        identity_machine_identifier(&body).as_deref() == Some(server.machine_identifier.as_str())
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
        // Stream-parse: serde_xml_rs 0.6 errors ("duplicate field 'Video'") on the
        // nested <Hub><Video/>…</Hub> shape, so walk it manually and preserve order.
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
            rdr.trim_text(true);
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
fn av(a: &quick_xml::events::attributes::Attribute) -> String {
    a.unescape_value()
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
        updated_at: None,
        media: vec![],
        year: None,
        media_type: None,
        index: None,
        parent_index: None,
        grandparent_title: None,
        parent_title: None,
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
            b"thumb" => v.thumb = Some(av(&a)),
            b"grandparentThumb" => v.grandparent_thumb = Some(av(&a)),
            b"art" => v.art = Some(av(&a)),
            b"year" => v.year = av(&a).parse().ok(),
            b"type" => v.media_type = Some(av(&a)),
            b"index" => v.index = av(&a).parse().ok(),
            b"parentIndex" => v.parent_index = av(&a).parse().ok(),
            b"grandparentTitle" => v.grandparent_title = Some(av(&a)),
            b"parentTitle" => v.parent_title = Some(av(&a)),
            _ => {}
        }
    }
    if v.rating_key.is_empty() {
        v.rating_key = v.key.clone();
    }
    v
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
            part_url("https://plex.example:32400", "/library/parts/42/file.mkv?x=1"),
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
}
