mod spotify_auth;

use std::sync::Mutex;
use tauri::State;

struct AppState {
    client_id: String,
    tokens: Mutex<Option<spotify_auth::TokenResponse>>,
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

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(AppState {
            client_id: "YOUR_CLIENT_ID_HERE".to_string(),
            tokens: Mutex::new(spotify_auth::load_tokens().unwrap_or(None)),
        })
        .invoke_handler(tauri::generate_handler![greet, spotify_login])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
