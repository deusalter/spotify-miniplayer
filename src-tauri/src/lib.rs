mod spotify_api;
mod spotify_auth;

#[cfg(target_os = "macos")]
#[macro_use]
extern crate objc;

use std::sync::Mutex;
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{LogicalSize, State, Manager};

struct AppState {
    client_id: String,
    tokens: Mutex<Option<spotify_auth::TokenResponse>>,
    pre_fullscreen: Mutex<Option<(u32, u32, i32, i32)>>,
}

fn get_access_token(state: &State<'_, AppState>) -> Result<String, String> {
    let tokens = state.tokens.lock().unwrap();
    tokens
        .as_ref()
        .map(|t| t.access_token.clone())
        .ok_or_else(|| "Not logged in".to_string())
}

async fn refresh_if_expired(state: &State<'_, AppState>) -> Result<String, String> {
    let (client_id, refresh_tok) = {
        let tokens = state.tokens.lock().unwrap();
        let t = tokens.as_ref().ok_or("Not logged in")?;
        (state.client_id.clone(), t.refresh_token.clone())
    };
    let new_tokens = spotify_auth::refresh_token(&client_id, &refresh_tok).await?;
    spotify_auth::save_tokens(&new_tokens).ok();
    let access = new_tokens.access_token.clone();
    *state.tokens.lock().unwrap() = Some(new_tokens);
    Ok(access)
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
    match spotify_api::get_playback_state(&token).await {
        Err(e) if e.contains("Token expired") => {
            let new_token = refresh_if_expired(&state).await?;
            spotify_api::get_playback_state(&new_token).await
        }
        other => other,
    }
}

#[tauri::command]
async fn play_pause(state: State<'_, AppState>) -> Result<(), String> {
    let token = get_access_token(&state)?;
    let result = async {
        let playback = spotify_api::get_playback_state(&token).await?;
        match playback {
            Some(ps) if ps.is_playing => spotify_api::pause(&token).await,
            _ => spotify_api::play(&token).await,
        }
    }.await;
    match result {
        Err(e) if e.contains("Token expired") => {
            let new_token = refresh_if_expired(&state).await?;
            let playback = spotify_api::get_playback_state(&new_token).await?;
            match playback {
                Some(ps) if ps.is_playing => spotify_api::pause(&new_token).await,
                _ => spotify_api::play(&new_token).await,
            }
        }
        other => other,
    }
}

#[tauri::command]
async fn next_track(state: State<'_, AppState>) -> Result<(), String> {
    let token = get_access_token(&state)?;
    match spotify_api::next_track(&token).await {
        Err(e) if e.contains("Token expired") => {
            let new_token = refresh_if_expired(&state).await?;
            spotify_api::next_track(&new_token).await
        }
        other => other,
    }
}

#[tauri::command]
async fn previous_track(state: State<'_, AppState>) -> Result<(), String> {
    let token = get_access_token(&state)?;
    match spotify_api::previous_track(&token).await {
        Err(e) if e.contains("Token expired") => {
            let new_token = refresh_if_expired(&state).await?;
            spotify_api::previous_track(&new_token).await
        }
        other => other,
    }
}

#[tauri::command]
async fn seek_to(state: State<'_, AppState>, position_ms: u64) -> Result<(), String> {
    let token = get_access_token(&state)?;
    match spotify_api::seek_to(&token, position_ms).await {
        Err(e) if e.contains("Token expired") => {
            let new_token = refresh_if_expired(&state).await?;
            spotify_api::seek_to(&new_token, position_ms).await
        }
        other => other,
    }
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

#[tauri::command]
async fn enter_fullscreen(app: tauri::AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("main") {
        let size = window.outer_size().map_err(|e| e.to_string())?;
        let pos = window.outer_position().map_err(|e| e.to_string())?;

        let state = app.state::<AppState>();
        *state.pre_fullscreen.lock().unwrap() = Some((
            size.width, size.height,
            pos.x, pos.y,
        ));

        window.navigate("fullscreen.html".parse().unwrap()).map_err(|e| e.to_string())?;
        window.set_fullscreen(true).map_err(|e| e.to_string())?;
        window.set_always_on_top(false).map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
async fn exit_fullscreen(app: tauri::AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("main") {
        window.set_fullscreen(false).map_err(|e| e.to_string())?;

        let state = app.state::<AppState>();
        if let Some((w, h, x, y)) = *state.pre_fullscreen.lock().unwrap() {
            window.set_size(tauri::PhysicalSize::new(w, h)).map_err(|e| e.to_string())?;
            window.set_position(tauri::PhysicalPosition::new(x, y)).map_err(|e| e.to_string())?;
        }

        window.set_always_on_top(true).map_err(|e| e.to_string())?;
        window.navigate("index.html".parse().unwrap()).map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(AppState {
            client_id: "719f70b25a4b4162814f4906e2cd9eb2".to_string(),
            tokens: Mutex::new(spotify_auth::load_tokens().unwrap_or(None)),
            pre_fullscreen: Mutex::new(None),
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
            load_window_position,
            enter_fullscreen,
            exit_fullscreen
        ])
        .setup(|app| {
            let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
            let show_hide =
                MenuItem::with_id(app, "show_hide", "Show/Hide", true, None::<&str>)?;
            let login =
                MenuItem::with_id(app, "login", "Login to Spotify", true, None::<&str>)?;
            let sep1 = PredefinedMenuItem::separator(app)?;
            let sep2 = PredefinedMenuItem::separator(app)?;
            let layout_horizontal =
                MenuItem::with_id(app, "layout_horizontal", "↔ Horizontal", true, None::<&str>)?;
            let layout_vertical =
                MenuItem::with_id(app, "layout_vertical", "↕ Vertical", true, None::<&str>)?;
            let layout_compact =
                MenuItem::with_id(app, "layout_compact", "⊡ Compact", true, None::<&str>)?;

            let menu = Menu::with_items(app, &[
                &show_hide, &login, &sep1,
                &layout_horizontal, &layout_vertical, &layout_compact, &sep2,
                &quit,
            ])?;

            let _tray = TrayIconBuilder::new()
                .icon(app.default_window_icon().cloned().unwrap())
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
                    "layout_horizontal" => {
                        if let Some(window) = app.get_webview_window("main") {
                            let _ = window.set_size(LogicalSize::new(280.0, 160.0));
                        }
                    }
                    "layout_vertical" => {
                        if let Some(window) = app.get_webview_window("main") {
                            let _ = window.set_size(LogicalSize::new(180.0, 260.0));
                        }
                    }
                    "layout_compact" => {
                        if let Some(window) = app.get_webview_window("main") {
                            let _ = window.set_size(LogicalSize::new(240.0, 130.0));
                        }
                    }
                    _ => {}
                })
                .tooltip("Spotify Mini Player")
                .build(app)?;

            // Make NSWindow background fully transparent
            #[cfg(target_os = "macos")]
            {
                use cocoa::appkit::NSWindow;
                use cocoa::base::id;

                if let Some(window) = app.get_webview_window("main") {
                    let ns_window = window.ns_window().unwrap() as id;
                    unsafe {
                        let clear_color: id = msg_send![class!(NSColor), colorWithRed:0.0f64 green:0.0f64 blue:0.0f64 alpha:0.0f64];
                        ns_window.setBackgroundColor_(clear_color);
                    }
                }
            }

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
