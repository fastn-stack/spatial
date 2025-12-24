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

    /// Initialize a new hub
    ///
    /// Creates FASTN_HOME directory, generates a new secret key,
    /// and saves the configuration.
    pub async fn init() -> Result<Self> {
        let home = Self::home_dir();

        // Check if already initialized
        if Self::is_initialized() {
            return Err(Error::Io(std::io::Error::new(
                std::io::ErrorKind::AlreadyExists,
                format!("Hub already initialized at {:?}", home),
            )));
        }

        // Create home directory
        tokio::fs::create_dir_all(&home).await?;

        // Generate new secret key
        let secret_key = SecretKey::generate(&mut rand::thread_rng());
        let public_key = secret_key.public();
        let hub_id52 = fastn_net::to_id52(&public_key);

        // Save secret key
        let key_path = home.join("hub.key");
        let key_bytes = secret_key.to_bytes();
        tokio::fs::write(&key_path, key_bytes).await?;

        // Create and save config
        let config = HubConfig {
            hub_id52,
            created_at: Utc::now(),
        };
        let config_path = home.join("config.json");
        let config_json = serde_json::to_string_pretty(&config)?;
        tokio::fs::write(&config_path, config_json).await?;

        // Create koshas directory
        tokio::fs::create_dir_all(home.join("koshas")).await?;

        Ok(Self {
            home,
            secret_key,
            config,
            koshas: HashMap::new(),
            acls: HashMap::new(),
        })
    }

    /// Load an existing hub
    ///
    /// Loads the secret key and configuration from FASTN_HOME.
    pub async fn load() -> Result<Self> {
        let home = Self::home_dir();

        // Check if initialized
        if !Self::is_initialized() {
            return Err(Error::NotInitialized);
        }

        // Load secret key
        let key_path = home.join("hub.key");
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
        let config: HubConfig = serde_json::from_str(&config_json)?;

        Ok(Self {
            home,
            secret_key,
            config,
            koshas: HashMap::new(),
            acls: HashMap::new(),
        })
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
    ///
    /// The `requester_hub_id` is the hub ID of the spoke making the request.
    /// Each user has their own hub, so requester_hub_id == self.id52() means
    /// the request is from the hub owner.
    pub async fn handle_request(
        &self,
        spoke_id52: &str,
        requester_hub_id: &str,
        request: Request,
    ) -> std::result::Result<Response, HubError> {
        // Extract path from payload for file operations (used in ACL checks)
        let path = Self::extract_path_from_payload(&request.command, &request.payload);

        // Build access context with hub IDs
        let ctx = AccessContext {
            requester_hub_id: requester_hub_id.to_string(),
            current_hub_id: self.config.hub_id52.clone(),
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

    /// Get the secret key
    pub fn secret_key(&self) -> &SecretKey {
        &self.secret_key
    }

    /// Run the hub server
    ///
    /// Starts the iroh endpoint and accepts connections in a loop.
    /// For each request, routes to the appropriate handler.
    pub async fn serve(self) -> Result<()> {
        // Create the network hub
        let net_hub = fastn_net::Hub::new(self.secret_key.clone()).await?;

        println!("Hub listening on ID52: {}", net_hub.id52());
        println!("FASTN_HOME: {:?}", self.home);

        // Accept connections in a loop
        loop {
            match net_hub.accept::<Request>().await {
                Ok((peer_key, request, responder)) => {
                    let peer_id52 = fastn_net::to_id52(&peer_key);

                    // For now, we assume the requester's hub ID is the same as their spoke ID
                    // In a full implementation, this would be extracted from the request
                    // or looked up from a registry
                    let requester_hub_id = peer_id52.clone();

                    match self.handle_request(&peer_id52, &requester_hub_id, request).await {
                        Ok(response) => {
                            if let Err(e) = responder.respond::<Response, HubError>(Ok(response)).await {
                                eprintln!("Failed to send response: {}", e);
                            }
                        }
                        Err(hub_error) => {
                            if let Err(e) = responder.respond::<Response, HubError>(Err(hub_error)).await {
                                eprintln!("Failed to send error response: {}", e);
                            }
                        }
                    }
                }
                Err(e) => {
                    eprintln!("Failed to accept connection: {}", e);
                }
            }
        }
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
// SPECIAL FILES (prefixed with `_`):
//   - _access.wasm - General access control
//   - _read.wasm   - Read access control
//   - _write.wasm  - Write access control
//   - _admin.wasm  - Admin access (for modifying ACL files)
//
// READ/WRITE vs GET/POST:
//   - read_file/write_file: Raw byte operations for file content
//   - get/post: HTTP-like semantics with content-type and WASM execution
//
// GET/POST WASM EXECUTION:
//   Any .wasm file NOT prefixed with `_` can handle get/post requests:
//   - foo.wasm handles requests to /foo.wasm
//   - foo.json.wasm handles requests to /foo.json (dynamic handler)
//   - /foo/ tries foo.wasm first, then foo/index.wasm
//
// CONSTRAINTS:
//   - foo.json and foo.json.wasm cannot both exist (write/rename fails)
//   - foo.wasm and foo/index.wasm cannot both exist
//
// Hierarchy (checked in order, top to bottom):
//
// 1. ROOT KOSHA - Global level (root/_access.wasm)
//    └── _access.wasm - Global ACL for all requests
//
// 2. ROOT KOSHA - App level (root/kosha/...)
//    ├── _access.wasm - All kosha operations
//    ├── _read.wasm   - All kosha read operations
//    └── _write.wasm  - All kosha write operations
//
// 3. ROOT KOSHA - Instance level (root/kosha/<name>/...)
//    ├── _access.wasm - This kosha's operations
//    ├── _read.wasm   - This kosha's reads
//    └── _write.wasm  - This kosha's writes
//
// 4. TARGET KOSHA - Folder levels (for file operations with paths)
//    For path "foo/bar/file.txt", check each level:
//    ├── /_access.wasm, /_read.wasm, /_write.wasm  (root of target kosha)
//    ├── /foo/_access.wasm, /foo/_read.wasm, ...
//    └── /foo/bar/_access.wasm, /foo/bar/_read.wasm, ...
//
// At each level, precedence is: _read/_write.wasm (specific) > _access.wasm (general)
//
// Flow:
//   - Check level → DENY = stop, ALLOW = continue to next level
//   - No module at level = implicit ALLOW (continue)
//   - All levels pass = ALLOW
//   - Must have at least one module somewhere (otherwise deny)
//
// ADMIN ACCESS:
//   Writes to special files (_access.wasm, _read.wasm, _write.wasm, _admin.wasm)
//   require admin permission. The system checks _admin.wasm from the target
//   directory upward to root. If no _admin.wasm is found, only hub owner can modify.
//
//   Example: To write foo/bar/_access.wasm:
//   1. Check foo/bar/_admin.wasm
//   2. If not found, check foo/_admin.wasm
//   3. If not found, check _admin.wasm (root)
//   4. If none found, deny (hub owner only)
//

/// Access control context passed to WASM modules
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccessContext {
    /// Hub ID of the requesting spoke (each user has their own hub)
    pub requester_hub_id: String,
    /// This hub's ID (if requester_hub_id == current_hub_id, it's the owner)
    pub current_hub_id: String,
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

impl AccessContext {
    /// Check if the requester is the hub owner (same user)
    pub fn is_owner(&self) -> bool {
        self.requester_hub_id == self.current_hub_id
    }
}

/// Request context passed to get/post WASM handlers
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequestContext {
    /// Hub ID of the requesting spoke
    pub requester_hub_id: String,
    /// This hub's ID
    pub current_hub_id: String,
    /// The spoke requesting access
    pub spoke_id52: String,
    /// HTTP method: "GET" or "POST"
    pub method: String,
    /// Request path
    pub path: String,
    /// Query string (if any)
    pub query: Option<String>,
    /// POST payload (JSON)
    pub payload: Option<serde_json::Value>,
}

impl RequestContext {
    /// Check if the requester is the hub owner (same user)
    pub fn is_owner(&self) -> bool {
        self.requester_hub_id == self.current_hub_id
    }
}

/// Database access context for _db.wasm ACL
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DbAccessContext {
    /// Hub ID of the requesting spoke
    pub requester_hub_id: String,
    /// This hub's ID
    pub current_hub_id: String,
    /// The spoke requesting access
    pub spoke_id52: String,
    /// Database name (e.g., "users.db")
    pub database: String,
    /// Operation: "query", "execute", "begin", "commit", "rollback"
    pub operation: String,
}

impl DbAccessContext {
    /// Check if the requester is the hub owner (same user)
    pub fn is_owner(&self) -> bool {
        self.requester_hub_id == self.current_hub_id
    }
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
    ///
    /// For write operations on ACL files (access.wasm, read.wasm, write.wasm, admin.wasm),
    /// an additional admin check is performed via admin.wasm.
    pub async fn check_access(&self, ctx: &AccessContext) -> AccessResult {
        // Get the root kosha for ACL modules
        let root = match self.koshas.get("root") {
            Some(k) => k,
            None => {
                // No root kosha means no ACL configured - deny by default
                return AccessResult::Denied("No root kosha configured".to_string());
            }
        };

        // Check if this is a write to a special file - requires admin access
        let is_special_write = matches!(ctx.command.as_str(), "write_file" | "delete" | "rename")
            && ctx.path.as_ref().map(|p| Self::is_special_file(p)).unwrap_or(false);

        if is_special_write {
            if let Some(ref path) = ctx.path {
                if let Some(target_kosha) = self.koshas.get(&ctx.instance) {
                    match self.check_admin_access(target_kosha, path, ctx).await {
                        AccessResult::Allowed => {}
                        AccessResult::Denied(reason) => return AccessResult::Denied(reason),
                        AccessResult::NoModule => {
                            return AccessResult::Denied(
                                "Admin access required to modify ACL files".to_string()
                            );
                        }
                    }
                }
            }
        }

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
        // First check category-specific module (_read.wasm or _write.wasm)
        if let Some(cat) = category {
            let path = format!("{}_{}.wasm", prefix, cat);
            match self.run_access_wasm(kosha, &path, ctx).await {
                AccessResult::Allowed => return LevelResult::Allowed,
                AccessResult::Denied(reason) => return LevelResult::Denied(reason),
                AccessResult::NoModule => {} // Continue to check _access.wasm
            }
        }

        // Then check general _access.wasm
        let path = format!("{}_access.wasm", prefix);
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

    /// Check if a path refers to a special WASM file (prefixed with `_`)
    /// Note: index.wasm is NOT a special file - it's the directory handler
    fn is_special_file(path: &str) -> bool {
        let filename = path.rsplit('/').next().unwrap_or(path);
        matches!(filename, "_access.wasm" | "_read.wasm" | "_write.wasm" | "_admin.wasm")
    }

    /// Check admin access for modifying ACL files
    ///
    /// When writing to ACL files (_access.wasm, _read.wasm, _write.wasm, _admin.wasm),
    /// we need to check _admin.wasm at the same level or parent levels.
    ///
    /// For example, to write `foo/bar/_access.wasm`:
    /// 1. Check `foo/bar/_admin.wasm`
    /// 2. If not found, check `foo/_admin.wasm`
    /// 3. If not found, check `_admin.wasm` (root)
    /// 4. If no _admin.wasm found anywhere, deny (only hub owner can modify)
    pub async fn check_admin_access(&self, kosha: &Kosha, path: &str, ctx: &AccessContext) -> AccessResult {
        // Get the directory containing the ACL file
        let dir = if let Some(idx) = path.rfind('/') {
            &path[..idx]
        } else {
            ""
        };

        // Check _admin.wasm from the target directory up to root
        let mut current_dir = dir.to_string();
        loop {
            let admin_path = if current_dir.is_empty() {
                "_admin.wasm".to_string()
            } else {
                format!("{}/_admin.wasm", current_dir)
            };

            match self.run_access_wasm(kosha, &admin_path, ctx).await {
                AccessResult::Allowed => return AccessResult::Allowed,
                AccessResult::Denied(reason) => return AccessResult::Denied(reason),
                AccessResult::NoModule => {
                    // No _admin.wasm at this level, try parent
                    if current_dir.is_empty() {
                        break;
                    }
                    current_dir = if let Some(idx) = current_dir.rfind('/') {
                        current_dir[..idx].to_string()
                    } else {
                        String::new()
                    };
                }
            }
        }

        // No _admin.wasm found anywhere - deny by default
        // Only hub owner (checked separately) can modify ACL files
        AccessResult::Denied("No _admin.wasm found - only hub owner can modify ACL files".to_string())
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
