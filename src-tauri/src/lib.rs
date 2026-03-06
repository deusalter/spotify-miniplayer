mod spotify_api;
mod spotify_auth;

use std::sync::Mutex;
use tauri::State;

struct AppState {
    client_id: String,
    tokens: Mutex<Option<spotify_auth::TokenResponse>>,
}

fn get_access_token(state: &State<'_, AppState>) -> Result<String, String> {
    let tokens = state.tokens.lock().unwrap();
    tokens
        .as_ref()
        .map(|t| t.access_token.clone())
        .ok_or_else(|| "Not logged in".to_string())
}

#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

#[tauri::command]
async fn spotify_login(state: State<'_, AppState>) -> Result<String, String> {
    let (code, verifier) = spotify_auth::start_auth_flow(&state.client_id).await?;
    let tokens = spotify_auth::exchange_code(&state.client_id, &code, &verifier).await?;
    spotify_auth::save_tokens(&tokens)?;
    *state.tokens.lock().unwrap() = Some(tokens);
    Ok("Login successful".to_string())
}

#[tauri::command]
async fn get_playback(state: State<'_, AppState>) -> Result<Option<spotify_api::PlaybackState>, String> {
    let token = get_access_token(&state)?;
    spotify_api::get_playback_state(&token).await
}

#[tauri::command]
async fn play_pause(state: State<'_, AppState>) -> Result<(), String> {
    let token = get_access_token(&state)?;
    let playback = spotify_api::get_playback_state(&token).await?;
    match playback {
        Some(ps) if ps.is_playing => spotify_api::pause(&token).await,
        _ => spotify_api::play(&token).await,
    }
}

#[tauri::command]
async fn next_track(state: State<'_, AppState>) -> Result<(), String> {
    let token = get_access_token(&state)?;
    spotify_api::next_track(&token).await
}

#[tauri::command]
async fn previous_track(state: State<'_, AppState>) -> Result<(), String> {
    let token = get_access_token(&state)?;
    spotify_api::previous_track(&token).await
}

#[tauri::command]
async fn seek_to(state: State<'_, AppState>, position_ms: u64) -> Result<(), String> {
    let token = get_access_token(&state)?;
    spotify_api::seek_to(&token, position_ms).await
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(AppState {
            client_id: "YOUR_CLIENT_ID_HERE".to_string(),
            tokens: Mutex::new(spotify_auth::load_tokens().unwrap_or(None)),
        })
        .invoke_handler(tauri::generate_handler![
            greet,
            spotify_login,
            get_playback,
            play_pause,
            next_track,
            previous_track,
            seek_to
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
