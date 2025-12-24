# fastn-kosha

Versioned file system and CRDT key-value store abstraction.

## Overview

A **Kosha** (Sanskrit for "treasury" or "storehouse") is a storage abstraction that provides:

1. **Versioned File System** - Files with full history tracking
2. **CRDT Key-Value Store** - Conflict-free replicated data using dson

## Directory Structure

Each kosha is stored on disk with the following layout:

```
<kosha-path>/
├── src/              # Current versions of all files
│   ├── foo.txt
│   └── bar/
│       └── baz.json
├── history/          # Historical versions (flat structure)
│   ├── foo.txt__20241224T153045Z
│   ├── foo.txt__20241224T160012Z
│   └── bar~baz.json__20241224T153045Z
└── kv/               # Key-value store (dson backed)
    └── store.dson
```

### History File Naming Convention

History files use a flat naming scheme:
- Path separators (`/`) are replaced with `~`
- Timestamp is appended after `__` separator
- Format: `<flattened-path>__<ISO8601-timestamp>`

Example: `src/foo/bar.txt` at 2024-12-24 15:30:45 UTC becomes:
`history/foo~bar.txt__20241224T153045Z`

## File Operations

### Read File
```rust
kosha.read_file("path/to/file.txt").await?
// Returns: Vec<u8>
```

### Write File (with history)
```rust
kosha.write_file("path/to/file.txt", content).await?
// Automatically creates history entry before overwriting
```

### List Directory
```rust
kosha.list_dir("path/to/dir").await?
// Returns: Vec<DirEntry>
```

### Get File Versions
```rust
kosha.get_versions("path/to/file.txt").await?
// Returns: Vec<FileVersion> with timestamps
```

### Read Specific Version
```rust
kosha.read_version("path/to/file.txt", timestamp).await?
// Returns: Vec<u8>
```

### Rename File
```rust
kosha.rename("old/path.txt", "new/path.txt").await?
// Preserves history
```

### Delete File
```rust
kosha.delete("path/to/file.txt").await?
// Creates final history entry, then removes from src/
```

## Key-Value Operations

The KV store uses dson for CRDT semantics, allowing conflict-free merges.

### Get Key
```rust
kosha.kv_get("my-key").await?
// Returns: Option<Value>
```

### Set Key
```rust
kosha.kv_set("my-key", value).await?
```

### Delete Key
```rust
kosha.kv_delete("my-key").await?
```

### Transaction (dson semantics)
```rust
kosha.kv_transaction(|tx| {
    let val = tx.get("counter")?;
    tx.set("counter", val + 1)?;
    Ok(())
}).await?
```

## API Types

```rust
pub struct Kosha {
    path: PathBuf,
    alias: String,
}

pub struct FileVersion {
    pub timestamp: DateTime<Utc>,
    pub size: u64,
}

pub struct DirEntry {
    pub name: String,
    pub is_dir: bool,
    pub size: u64,
    pub modified: DateTime<Utc>,
}
```

## Access Control (ACL)

Access control is managed via WASM modules stored in the kosha itself. ACL modules can be placed at any folder level and apply to everything within that folder.

### Unified Namespace

Files and KV keys share the same namespace and are subject to the same ACL rules:

- An `access.wasm` at `foo/` controls access to both:
  - Files: `foo/bar.txt`, `foo/baz/file.json`, etc.
  - KV keys: `foo/counter`, `foo/settings`, etc.

- More specific ACL files (`read.wasm`, `write.wasm`) take precedence over `access.wasm`

### Important Constraints

1. **No path collision**: The same path cannot be used as both a file and a KV key. If `foo/bar` exists as a file, you cannot use `foo/bar` as a KV key (and vice versa).

2. **Hierarchical checking**: ACL is checked from root to the target path. Any denial at any level stops access immediately.

3. **ACL module signature**: Each WASM module exports an `allowed(ctx_json: &str) -> bool` function that receives the access context (spoke ID, command, path, etc.).

### Example

```
mykosha/
├── src/
│   ├── public/
│   │   └── readme.txt
│   └── private/
│       ├── access.wasm      # Controls all access to private/*
│       ├── secrets.txt
│       └── config/
│           └── write.wasm   # Additional write restrictions for config/*
└── kv/
    └── store.dson           # Keys like "private/counter" also checked by private/access.wasm
```

## Design Notes

- **Timestamps are hub-generated**: The hub assigns timestamps when files are written, ensuring consistent ordering across the network.
- **History is immutable**: Once a version is created in history/, it is never modified.
- **CRDT merges**: When koshas sync between hubs, the dson KV store can merge without conflicts.
- **Spoke access**: Spokes access koshas through the hub API, not directly on disk.
- **Unified namespace**: Files and KV keys share the same path namespace for consistent ACL enforcement.
