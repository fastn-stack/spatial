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

    #[error("Spoke not initialized. Run 'fastn-spoke init <hub-id52>' first.")]
    NotInitialized,

    #[error("Hub not found: {0}")]
    HubNotFound(String),

    #[error("Connection failed: {0}")]
    ConnectionFailed(String),

    #[error("Hub rejected connection (not authorized). Ask hub admin to run: fastn-hub add-spoke {0}")]
    NotAuthorized(String),

    #[error("Hub error: {0}")]
    Hub(String),

    #[error("Invalid ID52: {0}")]
    InvalidId52(String),

    #[error("Spoke already initialized at {0:?}")]
    AlreadyInitialized(PathBuf),
}

pub type Result<T> = std::result::Result<T, Error>;

/// Spoke configuration stored in config.json
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpokeConfig {
    /// The spoke's ID52
    pub spoke_id52: String,
    /// The hub's ID52 this spoke connects to
    pub hub_id52: String,
    /// The hub's HTTP URL (e.g., "http://localhost:3000")
    pub hub_url: String,
    /// Human-readable name/alias for this spoke
    pub alias: String,
    /// When the spoke was created
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
    /// Known hubs (for future multi-hub support)
    #[allow(dead_code)]
    hubs: HubsConfig,
}

impl Spoke {
    /// Get the default home directory (platform-specific)
    ///
    /// This returns the platform default, NOT reading from SPOKE_HOME env var.
    /// The env var should be read in main.rs and passed to init/load.
    pub fn default_home() -> PathBuf {
        directories::ProjectDirs::from("com", "fastn", "fastn-spoke")
            .map(|p| p.data_dir().to_path_buf())
            .unwrap_or_else(|| {
                dirs::home_dir()
                    .unwrap_or_else(|| PathBuf::from("."))
                    .join(".fastn-spoke")
            })
    }

    /// Check if spoke is initialized at a specific path
    pub fn is_initialized(home: &std::path::Path) -> bool {
        home.join("spoke.key").exists()
    }

    /// Get the spoke's ID52
    pub fn id52(&self) -> &str {
        &self.config.spoke_id52
    }

    /// Get home directory
    pub fn home(&self) -> &PathBuf {
        &self.home
    }

    /// Get the hub's ID52 this spoke is configured to connect to
    pub fn hub_id52(&self) -> &str {
        &self.config.hub_id52
    }

    /// Get the spoke's alias
    pub fn alias(&self) -> &str {
        &self.config.alias
    }

    /// Get the secret key
    pub fn secret_key(&self) -> &SecretKey {
        &self.secret_key
    }

    /// Initialize a new spoke at the specified path
    ///
    /// Creates the home directory, generates a new secret key,
    /// and saves the configuration with the hub ID52, hub URL, and alias.
    pub async fn init(home: PathBuf, hub_id52: &str, hub_url: &str, alias: &str) -> Result<Self> {
        // Check if already initialized
        if Self::is_initialized(&home) {
            return Err(Error::AlreadyInitialized(home));
        }

        // Validate hub ID52 format
        fastn_net::from_id52(hub_id52)
            .map_err(|_| Error::InvalidId52(hub_id52.to_string()))?;

        // Create home directory
        tokio::fs::create_dir_all(&home).await?;

        // Generate new secret key
        let secret_key = SecretKey::generate();
        let public_key = secret_key.public();
        let spoke_id52 = public_key.id52();

        // Save secret key
        let key_path = home.join("spoke.key");
        let key_bytes = secret_key.to_bytes();
        tokio::fs::write(&key_path, key_bytes).await?;

        // Create and save config
        let config = SpokeConfig {
            spoke_id52,
            hub_id52: hub_id52.to_string(),
            hub_url: hub_url.to_string(),
            alias: alias.to_string(),
            created_at: Utc::now(),
        };
        let config_path = home.join("config.json");
        let config_json = serde_json::to_string_pretty(&config)?;
        tokio::fs::write(&config_path, config_json).await?;

        // Create empty hubs config
        let hubs = HubsConfig::default();
        let hubs_path = home.join("hubs.json");
        let hubs_json = serde_json::to_string_pretty(&hubs)?;
        tokio::fs::write(&hubs_path, hubs_json).await?;

        Ok(Self {
            home,
            secret_key,
            config,
            hubs,
        })
    }

    /// Load an existing spoke from the specified path
    pub async fn load(home: &std::path::Path) -> Result<Self> {
        // Check if initialized
        if !Self::is_initialized(home) {
            return Err(Error::NotInitialized);
        }

        let home = home.to_path_buf();

        // Load secret key
        let key_path = home.join("spoke.key");
        let key_bytes = tokio::fs::read(&key_path).await?;
        let key_array: [u8; 32] = key_bytes
            .try_into()
            .map_err(|_| Error::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "Invalid key file: expected 32 bytes",
            )))?;
        let secret_key = SecretKey::from_bytes(&key_array);

        // Load config
        let config_path = home.join("config.json");
        let config_json = tokio::fs::read_to_string(&config_path).await?;
        let config: SpokeConfig = serde_json::from_str(&config_json)?;

        // Load hubs config (or create default if missing)
        let hubs_path = home.join("hubs.json");
        let hubs = if hubs_path.exists() {
            let hubs_json = tokio::fs::read_to_string(&hubs_path).await?;
            serde_json::from_str(&hubs_json)?
        } else {
            HubsConfig::default()
        };

        Ok(Self {
            home,
            secret_key,
            config,
            hubs,
        })
    }

    /// Load or initialize spoke at the specified path
    pub async fn load_or_init(home: PathBuf, hub_id52: &str, hub_url: &str, alias: &str) -> Result<Self> {
        if Self::is_initialized(&home) {
            Self::load(&home).await
        } else {
            Self::init(home, hub_id52, hub_url, alias).await
        }
    }

    /// Get the hub's HTTP URL
    pub fn hub_url(&self) -> &str {
        &self.config.hub_url
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

    /// Connect to the configured hub
    pub fn connect(&self) -> HubConnection {
        let client = fastn_net::client::Client::new(
            self.secret_key.clone(),
            self.config.hub_id52.clone(),
            self.config.hub_url.clone(),
        );
        HubConnection {
            hub_id52: self.config.hub_id52.clone(),
            client,
        }
    }

    /// Connect to the hub (with HTTP, connection is made on each request)
    /// This is provided for API compatibility, but with HTTP transport
    /// the connection is per-request.
    pub fn connect_with_retry(&self, _retry_interval: std::time::Duration) -> HubConnection {
        // With HTTP transport, each request is independent
        // No persistent connection to retry
        self.connect()
    }
}

/// An active connection to a hub
pub struct HubConnection {
    /// The hub's ID52
    hub_id52: String,
    /// The underlying HTTP client
    client: fastn_net::client::Client,
}

impl HubConnection {
    /// Get the hub's ID52
    pub fn hub_id52(&self) -> &str {
        &self.hub_id52
    }

    /// Send a raw request to the hub
    ///
    /// - `target_hub`: "self" for local hub access, or hub alias for remote hub forwarding
    /// - `app`: Application identifier (e.g., "kosha")
    /// - `instance`: Instance name (e.g., kosha name)
    /// - `command`: Command to execute
    /// - `payload`: Request payload as JSON
    ///
    /// Returns the response payload as JSON
    pub async fn send_request(
        &self,
        target_hub: &str,
        app: &str,
        instance: &str,
        command: &str,
        payload: serde_json::Value,
    ) -> Result<serde_json::Value> {
        // Build the hub request
        // Note: The spoke's identity is derived from the cryptographic signature,
        // not from fields in the request. This provides security - the hub
        // verifies who the sender is, rather than trusting a claim.
        let request = fastn_hub::Request {
            target_hub: target_hub.to_string(),
            app: app.to_string(),
            instance: instance.to_string(),
            command: command.to_string(),
            payload,
        };

        // Call the hub using HTTP client
        let result: std::result::Result<fastn_hub::Response, fastn_hub::HubError> =
            self.client.call(&request).await?;

        match result {
            Ok(response) => Ok(response.payload),
            Err(hub_error) => Err(Error::Hub(format!("{:?}", hub_error))),
        }
    }

    /// Send a ping request to verify connectivity
    /// Returns Ok if hub accepts the connection
    pub async fn ping(&self) -> Result<()> {
        // For now, we'll just try to connect - the fastn_net::Spoke::new already establishes connection
        // A proper ping would send a special ping message
        // TODO: implement proper ping/heartbeat protocol
        Ok(())
    }

    // Kosha file operations (convenience wrappers around send_request)
    // All methods accept target_hub: "self" for local, or hub alias for remote

    /// Read a file from a kosha
    /// Returns: { content: base64 }
    pub async fn read_file(
        &self,
        target_hub: &str,
        kosha: &str,
        path: &str,
    ) -> Result<serde_json::Value> {
        self.send_request(
            target_hub,
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
        target_hub: &str,
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
        self.send_request(target_hub, "kosha", kosha, "write_file", payload)
            .await
    }

    /// List directory contents
    /// Returns: { entries: [...] }
    pub async fn list_dir(
        &self,
        target_hub: &str,
        kosha: &str,
        path: &str,
    ) -> Result<serde_json::Value> {
        self.send_request(
            target_hub,
            "kosha",
            kosha,
            "list_dir",
            serde_json::json!({ "path": path }),
        )
        .await
    }

    /// Get file versions
    /// Returns: { versions: [...] }
    pub async fn get_versions(
        &self,
        target_hub: &str,
        kosha: &str,
        path: &str,
    ) -> Result<serde_json::Value> {
        self.send_request(
            target_hub,
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
        target_hub: &str,
        kosha: &str,
        path: &str,
        timestamp: &str,
    ) -> Result<serde_json::Value> {
        self.send_request(
            target_hub,
            "kosha",
            kosha,
            "read_version",
            serde_json::json!({ "path": path, "timestamp": timestamp }),
        )
        .await
    }

    /// Rename a file
    /// Returns: {}
    pub async fn rename(
        &self,
        target_hub: &str,
        kosha: &str,
        from: &str,
        to: &str,
    ) -> Result<serde_json::Value> {
        self.send_request(
            target_hub,
            "kosha",
            kosha,
            "rename",
            serde_json::json!({ "from": from, "to": to }),
        )
        .await
    }

    /// Delete a file
    /// Returns: {}
    pub async fn delete(
        &self,
        target_hub: &str,
        kosha: &str,
        path: &str,
    ) -> Result<serde_json::Value> {
        self.send_request(
            target_hub,
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
    pub async fn kv_get(
        &self,
        target_hub: &str,
        kosha: &str,
        key: &str,
    ) -> Result<serde_json::Value> {
        self.send_request(
            target_hub,
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
        target_hub: &str,
        kosha: &str,
        key: &str,
        value: serde_json::Value,
    ) -> Result<serde_json::Value> {
        self.send_request(
            target_hub,
            "kosha",
            kosha,
            "kv_set",
            serde_json::json!({ "key": key, "value": value }),
        )
        .await
    }

    /// Delete a key from the KV store
    /// Returns: {}
    pub async fn kv_delete(
        &self,
        target_hub: &str,
        kosha: &str,
        key: &str,
    ) -> Result<serde_json::Value> {
        self.send_request(
            target_hub,
            "kosha",
            kosha,
            "kv_delete",
            serde_json::json!({ "key": key }),
        )
        .await
    }
}
