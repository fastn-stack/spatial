//! Spoke client for fastn P2P network
//!
//! A Spoke connects to hubs and accesses koshas.
//! See README.md for full documentation.
//!
//! # Platform Support
//!
//! - Native (desktop): Uses file system storage via tokio::fs
//! - WASM (web): Uses OPFS (Origin Private File System) storage

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[cfg(not(target_arch = "wasm32"))]
use std::path::PathBuf;

pub use fastn_net::{PublicKey, SecretKey};

/// Error types for spoke operations
#[derive(Error, Debug)]
pub enum Error {
    #[cfg(not(target_arch = "wasm32"))]
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

    #[cfg(not(target_arch = "wasm32"))]
    #[error("Spoke already initialized at {0:?}")]
    AlreadyInitialized(PathBuf),

    #[cfg(target_arch = "wasm32")]
    #[error("Spoke already initialized")]
    AlreadyInitialized,

    #[cfg(target_arch = "wasm32")]
    #[error("Storage error: {0}")]
    Storage(String),
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

// ============================================================================
// Native implementation (desktop)
// ============================================================================
#[cfg(not(target_arch = "wasm32"))]
mod native {
    use super::*;

    /// The Spoke client (native)
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
        pub async fn init(home: PathBuf, hub_id52: &str, hub_url: &str, alias: &str) -> Result<Self> {
            if Self::is_initialized(&home) {
                return Err(Error::AlreadyInitialized(home));
            }

            fastn_net::from_id52(hub_id52)
                .map_err(|_| Error::InvalidId52(hub_id52.to_string()))?;

            tokio::fs::create_dir_all(&home).await?;

            let secret_key = SecretKey::generate();
            let public_key = secret_key.public();
            let spoke_id52 = public_key.id52();

            let key_path = home.join("spoke.key");
            let key_bytes = secret_key.to_bytes();
            tokio::fs::write(&key_path, key_bytes).await?;

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
            if !Self::is_initialized(home) {
                return Err(Error::NotInitialized);
            }

            let home = home.to_path_buf();

            let key_path = home.join("spoke.key");
            let key_bytes = tokio::fs::read(&key_path).await?;
            let key_array: [u8; 32] = key_bytes
                .try_into()
                .map_err(|_| Error::Io(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "Invalid key file: expected 32 bytes",
                )))?;
            let secret_key = SecretKey::from_bytes(&key_array);

            let config_path = home.join("config.json");
            let config_json = tokio::fs::read_to_string(&config_path).await?;
            let config: SpokeConfig = serde_json::from_str(&config_json)?;

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
        pub fn connect_with_retry(&self, _retry_interval: std::time::Duration) -> HubConnection {
            self.connect()
        }
    }

    /// An active connection to a hub (native)
    pub struct HubConnection {
        hub_id52: String,
        client: fastn_net::client::Client,
    }

    impl HubConnection {
        pub fn hub_id52(&self) -> &str {
            &self.hub_id52
        }

        pub async fn send_request(
            &self,
            target_hub: &str,
            app: &str,
            instance: &str,
            command: &str,
            payload: serde_json::Value,
        ) -> Result<serde_json::Value> {
            let request = fastn_net::HubRequest {
                target_hub: target_hub.to_string(),
                app: app.to_string(),
                instance: instance.to_string(),
                command: command.to_string(),
                payload,
            };

            let result: std::result::Result<fastn_net::HubResponse, fastn_net::HubError> =
                self.client.call(&request).await?;

            match result {
                Ok(response) => Ok(response.payload),
                Err(hub_error) => Err(Error::Hub(format!("{:?}", hub_error))),
            }
        }

        pub async fn ping(&self) -> Result<()> {
            Ok(())
        }

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
}

#[cfg(not(target_arch = "wasm32"))]
pub use native::{HubConnection, Spoke};

// ============================================================================
// WASM implementation (web browser)
// ============================================================================
#[cfg(target_arch = "wasm32")]
mod wasm {
    use super::*;
    use wasm_bindgen::prelude::*;
    use wasm_bindgen_futures::JsFuture;
    use web_sys::{
        FileSystemDirectoryHandle, FileSystemFileHandle, FileSystemGetFileOptions,
        FileSystemGetDirectoryOptions, FileSystemWritableFileStream,
    };

    /// The Spoke client (WASM/web)
    pub struct Spoke {
        /// Spoke's secret key
        secret_key: SecretKey,
        /// Configuration
        config: SpokeConfig,
        /// Known hubs
        #[allow(dead_code)]
        hubs: HubsConfig,
        /// OPFS root directory handle
        opfs_root: FileSystemDirectoryHandle,
    }

    impl Spoke {
        /// Get the OPFS root directory
        async fn get_opfs_root() -> Result<FileSystemDirectoryHandle> {
            let window = web_sys::window()
                .ok_or_else(|| Error::Storage("No window object".to_string()))?;
            let navigator = window.navigator();
            let storage = navigator.storage();

            let promise = storage.get_directory();
            let root = JsFuture::from(promise)
                .await
                .map_err(|e| Error::Storage(format!("Failed to get OPFS root: {:?}", e)))?;

            Ok(root.unchecked_into())
        }

        /// Get or create a directory in OPFS
        async fn get_directory(
            parent: &FileSystemDirectoryHandle,
            name: &str,
            create: bool,
        ) -> Result<FileSystemDirectoryHandle> {
            let options = FileSystemGetDirectoryOptions::new();
            options.set_create(create);

            let promise = parent.get_directory_handle_with_options(name, &options);
            JsFuture::from(promise)
                .await
                .map(|v| v.unchecked_into())
                .map_err(|e| Error::Storage(format!("Failed to get directory {}: {:?}", name, e)))
        }

        /// Get or create a file in OPFS
        async fn get_file(
            parent: &FileSystemDirectoryHandle,
            name: &str,
            create: bool,
        ) -> Result<FileSystemFileHandle> {
            let options = FileSystemGetFileOptions::new();
            options.set_create(create);

            let promise = parent.get_file_handle_with_options(name, &options);
            JsFuture::from(promise)
                .await
                .map(|v| v.unchecked_into())
                .map_err(|e| Error::Storage(format!("Failed to get file {}: {:?}", name, e)))
        }

        /// Read file contents from OPFS
        async fn read_file_bytes(file_handle: &FileSystemFileHandle) -> Result<Vec<u8>> {
            let promise = file_handle.get_file();
            let file: web_sys::File = JsFuture::from(promise)
                .await
                .map_err(|e| Error::Storage(format!("Failed to get file: {:?}", e)))?
                .unchecked_into();

            let promise = file.array_buffer();
            let buffer = JsFuture::from(promise)
                .await
                .map_err(|e| Error::Storage(format!("Failed to read file: {:?}", e)))?;

            let array = js_sys::Uint8Array::new(&buffer);
            Ok(array.to_vec())
        }

        /// Write file contents to OPFS
        async fn write_file_bytes(file_handle: &FileSystemFileHandle, data: &[u8]) -> Result<()> {
            let options = web_sys::FileSystemCreateWritableOptions::new();
            let promise = file_handle.create_writable_with_options(&options);
            let writable: FileSystemWritableFileStream = JsFuture::from(promise)
                .await
                .map_err(|e| Error::Storage(format!("Failed to create writable: {:?}", e)))?
                .unchecked_into();

            let array = js_sys::Uint8Array::from(data);
            let promise = writable.write_with_buffer_source(&array)
                .map_err(|e| Error::Storage(format!("Failed to write: {:?}", e)))?;
            JsFuture::from(promise)
                .await
                .map_err(|e| Error::Storage(format!("Failed to write: {:?}", e)))?;

            let promise = writable.close();
            JsFuture::from(promise)
                .await
                .map_err(|e| Error::Storage(format!("Failed to close: {:?}", e)))?;

            Ok(())
        }

        /// Check if spoke is initialized in OPFS
        pub async fn is_initialized() -> bool {
            let Ok(root) = Self::get_opfs_root().await else {
                return false;
            };

            // Try to get the spoke.key file
            let options = FileSystemGetFileOptions::new();
            options.set_create(false);

            let promise = root.get_file_handle_with_options("spoke.key", &options);
            JsFuture::from(promise).await.is_ok()
        }

        /// Get the spoke's ID52
        pub fn id52(&self) -> &str {
            &self.config.spoke_id52
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

        /// Get the hub's HTTP URL
        pub fn hub_url(&self) -> &str {
            &self.config.hub_url
        }

        /// Initialize a new spoke in OPFS
        pub async fn init(hub_id52: &str, hub_url: &str, alias: &str) -> Result<Self> {
            if Self::is_initialized().await {
                return Err(Error::AlreadyInitialized);
            }

            fastn_net::from_id52(hub_id52)
                .map_err(|_| Error::InvalidId52(hub_id52.to_string()))?;

            let opfs_root = Self::get_opfs_root().await?;

            // Generate new secret key
            let secret_key = SecretKey::generate();
            let public_key = secret_key.public();
            let spoke_id52 = public_key.id52();

            // Save secret key
            let key_file = Self::get_file(&opfs_root, "spoke.key", true).await?;
            Self::write_file_bytes(&key_file, &secret_key.to_bytes()).await?;

            // Create and save config
            let config = SpokeConfig {
                spoke_id52,
                hub_id52: hub_id52.to_string(),
                hub_url: hub_url.to_string(),
                alias: alias.to_string(),
                created_at: Utc::now(),
            };
            let config_file = Self::get_file(&opfs_root, "config.json", true).await?;
            let config_json = serde_json::to_string_pretty(&config)?;
            Self::write_file_bytes(&config_file, config_json.as_bytes()).await?;

            // Create empty hubs config
            let hubs = HubsConfig::default();
            let hubs_file = Self::get_file(&opfs_root, "hubs.json", true).await?;
            let hubs_json = serde_json::to_string_pretty(&hubs)?;
            Self::write_file_bytes(&hubs_file, hubs_json.as_bytes()).await?;

            Ok(Self {
                secret_key,
                config,
                hubs,
                opfs_root,
            })
        }

        /// Load an existing spoke from OPFS
        pub async fn load() -> Result<Self> {
            if !Self::is_initialized().await {
                return Err(Error::NotInitialized);
            }

            let opfs_root = Self::get_opfs_root().await?;

            // Load secret key
            let key_file = Self::get_file(&opfs_root, "spoke.key", false).await?;
            let key_bytes = Self::read_file_bytes(&key_file).await?;
            let key_array: [u8; 32] = key_bytes
                .try_into()
                .map_err(|_| Error::Storage("Invalid key file: expected 32 bytes".to_string()))?;
            let secret_key = SecretKey::from_bytes(&key_array);

            // Load config
            let config_file = Self::get_file(&opfs_root, "config.json", false).await?;
            let config_bytes = Self::read_file_bytes(&config_file).await?;
            let config_json = String::from_utf8(config_bytes)
                .map_err(|e| Error::Storage(format!("Invalid config.json: {}", e)))?;
            let config: SpokeConfig = serde_json::from_str(&config_json)?;

            // Load hubs config (or create default if missing)
            let hubs = match Self::get_file(&opfs_root, "hubs.json", false).await {
                Ok(hubs_file) => {
                    let hubs_bytes = Self::read_file_bytes(&hubs_file).await?;
                    let hubs_json = String::from_utf8(hubs_bytes)
                        .map_err(|e| Error::Storage(format!("Invalid hubs.json: {}", e)))?;
                    serde_json::from_str(&hubs_json)?
                }
                Err(_) => HubsConfig::default(),
            };

            Ok(Self {
                secret_key,
                config,
                hubs,
                opfs_root,
            })
        }

        /// Load or initialize spoke
        pub async fn load_or_init(hub_id52: &str, hub_url: &str, alias: &str) -> Result<Self> {
            if Self::is_initialized().await {
                Self::load().await
            } else {
                Self::init(hub_id52, hub_url, alias).await
            }
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
            let client = fastn_net::web_client::Client::new(
                self.secret_key.clone(),
                self.config.hub_id52.clone(),
                self.config.hub_url.clone(),
            );
            HubConnection {
                hub_id52: self.config.hub_id52.clone(),
                client,
            }
        }
    }

    /// An active connection to a hub (WASM)
    pub struct HubConnection {
        hub_id52: String,
        client: fastn_net::web_client::Client,
    }

    impl HubConnection {
        pub fn hub_id52(&self) -> &str {
            &self.hub_id52
        }

        pub async fn send_request(
            &self,
            target_hub: &str,
            app: &str,
            instance: &str,
            command: &str,
            payload: serde_json::Value,
        ) -> Result<serde_json::Value> {
            let request = fastn_net::HubRequest {
                target_hub: target_hub.to_string(),
                app: app.to_string(),
                instance: instance.to_string(),
                command: command.to_string(),
                payload,
            };

            let result: std::result::Result<fastn_net::HubResponse, fastn_net::HubError> =
                self.client.call(&request).await?;

            match result {
                Ok(response) => Ok(response.payload),
                Err(hub_error) => Err(Error::Hub(format!("{:?}", hub_error))),
            }
        }

        pub async fn ping(&self) -> Result<()> {
            Ok(())
        }

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
}

#[cfg(target_arch = "wasm32")]
pub use wasm::{HubConnection, Spoke};
