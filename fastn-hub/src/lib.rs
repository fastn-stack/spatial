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

/// An authorized spoke entry (parsed from spokes.txt)
#[derive(Debug, Clone)]
pub struct AuthorizedSpoke {
    pub id52: String,
    pub alias: String,
}

/// Spokes configuration (parsed from spokes.txt)
/// Format: one line per spoke: `<id52>: <alias>`
#[derive(Debug, Clone, Default)]
pub struct SpokesConfig {
    pub spokes: Vec<AuthorizedSpoke>,
}

impl SpokesConfig {
    /// Parse spokes.txt content into SpokesConfig
    pub fn parse(content: &str) -> Self {
        let spokes = content
            .lines()
            .filter_map(|line| {
                let line = line.trim();
                if line.is_empty() || line.starts_with('#') {
                    return None;
                }
                let parts: Vec<&str> = line.splitn(2, ':').collect();
                if parts.len() == 2 {
                    Some(AuthorizedSpoke {
                        id52: parts[0].trim().to_string(),
                        alias: parts[1].trim().to_string(),
                    })
                } else {
                    None
                }
            })
            .collect();
        SpokesConfig { spokes }
    }

    /// Serialize SpokesConfig to spokes.txt format
    pub fn to_string(&self) -> String {
        self.spokes
            .iter()
            .map(|s| format!("{}: {}", s.id52, s.alias))
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Find a spoke by ID52
    pub fn find_by_id52(&self, id52: &str) -> Option<&AuthorizedSpoke> {
        self.spokes.iter().find(|s| s.id52 == id52)
    }

    /// Check if a spoke is authorized
    pub fn is_authorized(&self, id52: &str) -> bool {
        self.find_by_id52(id52).is_some()
    }

    /// Add a spoke (replaces if exists)
    pub fn add(&mut self, id52: &str, alias: &str) {
        // Remove existing if present
        self.spokes.retain(|s| s.id52 != id52);
        self.spokes.push(AuthorizedSpoke {
            id52: id52.to_string(),
            alias: alias.to_string(),
        });
    }

    /// Remove a spoke by ID52
    pub fn remove(&mut self, id52: &str) -> bool {
        let len_before = self.spokes.len();
        self.spokes.retain(|s| s.id52 != id52);
        self.spokes.len() < len_before
    }
}

// ============================================================================
// Hub Authorization - File-based ACL with @include support
// ============================================================================
//
// Hub authorization files are stored in:
// - Root kosha: hubs/<name>.txt
// - Other koshas: _hubs/<name>.txt
//
// File format (same as spokes.txt):
//   <id52>: <alias>     - authorize a hub with given alias
//   # comment           - comments are ignored
//   @<filename>         - include all hubs from another file
//   @ROOT/<alias>       - include from root kosha's hubs/<alias>.txt
//
// When including via @<filename>, included hubs get the includer's
// filename as their alias (for grouping purposes).
//
// Example: trusted.txt contains "@friends" - all hubs in friends.txt
// get alias "trusted" when resolved through trusted.txt.
//

/// An entry in a hub authorization file
#[derive(Debug, Clone)]
pub enum HubAuthEntry {
    /// Direct hub authorization: <id52>: <alias>
    Hub { id52: String, alias: String },
    /// Include another file: @<filename> (relative to current kosha)
    Include(String),
    /// Include from root kosha: @ROOT/<alias>
    IncludeRoot(String),
    /// Reference a single hub by alias: #<alias>
    AliasRef(String),
}

/// Parsed hub authorization file
#[derive(Debug, Clone, Default)]
pub struct HubAuthFile {
    /// The entries in this file
    pub entries: Vec<HubAuthEntry>,
}

impl HubAuthFile {
    /// Parse a hub authorization file content
    pub fn parse(content: &str) -> Self {
        let entries = content
            .lines()
            .filter_map(|line| {
                let line = line.trim();

                // Strip inline comments (` # ...` - space before # is required)
                let line = if let Some(idx) = line.find(" # ") {
                    line[..idx].trim()
                } else if line.ends_with(" #") {
                    line[..line.len() - 2].trim()
                } else {
                    line
                };

                // Skip empty lines and full-line comments (# at start with space or alone)
                if line.is_empty() || line == "#" || line.starts_with("# ") {
                    return None;
                }

                // Check for #<alias> reference (single hub by alias, no space after #)
                if let Some(alias) = line.strip_prefix('#') {
                    let alias = alias.trim();
                    if !alias.is_empty() {
                        return Some(HubAuthEntry::AliasRef(alias.to_string()));
                    }
                    return None;
                }

                // Check for @include directives
                if let Some(include) = line.strip_prefix('@') {
                    if let Some(root_path) = include.strip_prefix("ROOT/") {
                        return Some(HubAuthEntry::IncludeRoot(root_path.to_string()));
                    }
                    return Some(HubAuthEntry::Include(include.to_string()));
                }

                // Parse id52: alias format
                let parts: Vec<&str> = line.splitn(2, ':').collect();
                if parts.len() == 2 {
                    Some(HubAuthEntry::Hub {
                        id52: parts[0].trim().to_string(),
                        alias: parts[1].trim().to_string(),
                    })
                } else {
                    None
                }
            })
            .collect();
        HubAuthFile { entries }
    }

    /// Serialize to file format
    pub fn to_string(&self) -> String {
        self.entries
            .iter()
            .map(|e| match e {
                HubAuthEntry::Hub { id52, alias } => format!("{}: {}", id52, alias),
                HubAuthEntry::Include(name) => format!("@{}", name),
                HubAuthEntry::IncludeRoot(name) => format!("@ROOT/{}", name),
                HubAuthEntry::AliasRef(alias) => format!("#{}", alias),
            })
            .collect::<Vec<_>>()
            .join("\n")
    }
}

/// A resolved hub authorization entry (after processing @includes)
#[derive(Debug, Clone)]
pub struct ResolvedHubAuth {
    /// The hub's ID52
    pub id52: String,
    /// The alias to use for this hub
    pub alias: String,
    /// The file path where this hub was defined (for debugging)
    pub source_file: String,
}

/// Hub authorization resolver - resolves @includes recursively
pub struct HubAuthResolver<'a> {
    /// The root kosha for @ROOT includes
    root_kosha: &'a Kosha,
    /// The current kosha (for relative includes)
    current_kosha: Option<&'a Kosha>,
    /// Whether we're resolving from root kosha
    is_root: bool,
}

impl<'a> HubAuthResolver<'a> {
    /// Create a resolver for root kosha
    pub fn for_root(root_kosha: &'a Kosha) -> Self {
        Self {
            root_kosha,
            current_kosha: None,
            is_root: true,
        }
    }

    /// Create a resolver for a non-root kosha
    pub fn for_kosha(root_kosha: &'a Kosha, current_kosha: &'a Kosha) -> Self {
        Self {
            root_kosha,
            current_kosha: Some(current_kosha),
            is_root: false,
        }
    }

    /// Resolve a hub authorization file, returning all authorized hubs
    ///
    /// The `file_path` is relative to the hubs/ or _hubs/ folder.
    /// The `override_alias` is used when this file is included via @include.
    pub fn resolve<'b>(
        &'b self,
        file_path: &'b str,
        override_alias: Option<&'b str>,
        visited: &'b mut std::collections::HashSet<String>,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<Vec<ResolvedHubAuth>>> + Send + 'b>> {
        Box::pin(async move {
            // Prevent infinite loops
            let full_path = if self.is_root {
                format!("ROOT/hubs/{}", file_path)
            } else {
                format!("kosha/_hubs/{}", file_path)
            };

            if visited.contains(&full_path) {
                return Ok(vec![]);
            }
            visited.insert(full_path.clone());

            // Read the file
            let kosha = if self.is_root {
                self.root_kosha
            } else {
                self.current_kosha.unwrap_or(self.root_kosha)
            };

            let folder = if self.is_root { "hubs" } else { "_hubs" };
            let path = format!("{}/{}", folder, file_path);

            let content = match kosha.read_file(&path).await {
                Ok(bytes) => String::from_utf8_lossy(&bytes).to_string(),
                Err(fastn_kosha::Error::NotFound(_)) => return Ok(vec![]),
                Err(e) => return Err(Error::Kosha(e)),
            };

            let file = HubAuthFile::parse(&content);
            let mut results = Vec::new();

            // Derive alias from filename if including
            let file_alias = file_path
                .strip_suffix(".hubs")
                .unwrap_or(file_path)
                .rsplit('/')
                .next()
                .unwrap_or(file_path);

            for entry in file.entries {
                match entry {
                    HubAuthEntry::Hub { id52, alias } => {
                        // Use override alias if provided, otherwise use the original alias
                        let final_alias = override_alias.unwrap_or(&alias);
                        results.push(ResolvedHubAuth {
                            id52,
                            alias: final_alias.to_string(),
                            source_file: path.clone(),
                        });
                    }
                    HubAuthEntry::Include(name) => {
                        // Include from same folder
                        let include_path = format!("{}.hubs", name);
                        let included = self
                            .resolve(&include_path, Some(file_alias), visited)
                            .await?;
                        results.extend(included);
                    }
                    HubAuthEntry::IncludeRoot(name) => {
                        // Include from root kosha
                        let root_resolver = HubAuthResolver::for_root(self.root_kosha);
                        let include_path = format!("{}.hubs", name);
                        let included = root_resolver
                            .resolve(&include_path, Some(file_alias), visited)
                            .await?;
                        results.extend(included);
                    }
                    HubAuthEntry::AliasRef(alias) => {
                        // Reference a single hub by alias - look it up in all resolved hubs
                        // For now, we defer this - the alias lookup happens at a higher level
                        // after all files are resolved. We store it as a placeholder.
                        // TODO: Implement proper alias lookup during resolution
                        results.push(ResolvedHubAuth {
                            id52: format!("@alias:{}", alias), // Placeholder for alias lookup
                            alias: override_alias.unwrap_or(&alias).to_string(),
                            source_file: path.clone(),
                        });
                    }
                }
            }

            Ok(results)
        })
    }

    /// Resolve all hub authorizations from a folder
    pub async fn resolve_all(&self) -> Result<Vec<ResolvedHubAuth>> {
        let kosha = if self.is_root {
            self.root_kosha
        } else {
            self.current_kosha.unwrap_or(self.root_kosha)
        };

        let folder = if self.is_root { "hubs" } else { "_hubs" };

        // List all .txt files in the folder
        let entries = match kosha.list_dir(folder).await {
            Ok(entries) => entries,
            Err(fastn_kosha::Error::NotFound(_)) => return Ok(vec![]),
            Err(e) => return Err(Error::Kosha(e)),
        };

        let mut all_results = Vec::new();
        let mut visited = std::collections::HashSet::new();

        for entry in entries {
            if entry.name.ends_with(".hubs") && !entry.is_dir {
                let results = self.resolve(&entry.name, None, &mut visited).await?;
                all_results.extend(results);
            }
        }

        Ok(all_results)
    }

    /// Check if a hub ID52 is authorized
    pub async fn is_authorized(&self, id52: &str) -> Result<Option<ResolvedHubAuth>> {
        let all = self.resolve_all().await?;
        Ok(all.into_iter().find(|h| h.id52 == id52))
    }
}

/// A pending spoke connection (not yet authorized)
#[derive(Debug, Clone)]
pub struct PendingSpoke {
    pub id52: String,
    pub alias: String,
    pub first_seen: DateTime<Utc>,
    pub last_seen: DateTime<Utc>,
}

/// The Hub server - application router
pub struct Hub {
    /// Path to FASTN_HOME
    home: PathBuf,
    /// Hub's secret key
    secret_key: SecretKey,
    /// Configuration
    config: HubConfig,
    /// Authorized spokes
    spokes: SpokesConfig,
    /// Pending spokes (unauthorized, awaiting add-spoke)
    /// Key is the spoke's ID52
    pending_spokes: HashMap<String, PendingSpoke>,
    /// Root kosha for system configuration
    root_kosha: Kosha,
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
    /// creates root kosha, and writes empty spokes.txt.
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

        // Create root kosha at FASTN_HOME/koshas/root/
        let root_kosha_path = home.join("koshas").join("root");
        let root_kosha = Kosha::open(root_kosha_path, "root".to_string()).await?;

        // Write empty spokes.txt to root kosha
        let spokes_content = b"# Authorized spokes (one per line)\n# Format: <id52>: <alias>\n";
        root_kosha.write_file("spokes.txt", spokes_content).await?;

        // Create hubs/ folder with a README explaining the format
        let hubs_readme = b"# Hub Authorization Files\n\
#\n\
# This folder contains hub authorization lists (.hubs files).\n\
# Each .hubs file can contain:\n\
#\n\
#   <id52>: <alias>    - authorize a hub with given alias\n\
#   <id52>: <alias> # comment  - inline comments supported\n\
#   # full line comment        - lines starting with '# ' are comments\n\
#   @<filename>        - include all hubs from <filename>.hubs\n\
#   @ROOT/<name>       - include from root kosha's hubs/<name>.hubs\n\
#   #<alias>           - reference a single hub by its alias\n\
#\n\
# IMPORTANT: Each ID52 must be defined in exactly ONE file.\n\
# IMPORTANT: Each alias must be globally unique.\n\
#\n\
# Example:\n\
#   friends.hubs:\n\
#     ABCD...XYZ: alice  # my friend Alice\n\
#     EFGH...ABC: bob\n\
#\n\
#   family.hubs:\n\
#     IJKL...DEF: mom\n\
#     MNOP...GHI: dad\n\
#\n\
#   trusted.hubs:\n\
#     @friends           # all from friends.hubs get alias 'trusted'\n\
#     @family\n\
#     #alice             # just Alice (reference by alias)\n\
#\n\
# ACL Integration:\n\
#   _<name>.hubs corresponds to _<name>.wasm for access control.\n\
#   Example: _read.hubs lists hubs that can access _read.wasm features.\n\
";
        root_kosha.write_file("hubs/README.txt", hubs_readme).await?;

        let spokes = SpokesConfig::default();

        Ok(Self {
            home,
            secret_key,
            config,
            spokes,
            pending_spokes: HashMap::new(),
            root_kosha,
            koshas: HashMap::new(),
            acls: HashMap::new(),
        })
    }

    /// Load an existing hub
    ///
    /// Loads the secret key, configuration, and root kosha from FASTN_HOME.
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

        // Load root kosha
        let root_kosha_path = home.join("koshas").join("root");
        let root_kosha = Kosha::open(root_kosha_path, "root".to_string()).await?;

        // Load spokes.txt from root kosha
        let spokes = match root_kosha.read_file("spokes.txt").await {
            Ok(content) => {
                let content_str = String::from_utf8_lossy(&content);
                SpokesConfig::parse(&content_str)
            }
            Err(fastn_kosha::Error::NotFound(_)) => SpokesConfig::default(),
            Err(e) => return Err(Error::Kosha(e)),
        };

        Ok(Self {
            home,
            secret_key,
            config,
            spokes,
            pending_spokes: HashMap::new(),
            root_kosha,
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

    /// Record a pending spoke connection
    ///
    /// Called when an unauthorized spoke connects. Stores the alias for use
    /// when the spoke is later authorized via add_spoke().
    pub fn record_pending_spoke(&mut self, id52: &str, alias: &str) {
        let now = Utc::now();
        match self.pending_spokes.get_mut(id52) {
            Some(pending) => {
                pending.alias = alias.to_string();
                pending.last_seen = now;
            }
            None => {
                self.pending_spokes.insert(
                    id52.to_string(),
                    PendingSpoke {
                        id52: id52.to_string(),
                        alias: alias.to_string(),
                        first_seen: now,
                        last_seen: now,
                    },
                );
            }
        }
    }

    /// Get pending spokes
    pub fn pending_spokes(&self) -> &HashMap<String, PendingSpoke> {
        &self.pending_spokes
    }

    /// List pending spokes
    pub fn list_pending_spokes(&self) -> Vec<&PendingSpoke> {
        self.pending_spokes.values().collect()
    }

    /// Add an authorized spoke
    ///
    /// Uses the alias from a pending connection if available.
    /// If no pending connection exists, uses the first 8 characters of the ID52
    /// as a fallback alias.
    pub async fn add_spoke(&mut self, id52: &str) -> Result<String> {
        // Validate ID52 format
        fastn_net::from_id52(id52).map_err(|_| Error::InvalidId52(id52.to_string()))?;

        // Get alias from pending connections, or use first 8 chars as fallback
        let alias = self
            .pending_spokes
            .get(id52)
            .map(|p| p.alias.clone())
            .unwrap_or_else(|| id52[..8.min(id52.len())].to_string());

        self.spokes.add(id52, &alias);
        self.save_spokes().await?;

        // Remove from pending
        self.pending_spokes.remove(id52);

        Ok(alias)
    }

    /// Remove an authorized spoke
    pub async fn remove_spoke(&mut self, id52: &str) -> Result<bool> {
        let removed = self.spokes.remove(id52);
        if removed {
            self.save_spokes().await?;
        }
        Ok(removed)
    }

    /// Check if a spoke is authorized
    pub fn is_spoke_authorized(&self, id52: &str) -> bool {
        self.spokes.is_authorized(id52)
    }

    /// Find a spoke by ID52
    pub fn find_spoke(&self, id52: &str) -> Option<&AuthorizedSpoke> {
        self.spokes.find_by_id52(id52)
    }

    /// List all authorized spokes
    pub fn list_spokes(&self) -> &[AuthorizedSpoke] {
        &self.spokes.spokes
    }

    /// Save spokes.txt to root kosha
    async fn save_spokes(&self) -> Result<()> {
        let mut content = String::from("# Authorized spokes (one per line)\n# Format: <id52>: <alias>\n");
        if !self.spokes.spokes.is_empty() {
            content.push_str(&self.spokes.to_string());
            content.push('\n');
        }
        self.root_kosha.write_file("spokes.txt", content.as_bytes()).await?;
        Ok(())
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
    /// Spoke's alias (human-readable name)
    /// Required on first connection, optional on subsequent connections.
    /// The hub uses this to identify the spoke in pending connections.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub alias: Option<String>,
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
