//! Tauri GUI module for fastn-spoke
//!
//! Provides Tauri commands for the frontend to:
//! - Get spoke configuration
//! - Fetch WASM files from the hub kosha

use crate::Spoke;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Mutex;

/// Spoke configuration returned to the frontend
#[derive(Debug, Serialize, Deserialize)]
pub struct SpokeInfo {
    pub spoke_id52: String,
    pub hub_id52: String,
    pub hub_url: String,
    pub alias: String,
    pub initialized: bool,
}

/// State shared between Tauri commands
pub struct AppState {
    pub home: PathBuf,
    pub spoke: Mutex<Option<Spoke>>,
}

/// Get the spoke configuration
#[tauri::command]
pub async fn get_spoke_info(state: tauri::State<'_, Arc<AppState>>) -> Result<SpokeInfo, String> {
    let mut spoke_guard = state.spoke.lock().await;

    // Try to load spoke if not already loaded
    if spoke_guard.is_none() {
        match Spoke::load(&state.home).await {
            Ok(spoke) => {
                *spoke_guard = Some(spoke);
            }
            Err(_) => {
                return Ok(SpokeInfo {
                    spoke_id52: String::new(),
                    hub_id52: String::new(),
                    hub_url: String::new(),
                    alias: String::new(),
                    initialized: false,
                });
            }
        }
    }

    let spoke = spoke_guard.as_ref().unwrap();
    Ok(SpokeInfo {
        spoke_id52: spoke.id52().to_string(),
        hub_id52: spoke.hub_id52().to_string(),
        hub_url: spoke.hub_url().to_string(),
        alias: spoke.alias().to_string(),
        initialized: true,
    })
}

/// Fetch a file from the hub kosha and return as base64
/// The frontend will decode this to get the raw WASM bytes
#[tauri::command]
pub async fn fetch_kosha_file(
    state: tauri::State<'_, Arc<AppState>>,
    kosha: String,
    path: String,
) -> Result<String, String> {
    let spoke_guard = state.spoke.lock().await;

    let spoke = spoke_guard
        .as_ref()
        .ok_or_else(|| "Spoke not initialized".to_string())?;

    // Create connection to hub
    let conn = spoke.connect();

    // Read the file from the hub
    let response = conn
        .read_file("self", &kosha, &path)
        .await
        .map_err(|e| format!("Failed to read file: {}", e))?;

    // The response contains base64-encoded content
    response
        .get("content")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| "Invalid response: missing content field".to_string())
}

/// Build and run the Tauri application
pub fn run(home: PathBuf) {
    let state = Arc::new(AppState {
        home,
        spoke: Mutex::new(None),
    });

    tauri::Builder::default()
        .manage(state)
        .invoke_handler(tauri::generate_handler![
            get_spoke_info,
            fetch_kosha_file,
        ])
        .run(tauri::generate_context!())
        .expect("Failed to run Tauri application");
}
