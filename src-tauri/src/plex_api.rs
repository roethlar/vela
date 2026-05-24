use reqwest::Client;
use std::collections::HashMap;

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
    params.insert("X-Plex-Token", auth_token.to_string());
    params.insert("identifier", "com.plexapp.plugins.library".to_string());

    let _response = client
        .get(&url)
        .query(&params)
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
    params.insert("X-Plex-Token", auth_token.to_string());
    params.insert("X-Plex-Client-Identifier", client_identifier.to_string());
    params.insert("X-Plex-Product", "Vela".to_string());
    params.insert("X-Plex-Version", "0.1".to_string());
    params.insert("X-Plex-Platform", crate::platform_name().to_string());
    params.insert("X-Plex-Device", crate::platform_name().to_string());
    params.insert("X-Plex-Device-Name", "Vela".to_string());

    let _response = client
        .get(&url)
        .query(&params)
        .send()
        .await?
        .error_for_status()?;

    Ok(())
}
