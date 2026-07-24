use reqwest::Client;
use std::collections::HashMap;

const VERSION: &str = env!("CARGO_PKG_VERSION");

pub async fn update_progress(
    client: &Client,
    server_url: &str,
    auth_token: &str,
    client_identifier: &str,
    rating_key: &str,
    time: u64,
) -> Result<(), Box<dyn std::error::Error>> {
    let url = format!("{}/:/progress", server_url);
    let mut params = HashMap::new();
    // Plex expects `key` for the metadata rating key, plus identifier
    params.insert("key", rating_key.to_string());
    params.insert("time", time.to_string());
    params.insert("identifier", "com.plexapp.plugins.library".to_string());

    let _response = client
        .get(&url)
        .query(&params)
        .header("X-Plex-Token", auth_token)
        .header("X-Plex-Client-Identifier", client_identifier)
        .header("X-Plex-Product", "Vela")
        .send()
        .await?
        .error_for_status()?;

    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub async fn update_timeline(
    client: &Client,
    server_url: &str,
    auth_token: &str,
    client_identifier: &str,
    rating_key: &str,
    key: &str,
    state: &str,
    time: u64,
    duration: u64,
) -> Result<(), Box<dyn std::error::Error>> {
    let url = format!("{}/:/timeline", server_url);
    let mut params = HashMap::new();
    params.insert("ratingKey", rating_key.to_string());
    params.insert("key", key.to_string());
    params.insert("state", state.to_string());
    params.insert("time", time.to_string());
    params.insert("duration", duration.to_string());
    params.insert("type", "video".to_string());

    let _response = client
        .get(&url)
        .query(&params)
        .header("X-Plex-Token", auth_token)
        .header("X-Plex-Client-Identifier", client_identifier)
        .header("X-Plex-Product", "Vela")
        .header("X-Plex-Version", VERSION)
        .header("X-Plex-Platform", crate::platform_name())
        .header("X-Plex-Device", crate::platform_name())
        .header("X-Plex-Device-Name", "Vela")
        .send()
        .await?
        .error_for_status()?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    async fn capture_one_request() -> (String, tokio::task::JoinHandle<String>) {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
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
            stream
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n")
                .await
                .unwrap();
            String::from_utf8(bytes).unwrap()
        });
        (format!("http://{address}"), task)
    }

    fn assert_header_auth_without_query_token(request: &str, endpoint: &str) {
        let head = request.to_ascii_lowercase();
        let request_line = request.lines().next().unwrap_or_default();
        assert!(request_line.starts_with(&format!("GET /:/{endpoint}?")));
        assert!(!request_line.to_ascii_lowercase().contains("token"));
        assert!(head.contains("\r\nx-plex-token: synthetic-progress-token\r\n"));
        assert!(head.contains("\r\nx-plex-client-identifier: client-1\r\n"));
    }

    #[tokio::test]
    async fn progress_authentication_uses_headers_not_query() {
        let (base, captured) = capture_one_request().await;
        update_progress(
            &Client::new(),
            &base,
            "synthetic-progress-token",
            "client-1",
            "42",
            1234,
        )
        .await
        .unwrap();
        assert_header_auth_without_query_token(&captured.await.unwrap(), "progress");
    }

    #[tokio::test]
    async fn timeline_authentication_uses_headers_not_query() {
        let (base, captured) = capture_one_request().await;
        update_timeline(
            &Client::new(),
            &base,
            "synthetic-progress-token",
            "client-1",
            "42",
            "/library/metadata/42",
            "playing",
            1234,
            5000,
        )
        .await
        .unwrap();
        let request = captured.await.unwrap();
        assert_header_auth_without_query_token(&request, "timeline");
        let lower = request.to_ascii_lowercase();
        assert!(lower.contains("\r\nx-plex-product: vela\r\n"));
        assert!(lower.contains("\r\nx-plex-device-name: vela\r\n"));
    }
}
