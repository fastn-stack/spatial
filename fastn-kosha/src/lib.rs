//! Versioned file system and CRDT key-value store abstraction
//!
//! A Kosha provides:
//! - Versioned file storage with automatic history tracking
//! - CRDT-based key-value store using dson
//!
//! See README.md for full documentation.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use thiserror::Error;

/// Error types for kosha operations
#[derive(Error, Debug)]
pub enum Error {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("File not found: {0}")]
    NotFound(String),

    #[error("Invalid path: {0}")]
    InvalidPath(String),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Conflict: {0}")]
    Conflict(String),

    #[error("WASM execution error: {0}")]
    WasmExecution(String),
}

pub type Result<T> = std::result::Result<T, Error>;

// ============================================================================
// Response types for get/post operations
// ============================================================================

/// Response from get/post operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Response {
    /// Content type (e.g., "application/json", "text/html")
    pub content_type: String,
    /// Response body
    pub body: ResponseBody,
    /// Optional cache control header (e.g., "max-age=3600", "no-cache")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache_control: Option<String>,
    /// Optional ETag for conditional requests
    #[serde(skip_serializing_if = "Option::is_none")]
    pub etag: Option<String>,
}

/// Response body variants
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum ResponseBody {
    /// Raw bytes (base64 encoded in JSON)
    Bytes(Vec<u8>),
    /// JSON value
    Json(serde_json::Value),
    /// Redirect to another path
    Redirect(String),
    /// Not found
    NotFound,
}

impl Response {
    /// Create a JSON response
    pub fn json(value: serde_json::Value) -> Self {
        Self {
            content_type: "application/json".to_string(),
            body: ResponseBody::Json(value),
            cache_control: None,
            etag: None,
        }
    }

    /// Create a bytes response with content type
    pub fn bytes(content_type: &str, data: Vec<u8>) -> Self {
        Self {
            content_type: content_type.to_string(),
            body: ResponseBody::Bytes(data),
            cache_control: None,
            etag: None,
        }
    }

    /// Create a redirect response
    pub fn redirect(path: &str) -> Self {
        Self {
            content_type: "".to_string(),
            body: ResponseBody::Redirect(path.to_string()),
            cache_control: None,
            etag: None,
        }
    }

    /// Create a not found response
    pub fn not_found() -> Self {
        Self {
            content_type: "".to_string(),
            body: ResponseBody::NotFound,
            cache_control: None,
            etag: None,
        }
    }

    /// Set cache control header
    pub fn with_cache_control(mut self, cache_control: &str) -> Self {
        self.cache_control = Some(cache_control.to_string());
        self
    }

    /// Set ETag header
    pub fn with_etag(mut self, etag: &str) -> Self {
        self.etag = Some(etag.to_string());
        self
    }
}

/// Get content type from file extension
pub fn content_type_for_extension(path: &str) -> &'static str {
    let ext = path.rsplit('.').next().unwrap_or("");
    match ext.to_lowercase().as_str() {
        "json" => "application/json",
        "html" | "htm" => "text/html",
        "css" => "text/css",
        "js" => "application/javascript",
        "txt" => "text/plain",
        "xml" => "application/xml",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "svg" => "image/svg+xml",
        "webp" => "image/webp",
        "ico" => "image/x-icon",
        "woff" => "font/woff",
        "woff2" => "font/woff2",
        "ttf" => "font/ttf",
        "otf" => "font/otf",
        "pdf" => "application/pdf",
        "zip" => "application/zip",
        "wasm" => "application/wasm",
        "glb" => "model/gltf-binary",
        "gltf" => "model/gltf+json",
        _ => "application/octet-stream",
    }
}

/// A file version in history
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileVersion {
    pub timestamp: DateTime<Utc>,
    pub size: u64,
}

/// A directory entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DirEntry {
    pub name: String,
    pub is_dir: bool,
    pub size: u64,
    pub modified: DateTime<Utc>,
}

/// A Kosha - versioned file system with key-value store
#[derive(Clone)]
pub struct Kosha {
    /// Root path of this kosha on disk
    path: PathBuf,
    /// Unique alias for this kosha within a hub
    alias: String,
}

impl Kosha {
    /// Create or open a kosha at the given path
    pub async fn open(path: PathBuf, alias: String) -> Result<Self> {
        // Ensure directories exist
        tokio::fs::create_dir_all(path.join("files")).await?;
        tokio::fs::create_dir_all(path.join("history")).await?;
        tokio::fs::create_dir_all(path.join("kv")).await?;

        Ok(Self { path, alias })
    }

    /// Get the alias of this kosha
    pub fn alias(&self) -> &str {
        &self.alias
    }

    /// Get the root path
    pub fn path(&self) -> &PathBuf {
        &self.path
    }

    /// Get the files directory path
    fn files_path(&self) -> PathBuf {
        self.path.join("files")
    }

    /// Validate and sanitize a file path to prevent directory traversal
    fn validate_path(&self, path: &str) -> Result<PathBuf> {
        // Remove leading slashes
        let clean_path = path.trim_start_matches('/');

        // Check for directory traversal attempts
        if clean_path.contains("..") {
            return Err(Error::InvalidPath("Path cannot contain '..'".to_string()));
        }

        // Build full path
        let full_path = self.files_path().join(clean_path);

        // Verify the path is within files directory
        if !full_path.starts_with(&self.files_path()) {
            return Err(Error::InvalidPath("Path escapes kosha directory".to_string()));
        }

        Ok(full_path)
    }

    // File operations

    /// Read a file from files/
    pub async fn read_file(&self, path: &str) -> Result<Vec<u8>> {
        let full_path = self.validate_path(path)?;

        if !full_path.exists() {
            return Err(Error::NotFound(path.to_string()));
        }

        tokio::fs::read(&full_path)
            .await
            .map_err(|e| Error::Io(e))
    }

    /// Write a file to files/, creating history entry
    /// For now, history is not implemented - just writes the file
    pub async fn write_file(&self, path: &str, content: &[u8]) -> Result<()> {
        let full_path = self.validate_path(path)?;

        // Create parent directories if needed
        if let Some(parent) = full_path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        // TODO: Create history entry before overwriting

        tokio::fs::write(&full_path, content).await?;
        Ok(())
    }

    /// List directory contents
    pub async fn list_dir(&self, _path: &str) -> Result<Vec<DirEntry>> {
        todo!("list_dir")
    }

    /// Get all versions of a file
    pub async fn get_versions(&self, _path: &str) -> Result<Vec<FileVersion>> {
        todo!("get_versions")
    }

    /// Read a specific version from history
    pub async fn read_version(&self, _path: &str, _timestamp: DateTime<Utc>) -> Result<Vec<u8>> {
        todo!("read_version")
    }

    /// Rename a file
    pub async fn rename(&self, _from: &str, _to: &str) -> Result<()> {
        todo!("rename")
    }

    /// Delete a file (creates final history entry)
    pub async fn delete(&self, _path: &str) -> Result<()> {
        todo!("delete")
    }

    // Key-value operations - to be implemented

    /// Get a value from the KV store
    pub async fn kv_get(&self, _key: &str) -> Result<Option<serde_json::Value>> {
        todo!("kv_get")
    }

    /// Set a value in the KV store
    pub async fn kv_set(&self, _key: &str, _value: serde_json::Value) -> Result<()> {
        todo!("kv_set")
    }

    /// Delete a key from the KV store
    pub async fn kv_delete(&self, _key: &str) -> Result<()> {
        todo!("kv_delete")
    }

    // ========================================================================
    // Get/Post operations - HTTP-like semantics with WASM execution
    // ========================================================================

    /// Handle a GET request with HTTP-like semantics
    ///
    /// Resolution order:
    /// 1. If path ends with `/`, try `{path}.wasm`, then `{path}index.wasm`
    /// 2. If `{path}.wasm` exists, execute it
    /// 3. Otherwise, serve static file with appropriate content-type
    pub async fn get(&self, _path: &str) -> Result<Response> {
        todo!("get")
    }

    /// Handle a POST request with HTTP-like semantics
    ///
    /// Resolution order (same as GET):
    /// 1. If path ends with `/`, try `{path}.wasm`, then `{path}index.wasm`
    /// 2. If `{path}.wasm` exists, execute it with payload
    /// 3. Otherwise, error (can't POST to static files)
    pub async fn post(&self, _path: &str, _payload: serde_json::Value) -> Result<Response> {
        todo!("post")
    }

    /// Check if writing to a path would create a conflict
    ///
    /// Returns error if:
    /// - Writing `foo.json` when `foo.json.wasm` exists
    /// - Writing `foo.json.wasm` when `foo.json` exists
    /// - Writing `foo.wasm` when `foo/index.wasm` exists
    /// - Writing `foo/index.wasm` when `foo.wasm` exists
    pub async fn check_write_conflict(&self, _path: &str) -> Result<()> {
        todo!("check_write_conflict")
    }

    // ========================================================================
    // SQLite database operations
    // ========================================================================

    /// Execute a read-only query on a database
    ///
    /// Returns rows as JSON arrays
    pub async fn db_query(
        &self,
        _database: &str,
        _sql: &str,
        _params: Vec<serde_json::Value>,
    ) -> Result<Vec<serde_json::Value>> {
        todo!("db_query")
    }

    /// Execute a write statement on a database
    ///
    /// Returns the number of affected rows
    pub async fn db_execute(
        &self,
        _database: &str,
        _sql: &str,
        _params: Vec<serde_json::Value>,
    ) -> Result<usize> {
        todo!("db_execute")
    }

    /// Begin a database transaction
    ///
    /// Returns a transaction ID. Transactions have a maximum duration
    /// (default 30 seconds) after which they are automatically rolled back.
    pub async fn db_begin(&self, _database: &str) -> Result<String> {
        todo!("db_begin")
    }

    /// Execute a statement within a transaction
    pub async fn db_tx_execute(
        &self,
        _tx_id: &str,
        _sql: &str,
        _params: Vec<serde_json::Value>,
    ) -> Result<usize> {
        todo!("db_tx_execute")
    }

    /// Query within a transaction
    pub async fn db_tx_query(
        &self,
        _tx_id: &str,
        _sql: &str,
        _params: Vec<serde_json::Value>,
    ) -> Result<Vec<serde_json::Value>> {
        todo!("db_tx_query")
    }

    /// Commit a transaction
    pub async fn db_commit(&self, _tx_id: &str) -> Result<()> {
        todo!("db_commit")
    }

    /// Rollback a transaction
    pub async fn db_rollback(&self, _tx_id: &str) -> Result<()> {
        todo!("db_rollback")
    }
}

/// Convert a file path to a flat history filename
/// e.g., "foo/bar/baz.txt" -> "foo~bar~baz.txt"
pub fn flatten_path(path: &str) -> String {
    path.replace('/', "~")
}

/// Convert a flat history filename back to a path
/// e.g., "foo~bar~baz.txt" -> "foo/bar/baz.txt"
pub fn unflatten_path(flat: &str) -> String {
    flat.replace('~', "/")
}

/// Generate a history filename for a given path and timestamp
pub fn history_filename(path: &str, timestamp: DateTime<Utc>) -> String {
    let flat = flatten_path(path);
    let ts = timestamp.format("%Y%m%dT%H%M%SZ");
    format!("{}__{}", flat, ts)
}

impl Kosha {
    /// Handle a command from the hub router
    ///
    /// Commands:
    /// - read_file: { path: string } -> { content: base64, modified: timestamp }
    /// - write_file: { path: string, content: base64, base_version?: timestamp } -> { modified: timestamp }
    /// - list_dir: { path: string } -> { entries: [...] }
    /// - get_versions: { path: string } -> { versions: [...] }
    /// - read_version: { path: string, timestamp: string } -> { content: base64 }
    /// - rename: { from: string, to: string } -> {}
    /// - delete: { path: string } -> {}
    /// - kv_get: { key: string } -> { value: json | null }
    /// - kv_set: { key: string, value: json } -> {}
    /// - kv_delete: { key: string } -> {}
    pub async fn handle_command(
        &self,
        command: &str,
        payload: serde_json::Value,
    ) -> std::result::Result<serde_json::Value, String> {
        match command {
            "read_file" => {
                let path = payload.get("path")
                    .and_then(|v| v.as_str())
                    .ok_or("missing 'path' field")?;
                let content = self.read_file(path).await.map_err(|e| e.to_string())?;
                // Return base64 encoded content
                Ok(serde_json::json!({
                    "content": base64_encode(&content),
                }))
            }
            "write_file" => {
                let path = payload.get("path")
                    .and_then(|v| v.as_str())
                    .ok_or("missing 'path' field")?;
                let content_b64 = payload.get("content")
                    .and_then(|v| v.as_str())
                    .ok_or("missing 'content' field")?;
                let content = base64_decode(content_b64)
                    .map_err(|e| format!("invalid base64: {}", e))?;
                let _base_version = payload.get("base_version")
                    .and_then(|v| v.as_str());
                // TODO: implement optimistic locking with base_version
                self.write_file(path, &content).await.map_err(|e| e.to_string())?;
                Ok(serde_json::json!({
                    "modified": Utc::now(),
                }))
            }
            "list_dir" => {
                let path = payload.get("path")
                    .and_then(|v| v.as_str())
                    .ok_or("missing 'path' field")?;
                let entries = self.list_dir(path).await.map_err(|e| e.to_string())?;
                Ok(serde_json::json!({ "entries": entries }))
            }
            "get_versions" => {
                let path = payload.get("path")
                    .and_then(|v| v.as_str())
                    .ok_or("missing 'path' field")?;
                let versions = self.get_versions(path).await.map_err(|e| e.to_string())?;
                Ok(serde_json::json!({ "versions": versions }))
            }
            "read_version" => {
                let path = payload.get("path")
                    .and_then(|v| v.as_str())
                    .ok_or("missing 'path' field")?;
                let timestamp_str = payload.get("timestamp")
                    .and_then(|v| v.as_str())
                    .ok_or("missing 'timestamp' field")?;
                let timestamp: DateTime<Utc> = timestamp_str.parse()
                    .map_err(|e| format!("invalid timestamp: {}", e))?;
                let content = self.read_version(path, timestamp).await.map_err(|e| e.to_string())?;
                Ok(serde_json::json!({
                    "content": base64_encode(&content),
                }))
            }
            "rename" => {
                let from = payload.get("from")
                    .and_then(|v| v.as_str())
                    .ok_or("missing 'from' field")?;
                let to = payload.get("to")
                    .and_then(|v| v.as_str())
                    .ok_or("missing 'to' field")?;
                self.rename(from, to).await.map_err(|e| e.to_string())?;
                Ok(serde_json::json!({}))
            }
            "delete" => {
                let path = payload.get("path")
                    .and_then(|v| v.as_str())
                    .ok_or("missing 'path' field")?;
                self.delete(path).await.map_err(|e| e.to_string())?;
                Ok(serde_json::json!({}))
            }
            "kv_get" => {
                let key = payload.get("key")
                    .and_then(|v| v.as_str())
                    .ok_or("missing 'key' field")?;
                let value = self.kv_get(key).await.map_err(|e| e.to_string())?;
                Ok(serde_json::json!({ "value": value }))
            }
            "kv_set" => {
                let key = payload.get("key")
                    .and_then(|v| v.as_str())
                    .ok_or("missing 'key' field")?;
                let value = payload.get("value")
                    .cloned()
                    .ok_or("missing 'value' field")?;
                self.kv_set(key, value).await.map_err(|e| e.to_string())?;
                Ok(serde_json::json!({}))
            }
            "kv_delete" => {
                let key = payload.get("key")
                    .and_then(|v| v.as_str())
                    .ok_or("missing 'key' field")?;
                self.kv_delete(key).await.map_err(|e| e.to_string())?;
                Ok(serde_json::json!({}))
            }
            _ => Err(format!("unknown command: {}", command)),
        }
    }
}

// Base64 encoding/decoding helpers
fn base64_encode(data: &[u8]) -> String {
    use base64::Engine;
    base64::engine::general_purpose::STANDARD.encode(data)
}

fn base64_decode(s: &str) -> std::result::Result<Vec<u8>, base64::DecodeError> {
    use base64::Engine;
    base64::engine::general_purpose::STANDARD.decode(s)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_flatten_path() {
        assert_eq!(flatten_path("foo/bar/baz.txt"), "foo~bar~baz.txt");
        assert_eq!(flatten_path("simple.txt"), "simple.txt");
    }

    #[test]
    fn test_unflatten_path() {
        assert_eq!(unflatten_path("foo~bar~baz.txt"), "foo/bar/baz.txt");
    }
}
