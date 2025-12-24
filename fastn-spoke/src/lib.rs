//! Spoke client for fastn P2P network
//!
//! A Spoke connects to hubs and accesses koshas.
//! See README.md for full documentation.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use thiserror::Error;

pub use fastn_net::{PublicKey, SecretKey};

/// Error types for spoke operations
#[derive(Error, Debug)]
pub enum Error {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Network error: {0}")]
    Net(#[from] fastn_net::Error),

    #[error("Spoke not initialized. Run 'fastn-spoke init' first.")]
    NotInitialized,

    #[error("Hub not found: {0}")]
    HubNotFound(String),

    #[error("Connection failed: {0}")]
    ConnectionFailed(String),

    #[error("Hub error: {0}")]
    Hub(String),

    #[error("Invalid ID52: {0}")]
    InvalidId52(String),
}

pub type Result<T> = std::result::Result<T, Error>;

/// Spoke configuration stored in config.json
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpokeConfig {
    pub spoke_id52: String,
    pub created_at: DateTime<Utc>,
}

/// A known hub entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KnownHub {
    pub id52: String,
    pub alias: Option<String>,
    pub added_at: DateTime<Utc>,
}

/// Hubs configuration stored in hubs.json
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct HubsConfig {
    pub hubs: Vec<KnownHub>,
}

/// The Spoke client
pub struct Spoke {
    /// Path to SPOKE_HOME
    home: PathBuf,
    /// Spoke's secret key
    secret_key: SecretKey,
    /// Configuration
    config: SpokeConfig,
    /// Known hubs
    hubs: HubsConfig,
}

impl Spoke {
    /// Get the SPOKE_HOME directory
    pub fn home_dir() -> PathBuf {
        if let Ok(home) = std::env::var("SPOKE_HOME") {
            PathBuf::from(home)
        } else {
            directories::ProjectDirs::from("com", "fastn", "fastn-spoke")
                .map(|p| p.data_dir().to_path_buf())
                .unwrap_or_else(|| {
                    dirs::home_dir()
                        .unwrap_or_else(|| PathBuf::from("."))
                        .join(".fastn-spoke")
                })
        }
    }

    /// Check if spoke is initialized
    pub fn is_initialized() -> bool {
        Self::home_dir().join("spoke.key").exists()
    }

    /// Get the spoke's ID52
    pub fn id52(&self) -> &str {
        &self.config.spoke_id52
    }

    /// Get home directory
    pub fn home(&self) -> &PathBuf {
        &self.home
    }

    // Stub implementations - to be filled in

    /// Initialize a new spoke
    pub async fn init() -> Result<Self> {
        todo!("Spoke::init")
    }

    /// Load an existing spoke
    pub async fn load() -> Result<Self> {
        todo!("Spoke::load")
    }

    /// Load or initialize spoke
    pub async fn load_or_init() -> Result<Self> {
        if Self::is_initialized() {
            Self::load().await
        } else {
            Self::init().await
        }
    }

    /// Add a hub to known hubs
    pub async fn add_hub(&mut self, _id52: &str, _alias: Option<&str>) -> Result<()> {
        todo!("Spoke::add_hub")
    }

    /// Remove a hub from known hubs
    pub async fn remove_hub(&mut self, _id52_or_alias: &str) -> Result<()> {
        todo!("Spoke::remove_hub")
    }

    /// List known hubs
    pub fn list_hubs(&self) -> &[KnownHub] {
        &self.hubs.hubs
    }

    /// Find a hub by ID52 or alias
    pub fn find_hub(&self, id52_or_alias: &str) -> Option<&KnownHub> {
        self.hubs.hubs.iter().find(|h| {
            h.id52 == id52_or_alias || h.alias.as_deref() == Some(id52_or_alias)
        })
    }

    /// Connect to a hub
    pub async fn connect(&self, _hub_id52: &str) -> Result<HubConnection> {
        todo!("Spoke::connect")
    }
}

/// An active connection to a hub
pub struct HubConnection {
    /// The hub's ID52
    hub_id52: String,
    /// The underlying spoke connection
    spoke: fastn_net::Spoke,
}

impl HubConnection {
    /// Get the hub's ID52
    pub fn hub_id52(&self) -> &str {
        &self.hub_id52
    }

    /// Send a raw request to the hub
    /// Returns the response payload as JSON
    pub async fn send_request(
        &self,
        _app: &str,
        _instance: &str,
        _command: &str,
        _payload: serde_json::Value,
    ) -> Result<serde_json::Value> {
        todo!("HubConnection::send_request")
    }

    // Kosha file operations (convenience wrappers around send_request)

    /// Read a file from a kosha
    /// Returns: { content: base64 }
    pub async fn read_file(&self, kosha: &str, path: &str) -> Result<serde_json::Value> {
        self.send_request(
            "kosha",
            kosha,
            "read_file",
            serde_json::json!({ "path": path }),
        )
        .await
    }

    /// Write a file to a kosha
    /// Returns: { modified: timestamp }
    pub async fn write_file(
        &self,
        kosha: &str,
        path: &str,
        content_base64: &str,
        base_version: Option<&str>,
    ) -> Result<serde_json::Value> {
        let mut payload = serde_json::json!({
            "path": path,
            "content": content_base64,
        });
        if let Some(bv) = base_version {
            payload["base_version"] = serde_json::Value::String(bv.to_string());
        }
        self.send_request("kosha", kosha, "write_file", payload).await
    }

    /// List directory contents
    /// Returns: { entries: [...] }
    pub async fn list_dir(&self, kosha: &str, path: &str) -> Result<serde_json::Value> {
        self.send_request(
            "kosha",
            kosha,
            "list_dir",
            serde_json::json!({ "path": path }),
        )
        .await
    }

    /// Get file versions
    /// Returns: { versions: [...] }
    pub async fn get_versions(&self, kosha: &str, path: &str) -> Result<serde_json::Value> {
        self.send_request(
            "kosha",
            kosha,
            "get_versions",
            serde_json::json!({ "path": path }),
        )
        .await
    }

    /// Read a specific file version
    /// Returns: { content: base64 }
    pub async fn read_version(
        &self,
        kosha: &str,
        path: &str,
        timestamp: &str,
    ) -> Result<serde_json::Value> {
        self.send_request(
            "kosha",
            kosha,
            "read_version",
            serde_json::json!({ "path": path, "timestamp": timestamp }),
        )
        .await
    }

    /// Rename a file
    /// Returns: {}
    pub async fn rename(&self, kosha: &str, from: &str, to: &str) -> Result<serde_json::Value> {
        self.send_request(
            "kosha",
            kosha,
            "rename",
            serde_json::json!({ "from": from, "to": to }),
        )
        .await
    }

    /// Delete a file
    /// Returns: {}
    pub async fn delete(&self, kosha: &str, path: &str) -> Result<serde_json::Value> {
        self.send_request(
            "kosha",
            kosha,
            "delete",
            serde_json::json!({ "path": path }),
        )
        .await
    }

    // KV operations

    /// Get a value from the KV store
    /// Returns: { value: json | null }
    pub async fn kv_get(&self, kosha: &str, key: &str) -> Result<serde_json::Value> {
        self.send_request(
            "kosha",
            kosha,
            "kv_get",
            serde_json::json!({ "key": key }),
        )
        .await
    }

    /// Set a value in the KV store
    /// Returns: {}
    pub async fn kv_set(
        &self,
        kosha: &str,
        key: &str,
        value: serde_json::Value,
    ) -> Result<serde_json::Value> {
        self.send_request(
            "kosha",
            kosha,
            "kv_set",
            serde_json::json!({ "key": key, "value": value }),
        )
        .await
    }

    /// Delete a key from the KV store
    /// Returns: {}
    pub async fn kv_delete(&self, kosha: &str, key: &str) -> Result<serde_json::Value> {
        self.send_request(
            "kosha",
            kosha,
            "kv_delete",
            serde_json::json!({ "key": key }),
        )
        .await
    }
}
