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
        // Build access context
        let ctx = AccessContext {
            spoke_id52: spoke_id52.to_string(),
            app: request.app.clone(),
            instance: request.instance.clone(),
            command: request.command.clone(),
        };

        // Check ACL via WASM modules
        match self.check_access(&ctx).await {
            AccessResult::Allowed => {}
            AccessResult::Denied(_reason) => {
                return Err(HubError::AccessDenied {
                    app: request.app.clone(),
                    instance: request.instance.clone(),
                });
            }
            AccessResult::NoModule => {
                // This shouldn't happen as check_access returns Denied if no module found
                return Err(HubError::AccessDenied {
                    app: request.app.clone(),
                    instance: request.instance.clone(),
                });
            }
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
// ACL - WASM-based Access Control
// ============================================================================
//
// Access control is delegated to WASM modules stored in a special "root" kosha.
//
// File structure in root kosha:
//   access.wasm              - Global ACL (fallback for all requests)
//   kosha/<name>/access.wasm - Per-kosha general ACL
//   kosha/<name>/read.wasm   - Per-kosha read-specific ACL (higher precedence)
//   kosha/<name>/write.wasm  - Per-kosha write-specific ACL (higher precedence)
//
// Each WASM module exports an `allowed` function:
//   fn allowed(spoke_id52: &str, app: &str, instance: &str, command: &str) -> bool
//
// Precedence (most specific wins):
//   1. kosha/<name>/<command>.wasm (e.g., read.wasm, write.wasm)
//   2. kosha/<name>/access.wasm
//   3. access.wasm (global)
//
// If no WASM module is found at any level, access is DENIED by default.

/// Access control context passed to WASM modules
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccessContext {
    /// The spoke requesting access
    pub spoke_id52: String,
    /// Application type (e.g., "kosha")
    pub app: String,
    /// Application instance (e.g., kosha name)
    pub instance: String,
    /// Command being executed (e.g., "read_file", "write_file")
    pub command: String,
}

/// Result of an access check
#[derive(Debug, Clone)]
pub enum AccessResult {
    /// Access granted
    Allowed,
    /// Access denied with reason
    Denied(String),
    /// No ACL module found - defer to next level
    NoModule,
}

impl Hub {
    /// Check if a spoke has access to perform a command on an (app, instance)
    ///
    /// This is the main entry point for ACL checks. It:
    /// 1. Looks up command-specific WASM (e.g., kosha/<name>/read.wasm)
    /// 2. Falls back to instance-specific WASM (kosha/<name>/access.wasm)
    /// 3. Falls back to global WASM (access.wasm)
    /// 4. Denies if no module found
    pub async fn check_access(&self, ctx: &AccessContext) -> AccessResult {
        // Get the root kosha for ACL modules
        let root = match self.koshas.get("root") {
            Some(k) => k,
            None => {
                // No root kosha means no ACL configured - deny by default
                return AccessResult::Denied("No root kosha configured".to_string());
            }
        };

        // Determine the command category for WASM lookup
        let command_category = Self::command_category(&ctx.command);

        // 1. Try command-specific WASM: kosha/<instance>/<category>.wasm
        if let Some(category) = command_category {
            let path = format!("kosha/{}/{}.wasm", ctx.instance, category);
            match self.run_access_wasm(root, &path, ctx).await {
                AccessResult::Allowed => return AccessResult::Allowed,
                AccessResult::Denied(reason) => return AccessResult::Denied(reason),
                AccessResult::NoModule => {} // Continue to next level
            }
        }

        // 2. Try instance-specific WASM: kosha/<instance>/access.wasm
        let path = format!("kosha/{}/access.wasm", ctx.instance);
        match self.run_access_wasm(root, &path, ctx).await {
            AccessResult::Allowed => return AccessResult::Allowed,
            AccessResult::Denied(reason) => return AccessResult::Denied(reason),
            AccessResult::NoModule => {} // Continue to next level
        }

        // 3. Try global WASM: access.wasm
        match self.run_access_wasm(root, "access.wasm", ctx).await {
            AccessResult::Allowed => return AccessResult::Allowed,
            AccessResult::Denied(reason) => return AccessResult::Denied(reason),
            AccessResult::NoModule => {} // No module at any level
        }

        // 4. No ACL module found anywhere - deny by default
        AccessResult::Denied("No ACL module found".to_string())
    }

    /// Map a command to its category (read, write, etc.)
    fn command_category(command: &str) -> Option<&'static str> {
        match command {
            // Read operations
            "read_file" | "list_dir" | "get_versions" | "read_version" | "kv_get" => Some("read"),
            // Write operations
            "write_file" | "rename" | "delete" | "kv_set" | "kv_delete" => Some("write"),
            // Unknown commands don't have a category
            _ => None,
        }
    }

    /// Run an access control WASM module
    async fn run_access_wasm(
        &self,
        root: &Kosha,
        path: &str,
        ctx: &AccessContext,
    ) -> AccessResult {
        // Try to read the WASM file
        let wasm_bytes = match root.read_file(path).await {
            Ok(bytes) => bytes,
            Err(_) => return AccessResult::NoModule,
        };

        // Run the WASM module
        match self.execute_access_wasm(&wasm_bytes, ctx).await {
            Ok(allowed) => {
                if allowed {
                    AccessResult::Allowed
                } else {
                    AccessResult::Denied(format!("Denied by {}", path))
                }
            }
            Err(e) => {
                // WASM execution error - treat as deny for safety
                AccessResult::Denied(format!("ACL WASM error in {}: {}", path, e))
            }
        }
    }

    /// Execute an access control WASM module and return the result
    async fn execute_access_wasm(
        &self,
        _wasm_bytes: &[u8],
        _ctx: &AccessContext,
    ) -> std::result::Result<bool, String> {
        // TODO: Implement WASM execution
        // The WASM module should export: fn allowed(ctx_json: &str) -> bool
        // We serialize AccessContext to JSON and pass it to the function
        todo!("execute_access_wasm - need WASM runtime integration")
    }
}

// Legacy ACL types - kept for migration, will be removed
/// Access control entry (legacy - being replaced by WASM-based ACL)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AclEntry {
    /// Spoke ID52 that has access
    pub spoke_id52: String,
    /// Optional name for the spoke
    pub name: Option<String>,
    /// When access was granted
    pub granted_at: DateTime<Utc>,
}

/// ACL configuration stored per (app, instance) (legacy)
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Acl {
    /// List of authorized spokes for this (app, instance)
    pub entries: Vec<AclEntry>,
}
