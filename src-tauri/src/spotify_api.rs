use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PlaybackState {
    pub is_playing: bool,
    pub track_name: String,
    pub artist_name: String,
    pub album_art_url: String,
    pub progress_ms: u64,
    pub duration_ms: u64,
}

/// Fetches the current playback state from Spotify.
///
/// Returns `Ok(None)` if no active device is found (204 No Content).
/// Returns `Ok(Some(PlaybackState))` on success.
/// Returns `Err` on authentication failure, rate limiting, or other errors.
pub async fn get_playback_state(access_token: &str) -> Result<Option<PlaybackState>, String> {
    let client = reqwest::Client::new();

    let response = client
        .get("https://api.spotify.com/v1/me/player")
        .header("Authorization", format!("Bearer {}", access_token))
        .send()
        .await
        .map_err(|e| format!("Failed to fetch playback state: {}", e))?;

    let status = response.status();

    // 204 No Content - no active device
    if status.as_u16() == 204 {
        return Ok(None);
    }

    // 401 Unauthorized - token expired
    if status.as_u16() == 401 {
        return Err("Token expired".to_string());
    }

    // 429 Rate Limited
    if status.as_u16() == 429 {
        return Err("Rate limited".to_string());
    }

    // Other non-success status codes
    if !status.is_success() {
        let body = response
            .text()
            .await
            .unwrap_or_else(|_| "Unable to read response body".to_string());
        return Err(format!(
            "Spotify API error (status {}): {}",
            status, body
        ));
    }

    // 200 OK - parse JSON response
    let body: serde_json::Value = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse playback response JSON: {}", e))?;

    let is_playing = body["is_playing"].as_bool().unwrap_or(false);

    let item = &body["item"];

    let track_name = item["name"]
        .as_str()
        .unwrap_or("Unknown Track")
        .to_string();

    let artist_name = item["artists"]
        .as_array()
        .and_then(|artists| artists.first())
        .and_then(|artist| artist["name"].as_str())
        .unwrap_or("Unknown Artist")
        .to_string();

    // Prefer index 1 (300x300) if available, fall back to index 0 (640x640)
    let album_art_url = item["album"]["images"]
        .as_array()
        .and_then(|images| {
            images
                .get(1)
                .or_else(|| images.first())
        })
        .and_then(|image| image["url"].as_str())
        .unwrap_or("")
        .to_string();

    let progress_ms = body["progress_ms"].as_u64().unwrap_or(0);

    let duration_ms = item["duration_ms"].as_u64().unwrap_or(0);

    Ok(Some(PlaybackState {
        is_playing,
        track_name,
        artist_name,
        album_art_url,
        progress_ms,
        duration_ms,
    }))
}

/// Resumes playback on the current device.
pub async fn play(access_token: &str) -> Result<(), String> {
    send_playback_command(
        access_token,
        reqwest::Method::PUT,
        "https://api.spotify.com/v1/me/player/play",
    )
    .await
}

/// Pauses playback on the current device.
pub async fn pause(access_token: &str) -> Result<(), String> {
    send_playback_command(
        access_token,
        reqwest::Method::PUT,
        "https://api.spotify.com/v1/me/player/pause",
    )
    .await
}

/// Skips to the next track.
pub async fn next_track(access_token: &str) -> Result<(), String> {
    send_playback_command(
        access_token,
        reqwest::Method::POST,
        "https://api.spotify.com/v1/me/player/next",
    )
    .await
}

/// Skips to the previous track.
pub async fn previous_track(access_token: &str) -> Result<(), String> {
    send_playback_command(
        access_token,
        reqwest::Method::POST,
        "https://api.spotify.com/v1/me/player/previous",
    )
    .await
}

/// Seeks to a specific position in the current track.
pub async fn seek_to(access_token: &str, position_ms: u64) -> Result<(), String> {
    let url = format!(
        "https://api.spotify.com/v1/me/player/seek?position_ms={}",
        position_ms
    );
    send_playback_command(access_token, reqwest::Method::PUT, &url).await
}

/// Helper function to send a playback control command to Spotify.
async fn send_playback_command(
    access_token: &str,
    method: reqwest::Method,
    url: &str,
) -> Result<(), String> {
    let client = reqwest::Client::new();

    let response = client
        .request(method, url)
        .header("Authorization", format!("Bearer {}", access_token))
        .header("Content-Length", "0")
        .send()
        .await
        .map_err(|e| format!("Failed to send playback command: {}", e))?;

    let status = response.status();

    if status.as_u16() == 401 {
        return Err("Token expired".to_string());
    }

    if status.as_u16() == 429 {
        return Err("Rate limited".to_string());
    }

    if !status.is_success() {
        let body = response
            .text()
            .await
            .unwrap_or_else(|_| "Unable to read response body".to_string());
        return Err(format!(
            "Spotify API error (status {}): {}",
            status, body
        ));
    }

    Ok(())
}
