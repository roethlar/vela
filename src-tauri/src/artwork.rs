//! Credential-free Plex artwork references and the bounded app-local protocol
//! that resolves them. Provider credentials stay in Rust and reach Plex only as
//! an HTTP header; the webview sees an opaque source/image request marker that
//! `convertFileSrc` turns into the platform's custom-protocol URL.

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use tauri::http::{header, Method, Request, Response, StatusCode};

use crate::AppState;

pub(crate) const MAX_ARTWORK_BYTES: usize = 16 * 1024 * 1024;
const MAX_ARTWORK_PATH_BYTES: usize = 2048;
const MAX_ARTWORK_DIMENSION: u32 = 4096;
const MAX_ARTWORK_PIXELS: u64 = 8_388_608;
pub(crate) const ARTWORK_MARKER_PREFIX: &str = "vela-artwork:";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArtworkRequest {
    pub(crate) path: String,
    pub(crate) width: u32,
    pub(crate) height: u32,
}

#[derive(Debug)]
pub struct ArtworkResponse {
    pub(crate) content_type: &'static str,
    pub(crate) body: Vec<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArtworkError {
    InvalidRequest,
    Unsupported,
    Unavailable,
    InvalidResponse,
    TooLarge,
}

pub(crate) fn accepted_image_mime(value: &str) -> Option<&'static str> {
    match value
        .split(';')
        .next()
        .unwrap_or_default()
        .trim()
        .to_ascii_lowercase()
        .as_str()
    {
        "image/jpeg" => Some("image/jpeg"),
        "image/png" => Some("image/png"),
        "image/webp" => Some("image/webp"),
        "image/gif" => Some("image/gif"),
        "image/avif" => Some("image/avif"),
        _ => None,
    }
}

fn decode_percent_once(value: &str) -> Result<String, ArtworkError> {
    let bytes = value.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] != b'%' {
            decoded.push(bytes[index]);
            index += 1;
            continue;
        }
        if index + 2 >= bytes.len() {
            return Err(ArtworkError::InvalidRequest);
        }
        let digit = |byte: u8| match byte {
            b'0'..=b'9' => Some(byte - b'0'),
            b'a'..=b'f' => Some(byte - b'a' + 10),
            b'A'..=b'F' => Some(byte - b'A' + 10),
            _ => None,
        };
        let high = digit(bytes[index + 1]).ok_or(ArtworkError::InvalidRequest)?;
        let low = digit(bytes[index + 2]).ok_or(ArtworkError::InvalidRequest)?;
        decoded.push((high << 4) | low);
        index += 3;
    }
    String::from_utf8(decoded).map_err(|_| ArtworkError::InvalidRequest)
}

pub(crate) fn validate_artwork_request(request: &ArtworkRequest) -> Result<(), ArtworkError> {
    if request.width == 0
        || request.height == 0
        || request.width > MAX_ARTWORK_DIMENSION
        || request.height > MAX_ARTWORK_DIMENSION
        || u64::from(request.width) * u64::from(request.height) > MAX_ARTWORK_PIXELS
    {
        return Err(ArtworkError::InvalidRequest);
    }
    if request.path.is_empty() || request.path.len() > MAX_ARTWORK_PATH_BYTES {
        return Err(ArtworkError::InvalidRequest);
    }

    // Plex returns server-relative image paths. Decode repeatedly so layered
    // escapes cannot turn a path that looked local into traversal, a network
    // path, or a query only after the transcoder interprets it.
    let mut path = request.path.clone();
    for _ in 0..=4 {
        if !path.starts_with('/')
            || path.starts_with("//")
            || path.contains('\\')
            || path.contains('?')
            || path.contains('#')
            || path.chars().any(char::is_control)
            || path
                .split('/')
                .any(|segment| segment == "." || segment == "..")
        {
            return Err(ArtworkError::InvalidRequest);
        }
        if !path.contains('%') {
            return Ok(());
        }
        let decoded = decode_percent_once(&path)?;
        if decoded == path {
            return Ok(());
        }
        path = decoded;
    }
    Err(ArtworkError::InvalidRequest)
}

/// Build the only Plex artwork reference that may cross into the frontend.
///
/// This is deliberately a marker rather than a directly navigable URL. Tauri's
/// `convertFileSrc` emits `vela-artwork://localhost/...` on Unix/macOS and
/// `http://vela-artwork.localhost/...` on Windows, so the frontend must perform
/// that conversion instead of assuming one platform's protocol spelling.
pub(crate) fn plex_artwork_url(
    source_id: &str,
    path: &str,
    width: u32,
    height: u32,
) -> Option<String> {
    if source_id.is_empty() || source_id.len() > 256 || source_id.contains(':') {
        return None;
    }
    let request = ArtworkRequest {
        path: path.to_string(),
        width,
        height,
    };
    validate_artwork_request(&request).ok()?;
    Some(format!(
        "{ARTWORK_MARKER_PREFIX}{}.{}.{width}.{height}",
        URL_SAFE_NO_PAD.encode(source_id),
        URL_SAFE_NO_PAD.encode(path)
    ))
}

fn sanitize_legacy_artwork_value(source_id: &str, value: &mut Option<String>) -> bool {
    let Some(current) = value.as_deref() else {
        return false;
    };
    if current.starts_with(ARTWORK_MARKER_PREFIX) {
        return false;
    }
    let Ok(url) = url::Url::parse(current) else {
        return false;
    };
    let query = url.query_pairs().collect::<Vec<_>>();
    if !query
        .iter()
        .any(|(key, _)| key.eq_ignore_ascii_case("X-Plex-Token"))
    {
        return false;
    }

    // Vela 1.0.0-1.0.3 persisted exactly this Plex transcoder shape in recent
    // and playlist snapshots. Convert only a fully validated legacy request.
    // Any other URL that advertises a Plex token is dropped rather than passed
    // to the webview or guessed into a request.
    let replacement = if url.path() == "/photo/:/transcode" {
        let parameter = |name: &str| {
            query
                .iter()
                .find(|(key, _)| key.eq_ignore_ascii_case(name))
                .map(|(_, value)| value.as_ref())
        };
        parameter("url")
            .zip(parameter("width"))
            .zip(parameter("height"))
            .and_then(|((path, width), height)| {
                Some((
                    path,
                    width.parse::<u32>().ok()?,
                    height.parse::<u32>().ok()?,
                ))
            })
            .and_then(|(path, width, height)| plex_artwork_url(source_id, path, width, height))
    } else {
        None
    };
    *value = replacement;
    true
}

/// Remove or convert Plex token URLs saved by Vela versions before 1.0.4.
pub(crate) fn sanitize_item_artwork(item: &mut crate::source::ItemDto) -> bool {
    let mut changed = false;
    changed |= sanitize_legacy_artwork_value(&item.source_id, &mut item.poster);
    changed |= sanitize_legacy_artwork_value(&item.source_id, &mut item.series_poster);
    changed |= sanitize_legacy_artwork_value(&item.source_id, &mut item.backdrop);
    changed
}

fn parse_request(request: &Request<Vec<u8>>) -> Result<(String, ArtworkRequest), ArtworkError> {
    if request.method() != Method::GET
        || request.uri().query().is_some()
        || !request.body().is_empty()
    {
        return Err(ArtworkError::InvalidRequest);
    }
    let payload = request.uri().path().trim_start_matches('/');
    let segments = payload.split('.').collect::<Vec<_>>();
    let [encoded_source, encoded_path, width, height] = segments.as_slice() else {
        return Err(ArtworkError::InvalidRequest);
    };
    let source_id = String::from_utf8(
        URL_SAFE_NO_PAD
            .decode(encoded_source)
            .map_err(|_| ArtworkError::InvalidRequest)?,
    )
    .map_err(|_| ArtworkError::InvalidRequest)?;
    if source_id.is_empty() || source_id.len() > 256 || source_id.contains(':') {
        return Err(ArtworkError::InvalidRequest);
    }
    let path = String::from_utf8(
        URL_SAFE_NO_PAD
            .decode(encoded_path)
            .map_err(|_| ArtworkError::InvalidRequest)?,
    )
    .map_err(|_| ArtworkError::InvalidRequest)?;
    let artwork = ArtworkRequest {
        path,
        width: width.parse().map_err(|_| ArtworkError::InvalidRequest)?,
        height: height.parse().map_err(|_| ArtworkError::InvalidRequest)?,
    };
    validate_artwork_request(&artwork)?;
    Ok((source_id, artwork))
}

fn error_response(error: ArtworkError) -> Response<Vec<u8>> {
    let status = match error {
        ArtworkError::InvalidRequest => StatusCode::BAD_REQUEST,
        ArtworkError::Unsupported => StatusCode::NOT_FOUND,
        ArtworkError::Unavailable | ArtworkError::InvalidResponse => StatusCode::BAD_GATEWAY,
        ArtworkError::TooLarge => StatusCode::PAYLOAD_TOO_LARGE,
    };
    Response::builder()
        .status(status)
        .header(header::CONTENT_TYPE, "text/plain; charset=utf-8")
        .header(header::CACHE_CONTROL, "no-store")
        .header("X-Content-Type-Options", "nosniff")
        .body(b"Artwork unavailable.".to_vec())
        .expect("static artwork error response must be valid")
}

pub(crate) async fn handle_protocol_request(
    state: &AppState,
    request: Request<Vec<u8>>,
) -> Response<Vec<u8>> {
    if crate::durable::ensure_commands_ready().is_err() {
        return error_response(ArtworkError::Unavailable);
    }
    let (source_id, artwork) = match parse_request(&request) {
        Ok(parsed) => parsed,
        Err(error) => return error_response(error),
    };
    let source = {
        let registry = state.registry.lock().await;
        registry.get(&source_id)
    };
    let Some(source) = source else {
        return error_response(ArtworkError::Unsupported);
    };
    let fetched = match source.fetch_artwork(artwork).await {
        Ok(fetched) => fetched,
        Err(error) => return error_response(error),
    };
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, fetched.content_type)
        .header(header::CONTENT_LENGTH, fetched.body.len().to_string())
        .header(header::CACHE_CONTROL, "private, max-age=300")
        .header("X-Content-Type-Options", "nosniff")
        .body(fetched.body)
        .expect("validated artwork response headers must be valid")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn protocol_url_round_trips_without_credentials_or_query() {
        let url = plex_artwork_url("plex/a", "/library/metadata/1/thumb/2", 300, 450)
            .expect("valid artwork URL");
        assert!(url.starts_with(ARTWORK_MARKER_PREFIX));
        assert!(!url.contains('?'));
        assert!(!url.contains("token"));
        let payload = url.strip_prefix(ARTWORK_MARKER_PREFIX).unwrap();
        let request = Request::builder()
            .method(Method::GET)
            .uri(format!("vela-artwork://localhost/{payload}"))
            .body(Vec::new())
            .unwrap();
        let (source, artwork) = parse_request(&request).unwrap();
        assert_eq!(source, "plex/a");
        assert_eq!(artwork.path, "/library/metadata/1/thumb/2");
        assert_eq!((artwork.width, artwork.height), (300, 450));
    }

    #[test]
    fn artwork_request_rejects_traversal_network_paths_and_layered_escapes() {
        for path in [
            "../secret",
            "/a/../secret",
            "/a/%2e%2e/secret",
            "/a/%252e%252e/secret",
            "//example.test/image",
            "/%2f%2fexample.test/image",
            "/safe?url=https://example.test",
            "/safe\\image",
        ] {
            let request = ArtworkRequest {
                path: path.to_string(),
                width: 300,
                height: 450,
            };
            assert_eq!(
                validate_artwork_request(&request),
                Err(ArtworkError::InvalidRequest),
                "{path:?} must fail closed"
            );
        }
    }

    #[test]
    fn artwork_request_bounds_dimensions_and_mime_types() {
        for (width, height) in [(0, 1), (1, 0), (4097, 1), (4096, 4096)] {
            assert_eq!(
                validate_artwork_request(&ArtworkRequest {
                    path: "/library/metadata/1/thumb/2".to_string(),
                    width,
                    height,
                }),
                Err(ArtworkError::InvalidRequest)
            );
        }
        assert_eq!(
            accepted_image_mime("image/jpeg; charset=binary"),
            Some("image/jpeg")
        );
        assert_eq!(accepted_image_mime("image/svg+xml"), None);
        assert_eq!(accepted_image_mime("text/html"), None);
    }

    #[test]
    fn protocol_parser_rejects_query_and_unknown_shape() {
        for uri in [
            "vela-artwork://localhost/a.b.1.1?token=synthetic",
            "vela-artwork://localhost/a.b.1",
            "vela-artwork://localhost/a.b.1.1.extra",
        ] {
            let request = Request::builder()
                .method(Method::GET)
                .uri(uri)
                .body(Vec::new())
                .unwrap();
            assert_eq!(parse_request(&request), Err(ArtworkError::InvalidRequest));
        }
    }

    fn item_with_artwork(artwork: &str) -> crate::source::ItemDto {
        crate::source::ItemDto {
            rating_key: "plex-a:1".to_string(),
            title: "Legacy".to_string(),
            year: None,
            summary: None,
            duration_ms: None,
            media_type: Some("movie".to_string()),
            poster: Some(artwork.to_string()),
            series_poster: None,
            backdrop: None,
            view_offset_ms: None,
            played: None,
            last_watched_at_ms: None,
            added_at_ms: None,
            index: None,
            parent_index: None,
            grandparent_title: None,
            parent_title: None,
            parent_rating_key: None,
            grandparent_rating_key: None,
            source_id: "plex-a".to_string(),
            provider_ids: Vec::new(),
            backing: None,
            canonical_id: None,
            watch_key: None,
            detail_key: None,
        }
    }

    #[test]
    fn legacy_persisted_plex_artwork_is_converted_or_removed() {
        let mut item = item_with_artwork(
            "https://plex.example/photo/:/transcode?width=300&height=450&\
             minSize=1&url=%2Flibrary%2Fmetadata%2F1%2Fthumb%2F2&\
             X-Plex-Token=synthetic-old-token",
        );
        assert!(sanitize_item_artwork(&mut item));
        let poster = item.poster.unwrap();
        assert!(poster.starts_with(ARTWORK_MARKER_PREFIX));
        assert!(!poster.contains("synthetic-old-token"));

        let mut unsafe_item = item_with_artwork(
            "https://plex.example/unexpected?X%2dPlex%2dToken=synthetic-old-token",
        );
        assert!(sanitize_item_artwork(&mut unsafe_item));
        assert_eq!(unsafe_item.poster, None);

        let mut ordinary = item_with_artwork("https://example.test/poster.jpg?size=300");
        assert!(!sanitize_item_artwork(&mut ordinary));
        assert_eq!(
            ordinary.poster.as_deref(),
            Some("https://example.test/poster.jpg?size=300")
        );
    }
}
