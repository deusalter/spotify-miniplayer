mod spotify_api;
mod spotify_auth;

use std::sync::Mutex;
use tauri::menu::{Menu, MenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::State;
use tauri::Manager;

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

#[tauri::command]
fn save_window_position(x: i32, y: i32) -> Result<(), String> {
    let config_dir = dirs::home_dir()
        .ok_or("Could not find home directory")?
        .join(".spotify-miniplayer");

    std::fs::create_dir_all(&config_dir).map_err(|e| e.to_string())?;

    let config = serde_json::json!({ "x": x, "y": y });
    let config_path = config_dir.join("window_position.json");
    std::fs::write(config_path, serde_json::to_string_pretty(&config).unwrap())
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn load_window_position() -> Result<Option<(i32, i32)>, String> {
    let config_path = dirs::home_dir()
        .ok_or("Could not find home directory")?
        .join(".spotify-miniplayer")
        .join("window_position.json");

    if !config_path.exists() {
        return Ok(None);
    }

    let data = std::fs::read_to_string(config_path).map_err(|e| e.to_string())?;
    let json: serde_json::Value = serde_json::from_str(&data).map_err(|e| e.to_string())?;

    let x = json["x"].as_i64().unwrap_or(100) as i32;
    let y = json["y"].as_i64().unwrap_or(100) as i32;
    Ok(Some((x, y)))
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
            seek_to,
            save_window_position,
            load_window_position
        ])
        .setup(|app| {
            let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
            let show_hide =
                MenuItem::with_id(app, "show_hide", "Show/Hide", true, None::<&str>)?;
            let login =
                MenuItem::with_id(app, "login", "Login to Spotify", true, None::<&str>)?;

            let menu = Menu::with_items(app, &[&show_hide, &login, &quit])?;

            let _tray = TrayIconBuilder::new()
                .menu(&menu)
                .on_menu_event(move |app, event| match event.id().as_ref() {
                    "quit" => {
                        app.exit(0);
                    }
                    "show_hide" => {
                        if let Some(window) = app.get_webview_window("main") {
                            if window.is_visible().unwrap_or(false) {
                                let _ = window.hide();
                            } else {
                                let _ = window.show();
                                let _ = window.set_focus();
                            }
                        }
                    }
                    "login" => {
                        let app_handle = app.clone();
                        tauri::async_runtime::spawn(async move {
                            let state = app_handle.state::<AppState>();
                            match spotify_auth::start_auth_flow(&state.client_id).await {
                                Ok((code, verifier)) => {
                                    match spotify_auth::exchange_code(
                                        &state.client_id,
                                        &code,
                                        &verifier,
                                    )
                                    .await
                                    {
                                        Ok(tokens) => {
                                            let _ = spotify_auth::save_tokens(&tokens);
                                            *state.tokens.lock().unwrap() = Some(tokens);
                                        }
                                        Err(e) => eprintln!("Token exchange failed: {}", e),
                                    }
                                }
                                Err(e) => eprintln!("Auth flow failed: {}", e),
                            }
                        });
                    }
                    _ => {}
                })
                .tooltip("Spotify Mini Player")
                .build(app)?;

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
