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
}

pub type Result<T> = std::result::Result<T, Error>;

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
        tokio::fs::create_dir_all(path.join("src")).await?;
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

    // File operations - to be implemented

    /// Read a file from src/
    pub async fn read_file(&self, _path: &str) -> Result<Vec<u8>> {
        todo!("read_file")
    }

    /// Write a file to src/, creating history entry
    pub async fn write_file(&self, _path: &str, _content: &[u8]) -> Result<()> {
        todo!("write_file")
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
