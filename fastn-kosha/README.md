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
├── files/            # Current versions of all files
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

Example: `files/foo/bar.txt` at 2024-12-24 15:30:45 UTC becomes:
`history/foo~bar.txt__20241224T153045Z`

## File Operations

### Read File
```rust
kosha.read_file("path/to/file.txt").await?
// Returns: Vec<u8>
```

### Partial Read (Range-based)

Files support partial reads with open-ended ranges:

```rust
// Read bytes 100-199
kosha.read_file_range("path/to/file.txt", 100..200).await?

// Read from byte 100 to end
kosha.read_file_range("path/to/file.txt", 100..).await?

// Read first 100 bytes
kosha.read_file_range("path/to/file.txt", ..100).await?
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
// Creates final history entry, then removes from files/
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

## Watch for Changes

Watch provides a unified interface to wait for changes to files or KV keys. The watch will block until a change occurs or the timeout expires.

### Basic Watch

```rust
// Watch a single file
kosha.watch("config.json", timeout).await?

// Watch a pattern (glob-style)
kosha.watch("logs/*", timeout).await?

// Watch a KV key
kosha.watch("settings/theme", timeout).await?
```

### Conditional Watch (If-Modified-Since)

If you provide a `last_modified` timestamp, the watch returns immediately if the target is already newer:

```rust
// Only wait if file hasn't changed since our last read
kosha.watch_since("config.json", last_modified, timeout).await?
// Returns immediately if config.json was modified after last_modified
```

### Watch JSON Path in KV Keys

For KV keys containing JSON, you can watch a specific JSON path within the value:

```rust
// Watch only the "theme" field inside the "settings" key
kosha.watch_json_path("settings", "$.theme", timeout).await?

// Watch nested paths
kosha.watch_json_path("user/preferences", "$.display.fontSize", timeout).await?
```

### Watch Response

```rust
pub struct WatchResult {
    pub path: String,           // Which path triggered the watch
    pub modified: DateTime<Utc>, // When it was modified
    pub is_file: bool,          // true for file, false for KV key
}
```

### Important Notes

- **Unified watch, separate read/write**: While watch works the same for files and KV keys, the actual read/write APIs remain separate (`read_file` vs `kv_get`, `write_file` vs `kv_set`).
- **ACL**: Watch follows the same ACL rules as read operations. If you can't read a path, you can't watch it.
- **Pattern matching**: Glob patterns like `foo/*` match both files and KV keys in that namespace.
- **Timeout**: Always specify a reasonable timeout to avoid indefinite blocking.

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

## Read/Write vs Get/Post

Kosha provides two sets of operations with different semantics:

### Read/Write - Raw Byte Operations

For directly manipulating file content as bytes. Uses **exact paths** - no fallback logic, no WASM execution.

```rust
// Read raw bytes - exact path only
kosha.read_file("config.json").await?  // Returns: Vec<u8>
kosha.read_file("index.wasm").await?   // Returns raw WASM bytes, NOT executed

// Write raw bytes
kosha.write_file("config.json", bytes).await?
```

**Key points:**
- `read_file("index.wasm")` returns the raw WASM bytes, it does NOT execute the WASM
- `read_file("foo/")` would fail - directories have no raw bytes
- No content-type handling, no caching headers

### Get/Post - HTTP-like Semantics

For HTTP-style requests with content-type handling, WASM execution, and caching.
Uses **fallback logic** for path resolution.

```rust
// Get with content-type and caching
kosha.get("config.json").await?
// Returns: Response { content_type: "application/json", body: ..., cache_control: Some(...) }

// Get directory - uses fallback logic
kosha.get("api/").await?
// Tries: api.wasm → api/index.wasm → 404

// Post with payload
kosha.post("api/users", payload).await?
// Returns: Response
```

**Key points:**
- `get("index.wasm")` executes the WASM and returns its response
- `get("api/")` uses fallback logic: `api.wasm` then `api/index.wasm`
- Response includes cache headers (cache_control, etag) that clients can use
- WASM modules can set their own cache headers in the response

### WASM Execution via Get/Post

Any `.wasm` file (except `_`-prefixed special files) can handle get/post requests:

1. **Direct WASM**: `foo.wasm` handles requests to `/foo.wasm`
2. **File Handler**: `foo.json.wasm` handles requests to `/foo.json`
3. **Directory Handler**: For `/foo/`, tries `foo.wasm` then `foo/index.wasm`

**Important Constraints**:
- `foo.json` and `foo.json.wasm` cannot both exist (write/rename fails if conflict)
- `foo.wasm` and `foo/index.wasm` cannot both exist

```rust
// Request to /api/data.json
// If api/data.json.wasm exists → execute it
// Else if api/data.json exists → return with content-type: application/json

// Request to /api/users/
// If api/users.wasm exists → execute it
// Else if api/users/index.wasm exists → execute it
// Else → 404
```

### Response Type

Get/Post operations return a `Response` with optional caching headers:

```rust
pub struct Response {
    pub content_type: String,
    pub body: ResponseBody,
    pub cache_control: Option<String>,  // e.g., "max-age=3600", "no-cache"
    pub etag: Option<String>,           // For conditional requests
}

pub enum ResponseBody {
    /// Raw bytes
    Bytes(Vec<u8>),
    /// JSON value
    Json(serde_json::Value),
    /// Redirect to another path
    Redirect(String),
    /// Not found
    NotFound,
}
```

WASM modules can set cache headers in their response:
```rust
Response::json(data)
    .with_cache_control("max-age=3600")
    .with_etag("abc123")
```

### Content-Type Mapping

For static files, content-type is derived from extension:
- `.json` → `application/json`
- `.html` → `text/html`
- `.txt` → `text/plain`
- `.png` → `image/png`
- etc.

## Access Control (ACL)

Access control is managed via special WASM modules (prefixed with `_`) stored in the kosha itself. ACL modules can be placed at any folder level and apply to everything within that folder.

**Special files** (prefixed with `_`) are reserved for system use:
- `_access.wasm` - General access control
- `_read.wasm` - Read access control
- `_write.wasm` - Write access control
- `_admin.wasm` - Admin access (for modifying ACL files)

Note: `index.wasm` is NOT a special file - it's a regular executable used for directory handling.

### Hub Authorization Files (`.hubs`)

Each ACL WASM module can have a corresponding `.hubs` file that lists authorized hubs:
- `_access.hubs` - lists hubs that can pass `_access.wasm` ACL
- `_read.hubs` - lists hubs for `_read.wasm` checks
- `_write.hubs` - lists hubs for `_write.wasm` checks
- `_admin.hubs` - lists hubs for `_admin.wasm` admin access

The `.hubs` file provides a declarative list that the WASM module can query. This enables simple ACL patterns without hardcoding hub IDs in WASM:

```rust
// In _access.wasm
fn allowed(ctx: &AccessContext) -> bool {
    // Check if requester is in the _access.hubs file
    ctx.is_hub_authorized("_access.hubs")
}
```

**File format** (same as hub authorization files):
```
# Authorized hubs
ABCD...XYZ: alice       # Direct hub entry
EFGH...ABC: bob  # inline comment
@friends                # Include from friends.hubs
@ROOT/trusted           # Include from root kosha's hubs/trusted.hubs
#alice                  # Reference single hub by alias
```

**Uniqueness constraints:**
- Each ID52 must be defined in exactly **one** `.hubs` file
- Each alias must be **globally unique** across all files

### Unified Namespace

Files, KV keys, and databases share the same namespace and are subject to the same ACL rules:

- A `_access.wasm` at `foo/` controls access to:
  - Files: `foo/bar.txt`, `foo/baz/file.json`, etc.
  - KV keys: `foo/counter`, `foo/settings`, etc.
  - Databases: `foo/data.sqlite3` (SELECT = read, INSERT/UPDATE/DELETE = write)

- More specific ACL files (`_read.wasm`, `_write.wasm`) take precedence over `_access.wasm`

- Database operations map to read/write:
  - **Read operations**: `db_query` (SELECT)
  - **Write operations**: `db_execute` (INSERT, UPDATE, DELETE), `db_begin`, `db_commit`, `db_rollback`

### Admin Access (Modifying ACL)

ACL WASM files (`_access.wasm`, `_read.wasm`, `_write.wasm`) are protected by `_admin.wasm`:

- To create, modify, or delete any ACL file at `foo/_access.wasm`, the system checks `foo/_admin.wasm`
- If no `_admin.wasm` exists at that level, it checks parent directories up to root
- If no `_admin.wasm` exists anywhere, only the hub owner can modify ACL files

```
mykosha/
├── files/
│   ├── _admin.wasm          # Controls who can modify ACL at root and below
│   ├── _access.wasm         # Protected by _admin.wasm
│   └── private/
│       ├── _admin.wasm      # Can override root admin for private/*
│       └── _access.wasm     # Protected by private/_admin.wasm
```

### Important Constraints

1. **No path collision**: The same path cannot be used as both a file and a KV key. If `foo/bar` exists as a file, you cannot use `foo/bar` as a KV key (and vice versa).

2. **Hierarchical checking (like Linux permissions)**: ACL is checked from root to the target path. Each level must grant access before proceeding to the next. Any denial stops access immediately.

   For a request to `api/data/users.sqlite3`:
   ```
   1. Check root kosha ACL (FASTN_HOME/koshas/root/files/_access.wasm)
   2. Check target kosha root (_access.wasm at kosha root)
   3. Check api/_access.wasm (or _read/_write.wasm)
   4. Check api/data/_access.wasm (or _read/_write.wasm)
   5. Access granted only if ALL checks pass
   ```

   If no ACL file exists at a level, that level is skipped (implicitly allowed).
   If a `.hubs` file exists without a `.wasm` file, the hub list is checked directly.

3. **ACL module signature**: Each ACL WASM module exports an `allowed(ctx_json: &str) -> bool` function that receives the access context (spoke ID, command, path, etc.).

4. **Admin protection**: Writes to special `_*.wasm` files require admin permission checked via `_admin.wasm`.

5. **Reserved prefix**: Files starting with `_` are reserved for system use. Regular `.wasm` files (without `_` prefix) are user-executable.

### Example

```
mykosha/
├── files/
│   ├── _access.wasm         # Root ACL (WASM module)
│   ├── _access.hubs         # Hubs authorized for root access
│   ├── api.wasm             # Handles GET/POST /api/ (alternative to api/index.wasm)
│   ├── api/
│   │   ├── _read.wasm       # Read ACL for api/*
│   │   ├── _read.hubs       # Hubs authorized to read api/*
│   │   ├── users.wasm       # Handles GET/POST /api/users/
│   │   ├── config.json      # Static file: GET returns with content-type: application/json
│   │   └── data/
│   │       ├── index.wasm   # Handles GET/POST /api/data/
│   │       └── stats.json.wasm  # Handles GET/POST /api/data/stats.json (dynamic)
│   └── private/
│       ├── _access.wasm     # Controls all access to private/*
│       ├── _access.hubs     # Hubs authorized for private/*
│       ├── secrets.txt      # Static file
│       └── config/
│           ├── _write.wasm  # Additional write restrictions for config/*
│           └── _write.hubs  # Hubs authorized to write to config/*
└── kv/
    └── store.dson           # Keys like "private/counter" also checked by private/_access.wasm
```

Note: In the above example:
- `api/data/stats.json.wasm` handles requests for `/api/data/stats.json`
- `api/data/stats.json` must NOT exist (conflict error on write/rename)

## SQLite Databases

Any file with `.sqlite3` extension in the kosha is treated as a SQLite database. Databases can be placed anywhere in the file hierarchy.

### Directory Structure

```
<kosha-path>/
├── files/
│   ├── users.sqlite3           # Database in root
│   ├── api/
│   │   ├── _read.wasm          # Read ACL (controls db_query)
│   │   ├── _write.wasm         # Write ACL (controls db_execute)
│   │   └── analytics.sqlite3   # Database in api/
│   └── private/
│       ├── _access.wasm        # Access ACL for private/*
│       └── data.sqlite3        # Database in private/
├── history/
└── kv/
```

### Database Operations

```rust
// Query (read-only, returns rows)
kosha.db_query("users.sqlite3", "SELECT * FROM users WHERE id = ?", params![1]).await?
// Returns: Vec<Row>

// Query in subdirectory
kosha.db_query("api/analytics.sqlite3", "SELECT * FROM events", params![]).await?

// Execute (write, returns affected rows)
kosha.db_execute("users.sqlite3", "INSERT INTO users (name) VALUES (?)", params!["Alice"]).await?
// Returns: usize (rows affected)
```

### Transactions

Transactions provide atomic multi-statement operations:

```rust
// Begin a transaction (returns transaction ID)
let tx_id = kosha.db_begin("users.db").await?;

// Execute within transaction
kosha.db_tx_execute(tx_id, "INSERT INTO users (name) VALUES (?)", params!["Alice"]).await?;
kosha.db_tx_execute(tx_id, "UPDATE counters SET count = count + 1", params![]).await?;

// Commit (or rollback)
kosha.db_commit(tx_id).await?;
// kosha.db_rollback(tx_id).await?;
```

**Transaction Limits:**
- Maximum transaction duration: configurable (default 30 seconds)
- Transactions that exceed the limit are automatically rolled back
- Hub serializes all write operations - no concurrent write issues

### Database ACL

Databases use the same ACL as files. The `resource_type` field in `AccessContext` indicates when a database is being accessed:

- `db_query` (SELECT) → checks `_read.wasm` / `_read.hubs`
- `db_execute` (INSERT/UPDATE/DELETE) → checks `_write.wasm` / `_write.hubs`
- `db_begin`, `db_commit`, `db_rollback` → checks `_write.wasm` / `_write.hubs`

ACL resolution for `api/analytics.sqlite3`:
1. Check `api/_read.wasm` (for query) or `api/_write.wasm` (for execute)
2. If not found, check parent directories up to root
3. Fall back to `_access.wasm` if no specific read/write ACL exists

## WASM Execution Context

All WASM modules (ACL and get/post handlers) receive context about the request:

### Access Control Context

```rust
pub struct AccessContext {
    pub requester_hub_id: String,  // Hub ID of the requesting spoke
    pub current_hub_id: String,    // This hub's ID (same = local user)
    pub spoke_id52: String,        // Spoke's public key ID
    pub app: String,               // Application (e.g., "kosha")
    pub instance: String,          // Instance name (e.g., kosha alias)
    pub command: String,           // Command being executed
    pub resource_type: String,     // "file", "db", or "kv"
    pub path: String,              // Full path to the resource being accessed
}

// Check if request is from the same user (hub owner)
fn is_owner(ctx: &AccessContext) -> bool {
    ctx.requester_hub_id == ctx.current_hub_id
}
```

### Get/Post Handler Context

```rust
pub struct RequestContext {
    pub requester_hub_id: String,  // Hub ID of the requesting spoke
    pub current_hub_id: String,    // This hub's ID
    pub spoke_id52: String,        // Spoke's public key ID
    pub method: String,            // "GET" or "POST"
    pub path: String,              // Request path
    pub query: Option<String>,     // Query string
    pub payload: Option<Value>,    // POST payload (JSON)
}
```

### Hub Identity Model

- Each user has their own hub (hubs are not shared)
- `requester_hub_id == current_hub_id` means the request is from the hub owner
- This enables simple "is owner" checks for private data

## Design Notes

- **Timestamps are hub-generated**: The hub assigns timestamps when files are written, ensuring consistent ordering across the network.
- **History is immutable**: Once a version is created in history/, it is never modified.
- **CRDT merges**: When koshas sync between hubs, the dson KV store can merge without conflicts.
- **Spoke access**: Spokes access koshas through the hub API, not directly on disk.
- **Unified namespace**: Files and KV keys share the same path namespace for consistent ACL enforcement.
- **Serialized writes**: Hub serializes all write operations (files, KV, SQLite) - no concurrent write issues.
- **One hub per user**: Each user runs their own hub, simplifying ownership checks.
