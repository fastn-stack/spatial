//! Hub server for fastn P2P network
//!
//! A Hub is an application router that:
//! - Manages koshas (and other future apps)
//! - Handles ACL at the (app, instance) level
//! - Routes requests to the appropriate application handler
//!
//! The hub depends on apps directly and uses hardcoded app names.
//! JSON in, JSON out interface.
//!
//! See README.md for full documentation.

use chrono::{DateTime, Utc};
use fastn_kosha::Kosha;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use thiserror::Error;

pub use fastn_net::{PublicKey, SecretKey};

/// Error types for hub operations
#[derive(Error, Debug)]
pub enum Error {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Network error: {0}")]
    Net(#[from] fastn_net::Error),

    #[error("Hub not initialized. Run 'fastn-hub init' first.")]
    NotInitialized,

    #[error("Spoke not authorized: {0}")]
    Unauthorized(String),

    #[error("Access denied to {0}/{1}")]
    AccessDenied(String, String),

    #[error("Application not found: {0}")]
    AppNotFound(String),

    #[error("Instance not found: {0}/{1}")]
    InstanceNotFound(String, String),

    #[error("Invalid ID52: {0}")]
    InvalidId52(String),

    #[error("Kosha error: {0}")]
    Kosha(#[from] fastn_kosha::Error),
}

pub type Result<T> = std::result::Result<T, Error>;

/// Hub configuration stored in config.json
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HubConfig {
    pub hub_id52: String,
    pub created_at: DateTime<Utc>,
}

/// The Hub server - application router
pub struct Hub {
    /// Path to FASTN_HOME
    home: PathBuf,
    /// Hub's secret key
    secret_key: SecretKey,
    /// Configuration
    config: HubConfig,
    /// Registered koshas by alias
    koshas: HashMap<String, Kosha>,
    /// ACLs by (app, instance) -> Acl
    acls: HashMap<(String, String), Acl>,
}

impl Hub {
    /// Get the FASTN_HOME directory
    pub fn home_dir() -> PathBuf {
        if let Ok(home) = std::env::var("FASTN_HOME") {
            PathBuf::from(home)
        } else {
            directories::ProjectDirs::from("com", "fastn", "fastn")
                .map(|p| p.data_dir().to_path_buf())
                .unwrap_or_else(|| {
                    dirs::home_dir()
                        .unwrap_or_else(|| PathBuf::from("."))
                        .join(".fastn")
                })
        }
    }

    /// Check if hub is initialized
    pub fn is_initialized() -> bool {
        Self::home_dir().join("hub.key").exists()
    }

    /// Get the hub's ID52
    pub fn id52(&self) -> &str {
        &self.config.hub_id52
    }

    /// Get home directory
    pub fn home(&self) -> &PathBuf {
        &self.home
    }

    // Stub implementations - to be filled in

    /// Initialize a new hub
    pub async fn init() -> Result<Self> {
        todo!("Hub::init")
    }

    /// Load an existing hub
    pub async fn load() -> Result<Self> {
        todo!("Hub::load")
    }

    /// Load or initialize hub
    pub async fn load_or_init() -> Result<Self> {
        if Self::is_initialized() {
            Self::load().await
        } else {
            Self::init().await
        }
    }

    /// Register a kosha
    pub fn register_kosha(&mut self, kosha: Kosha) {
        self.koshas.insert(kosha.alias().to_string(), kosha);
    }

    /// Get a registered kosha by alias
    pub fn get_kosha(&self, alias: &str) -> Option<&Kosha> {
        self.koshas.get(alias)
    }

    /// List registered kosha aliases
    pub fn list_koshas(&self) -> Vec<&str> {
        self.koshas.keys().map(|s| s.as_str()).collect()
    }

    /// Grant access to (app, instance) for a spoke
    pub fn grant_access(&mut self, app: &str, instance: &str, spoke_id52: &str, name: Option<&str>) {
        let key = (app.to_string(), instance.to_string());
        let acl = self.acls.entry(key).or_default();

        // Don't add duplicate
        if acl.entries.iter().any(|e| e.spoke_id52 == spoke_id52) {
            return;
        }

        acl.entries.push(AclEntry {
            spoke_id52: spoke_id52.to_string(),
            name: name.map(|s| s.to_string()),
            granted_at: Utc::now(),
        });
    }

    /// Revoke access to (app, instance) for a spoke
    pub fn revoke_access(&mut self, app: &str, instance: &str, spoke_id52: &str) {
        let key = (app.to_string(), instance.to_string());
        if let Some(acl) = self.acls.get_mut(&key) {
            acl.entries.retain(|e| e.spoke_id52 != spoke_id52);
        }
    }

    /// Check if spoke has access to (app, instance)
    pub fn has_access(&self, app: &str, instance: &str, spoke_id52: &str) -> bool {
        let key = (app.to_string(), instance.to_string());
        self.acls
            .get(&key)
            .map(|acl| acl.entries.iter().any(|e| e.spoke_id52 == spoke_id52))
            .unwrap_or(false)
    }

    /// Handle a request from a spoke
    ///
    /// Routes based on hardcoded app names:
    /// - "kosha": routes to registered koshas
    pub async fn handle_request(&self, spoke_id52: &str, request: Request) -> std::result::Result<Response, HubError> {
        // Check ACL first (applies to all app types)
        if !self.has_access(&request.app, &request.instance, spoke_id52) {
            return Err(HubError::AccessDenied {
                app: request.app.clone(),
                instance: request.instance.clone(),
            });
        }

        // Route based on hardcoded app name
        match request.app.as_str() {
            "kosha" => {
                // Find the kosha by instance name (alias)
                let kosha = self.koshas.get(&request.instance).ok_or_else(|| {
                    HubError::InstanceNotFound {
                        app: request.app.clone(),
                        instance: request.instance.clone(),
                    }
                })?;

                // Forward to kosha's handle_command
                let payload = kosha
                    .handle_command(&request.command, request.payload)
                    .await
                    .map_err(|e| HubError::AppError { message: e })?;

                Ok(Response { payload })
            }
            _ => Err(HubError::AppNotFound {
                app: request.app.clone(),
            }),
        }
    }

    /// Run the hub server
    pub async fn serve(&self) -> Result<()> {
        todo!("Hub::serve")
    }
}

// ============================================================================
// Hub Protocol - Generic Application Router
// ============================================================================

/// Request envelope from spokes
/// Hub routes based on (app, instance) and does ACL check before forwarding
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Request {
    /// Application type (e.g., "kosha", "chat", "sync")
    pub app: String,
    /// Application instance (e.g., "my-kosha", "work-chat")
    pub instance: String,
    /// Application-specific command name
    pub command: String,
    /// Application-specific payload (JSON)
    pub payload: serde_json::Value,
}

/// Response envelope to spokes
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Response {
    /// Application-specific response payload (JSON)
    pub payload: serde_json::Value,
}

/// Hub-level errors (before reaching application)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum HubError {
    /// Spoke not authorized for this hub
    Unauthorized,
    /// Spoke not authorized for this (app, instance)
    AccessDenied { app: String, instance: String },
    /// Application type not registered
    AppNotFound { app: String },
    /// Application instance not found
    InstanceNotFound { app: String, instance: String },
    /// Application returned an error
    AppError { message: String },
}

// ============================================================================
// ACL - Access Control
// ============================================================================

/// Access control entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AclEntry {
    /// Spoke ID52 that has access
    pub spoke_id52: String,
    /// Optional name for the spoke
    pub name: Option<String>,
    /// When access was granted
    pub granted_at: DateTime<Utc>,
}

/// ACL configuration stored per (app, instance)
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Acl {
    /// List of authorized spokes for this (app, instance)
    pub entries: Vec<AclEntry>,
}
