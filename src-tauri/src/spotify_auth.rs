use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use rand::Rng;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::PathBuf;

const REDIRECT_URI: &str = "http://127.0.0.1:8888/callback";
const SCOPES: &str = "user-read-playback-state user-modify-playback-state user-read-currently-playing";
const TOKEN_DIR: &str = ".spotify-miniplayer";
const TOKEN_FILE: &str = "tokens.json";

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TokenResponse {
    pub access_token: String,
    pub refresh_token: String,
    pub expires_in: u64,
    pub token_type: String,
}

/// Generates a PKCE code_verifier and code_challenge pair.
/// Returns (code_verifier, code_challenge).
fn generate_pkce_pair() -> (String, String) {
    let mut rng = rand::thread_rng();
    let mut random_bytes = [0u8; 32];
    rng.fill(&mut random_bytes);

    let code_verifier = URL_SAFE_NO_PAD.encode(random_bytes);

    let mut hasher = Sha256::new();
    hasher.update(code_verifier.as_bytes());
    let hash = hasher.finalize();
    let code_challenge = URL_SAFE_NO_PAD.encode(hash);

    (code_verifier, code_challenge)
}

/// Starts the Spotify OAuth PKCE auth flow.
/// Opens the user's browser to the Spotify authorization page, then listens
/// on a local HTTP server for the callback with the authorization code.
/// Returns (authorization_code, code_verifier) on success.
pub async fn start_auth_flow(client_id: &str) -> Result<(String, String), String> {
    let (code_verifier, code_challenge) = generate_pkce_pair();

    let auth_url = format!(
        "https://accounts.spotify.com/authorize?client_id={}&response_type=code&redirect_uri={}&scope={}&code_challenge_method=S256&code_challenge={}",
        urlencoding::encode(client_id),
        urlencoding::encode(REDIRECT_URI),
        urlencoding::encode(SCOPES),
        urlencoding::encode(&code_challenge),
    );

    // Open the authorization URL in the user's default browser
    open::that(&auth_url).map_err(|e| format!("Failed to open browser: {}", e))?;

    // Start a local HTTP server to listen for the callback
    let server = tiny_http::Server::http("127.0.0.1:8888")
        .map_err(|e| format!("Failed to start local HTTP server on port 8888: {}", e))?;

    // Wait for the callback request (blocking, with a timeout via incoming_requests)
    let request = server
        .recv()
        .map_err(|e| format!("Failed to receive callback request: {}", e))?;

    let url = request.url().to_string();

    // Extract the authorization code from the query string
    let code = extract_query_param(&url, "code");
    let error = extract_query_param(&url, "error");

    if let Some(err) = error {
        // Respond to the browser with an error message
        let response = tiny_http::Response::from_string(format!(
            "<html><body><h1>Authentication Failed</h1><p>Error: {}</p><p>You can close this window.</p></body></html>",
            err
        ))
        .with_header(
            "Content-Type: text/html"
                .parse::<tiny_http::Header>()
                .unwrap(),
        );
        let _ = request.respond(response);
        return Err(format!("Spotify authorization error: {}", err));
    }

    let code = code.ok_or_else(|| {
        "No authorization code found in callback URL. The user may have denied access.".to_string()
    })?;

    // Respond to the browser with a success message
    let response = tiny_http::Response::from_string(
        "<html><body><h1>Authentication Successful!</h1><p>You can close this window and return to Spotify Mini Player.</p></body></html>",
    )
    .with_header(
        "Content-Type: text/html"
            .parse::<tiny_http::Header>()
            .unwrap(),
    );
    let _ = request.respond(response);

    Ok((code, code_verifier))
}

/// Exchanges an authorization code for access and refresh tokens.
pub async fn exchange_code(
    client_id: &str,
    code: &str,
    code_verifier: &str,
) -> Result<TokenResponse, String> {
    let client = reqwest::Client::new();

    let params = [
        ("grant_type", "authorization_code"),
        ("code", code),
        ("redirect_uri", REDIRECT_URI),
        ("client_id", client_id),
        ("code_verifier", code_verifier),
    ];

    let response = client
        .post("https://accounts.spotify.com/api/token")
        .form(&params)
        .send()
        .await
        .map_err(|e| format!("Failed to send token exchange request: {}", e))?;

    let status = response.status();
    if !status.is_success() {
        let body = response
            .text()
            .await
            .unwrap_or_else(|_| "Unable to read response body".to_string());
        return Err(format!(
            "Token exchange failed with status {}: {}",
            status, body
        ));
    }

    let token_response: TokenResponse = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse token response: {}", e))?;

    Ok(token_response)
}

/// Refreshes an access token using the refresh token.
pub async fn refresh_token(
    client_id: &str,
    refresh_token: &str,
) -> Result<TokenResponse, String> {
    let client = reqwest::Client::new();

    let params = [
        ("grant_type", "refresh_token"),
        ("refresh_token", refresh_token),
        ("client_id", client_id),
    ];

    let response = client
        .post("https://accounts.spotify.com/api/token")
        .form(&params)
        .send()
        .await
        .map_err(|e| format!("Failed to send token refresh request: {}", e))?;

    let status = response.status();
    if !status.is_success() {
        let body = response
            .text()
            .await
            .unwrap_or_else(|_| "Unable to read response body".to_string());
        return Err(format!(
            "Token refresh failed with status {}: {}",
            status, body
        ));
    }

    let token_response: TokenResponse = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse refresh token response: {}", e))?;

    Ok(token_response)
}

/// Returns the path to the token storage directory (~/.spotify-miniplayer/).
fn token_dir_path() -> Result<PathBuf, String> {
    let home = dirs::home_dir().ok_or_else(|| "Unable to determine home directory".to_string())?;
    Ok(home.join(TOKEN_DIR))
}

/// Returns the path to the tokens.json file.
fn token_file_path() -> Result<PathBuf, String> {
    Ok(token_dir_path()?.join(TOKEN_FILE))
}

/// Saves tokens to ~/.spotify-miniplayer/tokens.json.
/// Creates the directory if it does not exist.
pub fn save_tokens(tokens: &TokenResponse) -> Result<(), String> {
    let dir = token_dir_path()?;
    if !dir.exists() {
        fs::create_dir_all(&dir)
            .map_err(|e| format!("Failed to create token directory {:?}: {}", dir, e))?;
    }

    let path = token_file_path()?;
    let json = serde_json::to_string_pretty(tokens)
        .map_err(|e| format!("Failed to serialize tokens: {}", e))?;

    fs::write(&path, json).map_err(|e| format!("Failed to write tokens to {:?}: {}", path, e))?;

    Ok(())
}

/// Loads tokens from ~/.spotify-miniplayer/tokens.json.
/// Returns Ok(None) if the file does not exist.
pub fn load_tokens() -> Result<Option<TokenResponse>, String> {
    let path = token_file_path()?;

    if !path.exists() {
        return Ok(None);
    }

    let contents =
        fs::read_to_string(&path).map_err(|e| format!("Failed to read {:?}: {}", path, e))?;

    let tokens: TokenResponse = serde_json::from_str(&contents)
        .map_err(|e| format!("Failed to parse tokens from {:?}: {}", path, e))?;

    Ok(Some(tokens))
}

/// Extracts a query parameter value from a URL path string like "/callback?code=abc&state=xyz".
fn extract_query_param(url: &str, param: &str) -> Option<String> {
    let query_string = url.split('?').nth(1)?;
    for pair in query_string.split('&') {
        let mut kv = pair.splitn(2, '=');
        if let (Some(key), Some(value)) = (kv.next(), kv.next()) {
            if key == param {
                // URL-decode the value
                return Some(urlencoding::decode(value).unwrap_or_default().into_owned());
            }
        }
    }
    None
}
