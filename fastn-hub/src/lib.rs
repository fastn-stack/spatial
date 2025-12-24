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
        // Extract path from payload for file operations (used in ACL checks)
        let path = Self::extract_path_from_payload(&request.command, &request.payload);

        // Build access context
        let ctx = AccessContext {
            spoke_id52: spoke_id52.to_string(),
            app: request.app.clone(),
            instance: request.instance.clone(),
            command: request.command.clone(),
            path,
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
// ACL - WASM-based Access Control (Cascading)
// ============================================================================
//
// Access control is cascading from top to bottom. Each level must ALLOW before
// proceeding to the next level. If any level DENIES, access is immediately denied.
// If no WASM module exists at a level, that level is skipped (implicit allow).
//
// Hierarchy (checked in order, top to bottom):
//
// 1. ROOT KOSHA - Global level (root/access.wasm)
//    └── access.wasm - Global ACL for all requests
//
// 2. ROOT KOSHA - App level (root/kosha/...)
//    ├── access.wasm - All kosha operations
//    ├── read.wasm   - All kosha read operations
//    └── write.wasm  - All kosha write operations
//
// 3. ROOT KOSHA - Instance level (root/kosha/<name>/...)
//    ├── access.wasm - This kosha's operations
//    ├── read.wasm   - This kosha's reads
//    └── write.wasm  - This kosha's writes
//
// 4. TARGET KOSHA - Folder levels (for file operations with paths)
//    For path "foo/bar/file.txt", check each level:
//    ├── /access.wasm, /read.wasm, /write.wasm  (root of target kosha)
//    ├── /foo/access.wasm, /foo/read.wasm, ...
//    └── /foo/bar/access.wasm, /foo/bar/read.wasm, ...
//
// At each level, precedence is: read/write.wasm (specific) > access.wasm (general)
//
// Flow:
//   - Check level → DENY = stop, ALLOW = continue to next level
//   - No module at level = implicit ALLOW (continue)
//   - All levels pass = ALLOW
//   - Must have at least one module somewhere (otherwise deny)

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
    /// Path within the kosha (for file operations), None for non-path operations
    pub path: Option<String>,
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
    /// This implements cascading ACL checks from top to bottom.
    /// Each level must ALLOW before proceeding. Any DENY stops immediately.
    pub async fn check_access(&self, ctx: &AccessContext) -> AccessResult {
        // Get the root kosha for ACL modules
        let root = match self.koshas.get("root") {
            Some(k) => k,
            None => {
                // No root kosha means no ACL configured - deny by default
                return AccessResult::Denied("No root kosha configured".to_string());
            }
        };

        let category = Self::command_category(&ctx.command);
        let mut found_any_module = false;

        // Level 1: Global ACL (root/access.wasm)
        match self.check_level(root, "", category, ctx).await {
            LevelResult::Denied(reason) => return AccessResult::Denied(reason),
            LevelResult::Allowed => found_any_module = true,
            LevelResult::NoModule => {}
        }

        // Level 2: App-level ACL (root/kosha/[access|read|write].wasm)
        let app_prefix = format!("{}/", ctx.app);
        match self.check_level(root, &app_prefix, category, ctx).await {
            LevelResult::Denied(reason) => return AccessResult::Denied(reason),
            LevelResult::Allowed => found_any_module = true,
            LevelResult::NoModule => {}
        }

        // Level 3: Instance-level ACL (root/kosha/<instance>/[access|read|write].wasm)
        let instance_prefix = format!("{}/{}/", ctx.app, ctx.instance);
        match self.check_level(root, &instance_prefix, category, ctx).await {
            LevelResult::Denied(reason) => return AccessResult::Denied(reason),
            LevelResult::Allowed => found_any_module = true,
            LevelResult::NoModule => {}
        }

        // Level 4: Target kosha folder-level ACL (for file operations with paths)
        if let Some(ref path) = ctx.path {
            // Get the target kosha
            if let Some(target_kosha) = self.koshas.get(&ctx.instance) {
                // Check each folder level from root to parent of target file
                let path_segments: Vec<&str> = path.split('/').collect();
                let mut current_prefix = String::new();

                // Check kosha root level
                match self.check_level(target_kosha, "", category, ctx).await {
                    LevelResult::Denied(reason) => return AccessResult::Denied(reason),
                    LevelResult::Allowed => found_any_module = true,
                    LevelResult::NoModule => {}
                }

                // Check each folder level (excluding the file itself)
                for segment in path_segments.iter().take(path_segments.len().saturating_sub(1)) {
                    if current_prefix.is_empty() {
                        current_prefix = format!("{}/", segment);
                    } else {
                        current_prefix = format!("{}{}/", current_prefix, segment);
                    }

                    match self.check_level(target_kosha, &current_prefix, category, ctx).await {
                        LevelResult::Denied(reason) => return AccessResult::Denied(reason),
                        LevelResult::Allowed => found_any_module = true,
                        LevelResult::NoModule => {}
                    }
                }
            }
        }

        // All levels passed, but we need at least one module to have been found
        if found_any_module {
            AccessResult::Allowed
        } else {
            AccessResult::Denied("No ACL module found at any level".to_string())
        }
    }

    /// Check a single level for ACL modules
    /// Returns Allowed if a module exists and allows, Denied if denies, NoModule if no module found
    async fn check_level(
        &self,
        kosha: &Kosha,
        prefix: &str,
        category: Option<&str>,
        ctx: &AccessContext,
    ) -> LevelResult {
        // First check category-specific module (read.wasm or write.wasm)
        if let Some(cat) = category {
            let path = format!("{}{}.wasm", prefix, cat);
            match self.run_access_wasm(kosha, &path, ctx).await {
                AccessResult::Allowed => return LevelResult::Allowed,
                AccessResult::Denied(reason) => return LevelResult::Denied(reason),
                AccessResult::NoModule => {} // Continue to check access.wasm
            }
        }

        // Then check general access.wasm
        let path = format!("{}access.wasm", prefix);
        match self.run_access_wasm(kosha, &path, ctx).await {
            AccessResult::Allowed => LevelResult::Allowed,
            AccessResult::Denied(reason) => LevelResult::Denied(reason),
            AccessResult::NoModule => LevelResult::NoModule,
        }
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

    /// Extract the path from a request payload for file operations
    /// Returns None for non-path operations (like kv_get, kv_set, etc.)
    fn extract_path_from_payload(command: &str, payload: &serde_json::Value) -> Option<String> {
        match command {
            // File operations that use "path" field
            "read_file" | "write_file" | "list_dir" | "get_versions" | "read_version" | "delete" => {
                payload.get("path").and_then(|v| v.as_str()).map(|s| s.to_string())
            }
            // Rename uses "from" as the source path for ACL check
            "rename" => {
                payload.get("from").and_then(|v| v.as_str()).map(|s| s.to_string())
            }
            // KV operations and others don't have paths
            _ => None,
        }
    }

    /// Run an access control WASM module
    async fn run_access_wasm(
        &self,
        kosha: &Kosha,
        path: &str,
        ctx: &AccessContext,
    ) -> AccessResult {
        // Try to read the WASM file
        let wasm_bytes = match kosha.read_file(path).await {
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

/// Result of checking a single ACL level
enum LevelResult {
    Allowed,
    Denied(String),
    NoModule,
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
