# fastn-hub

Hub server for the fastn P2P network. Manages koshas (versioned storage) and authenticated spokes.

## Overview

A **Hub** is a P2P server that:
- Listens for connections from authenticated spokes
- Manages one or more koshas (storage units)
- Authorizes spokes before allowing connections
- Routes requests to the appropriate application handler

## FASTN_HOME Directory

The hub stores its configuration and data in `FASTN_HOME`:
- Environment variable: `FASTN_HOME`
- Default: `~/.fastn` (or platform-specific config directory)

```
$FASTN_HOME/
├── hub.key           # Hub's secret key (Ed25519)
├── config.json       # Hub configuration
└── koshas/           # Kosha storage
    ├── root/         # Root kosha (system config)
    │   ├── files/
    │   │   ├── spokes.txt       # Authorized spokes (id52: alias)
    │   │   └── hubs/            # Hub authorization lists
    │   │       ├── README.txt   # Format documentation
    │   │       ├── friends.txt  # Example: friend hubs
    │   │       └── trusted.txt  # Example: combined list
    │   ├── history/
    │   └── kv/
    └── my-kosha/     # User koshas
        ├── files/
        │   └── _hubs/           # Per-kosha hub authorization
        ├── history/
        └── kv/
```

## CLI Commands

### Initialize Hub
```bash
fastn-hub init
```
Creates `hub.key` and initial config. Prints the hub's ID52.

### Show Hub Info
```bash
fastn-hub info
```
Displays hub ID52 and spoke count.

### Add Spoke
```bash
fastn-hub add-spoke <spoke-id52>
```
Authorizes a spoke to connect to this hub. The alias defaults to the first
8 characters of the ID52. To change aliases, edit spokes.txt directly in
the root kosha.

### Remove Spoke
```bash
fastn-hub remove-spoke <spoke-id52>
```
Revokes spoke authorization.

### List Spokes
```bash
fastn-hub list-spokes
```
Shows all authorized spokes with their aliases.

### Run Hub Server
```bash
fastn-hub
```
Starts the hub server, listening for spoke connections.

## Configuration (config.json)

```json
{
  "hub_id52": "ABCD...XYZ",
  "created_at": "2024-12-24T15:30:45Z"
}
```

## Spokes Configuration (spokes.txt)

Stored in the root kosha at `FASTN_HOME/koshas/root/files/spokes.txt`.
Simple text file with one spoke per line:
```
# Authorized spokes (one per line)
# Format: <id52>: <alias>
ABCD1234...XYZ: my-laptop
EFGH5678...ABC: work-machine
```

The alias is provided by the spoke when it connects for the first time.
Spokes must run `fastn-spoke init <hub-id52> <alias>` to set their alias.
When you run `fastn-hub add-spoke <id52>`, the hub uses the alias from
the pending connection.

### Pending Spokes

When an unauthorized spoke connects, the hub records it as "pending".
To see pending spokes:
```bash
fastn-hub list-pending
```

This shows spoke IDs with their requested aliases, which you can then
authorize with `fastn-hub add-spoke <id52>`.

## Hub Authorization (hubs/ folder)

For hub-to-hub communication, authorization lists are stored in the
`hubs/` folder within the root kosha (`FASTN_HOME/koshas/root/files/hubs/`).

Each `.hubs` file can contain:
- `<id52>: <alias>` - authorize a hub with given alias
- `<id52>: <alias> # comment` - inline comments (` # ` required)
- `# comment` - full-line comments (space after `#` required)
- `@<filename>` - include all hubs from `<filename>.hubs`
- `@ROOT/<name>` - include from root kosha's `hubs/<name>.hubs`
- `#<alias>` - reference a single hub by its alias (no space after `#`)

### Uniqueness Constraints

**IMPORTANT:**
- Each ID52 must be defined in exactly **one** `.hubs` file
- Each alias must be **globally unique** across all files
- Duplicates will cause errors during resolution

### Example: Hub Authorization Files

```
hubs/
├── README.txt       # Auto-generated with format documentation
├── friends.hubs     # Personal friends
├── family.hubs      # Family members
└── trusted.hubs     # Combined list
```

**friends.hubs:**
```
# Close friends with their own hubs
ABCD...XYZ: alice  # my friend Alice
EFGH...ABC: bob
```

**family.hubs:**
```
IJKL...DEF: mom
MNOP...GHI: dad
```

**trusted.hubs:**
```
# Include everyone from friends and family
# They all get the alias "trusted" when accessed through this file
@friends
@family
#alice   # just Alice (reference by her alias)
```

### Include Semantics

When you include via `@filename`, all included hubs get the **includer's
filename** as their alias (not their original alias). This is useful for
grouping - e.g., including friends in a "trusted" file gives them the
"trusted" alias.

When you reference a single hub via `#alias`, only that specific hub is
included, using the alias you gave it when defining it.

### Non-Root Kosha Authorization (_hubs/)

In non-root koshas, hub authorization lists are stored in `_hubs/`:
```
my-kosha/
├── files/
│   ├── _hubs/              # Hub authorization for this kosha
│   │   └── allowed.hubs    # Who can access this kosha
│   └── data/
└── ...
```

You can reference root kosha files with `@ROOT/<name>`:
```
# allowed.hubs in a non-root kosha
@ROOT/trusted    # Include all from root's hubs/trusted.hubs
```

### ACL Integration with WASM

Hub authorization files can be paired with WASM access control modules:
- `_access.hubs` - lists hubs that can pass `_access.wasm` ACL
- `_read.hubs` - lists hubs for `_read.wasm` features
- `_write.hubs` - lists hubs for `_write.wasm` features

The naming convention is `_<name>.hubs` corresponds to `_<name>.wasm`.

## API Protocol

Spokes communicate with the hub using fastn-net's request/response protocol.
All requests include the kosha alias to target.

### Request Types

```rust
enum HubRequest {
    // File operations
    ReadFile { kosha: String, path: String },
    WriteFile { kosha: String, path: String, content: Vec<u8> },
    ListDir { kosha: String, path: String },
    GetVersions { kosha: String, path: String },
    ReadVersion { kosha: String, path: String, timestamp: DateTime<Utc> },
    Rename { kosha: String, from: String, to: String },
    Delete { kosha: String, path: String },

    // KV operations
    KvGet { kosha: String, key: String },
    KvSet { kosha: String, key: String, value: Value },
    KvDelete { kosha: String, key: String },

    // Meta
    ListKoshas,
    Ping,
}
```

### Response Types

```rust
enum HubResponse {
    // File responses
    FileContent(Vec<u8>),
    DirListing(Vec<DirEntry>),
    Versions(Vec<FileVersion>),

    // KV responses
    KvValue(Option<Value>),

    // Meta
    KoshaList(Vec<String>),
    Pong,
    Ok,
}

enum HubError {
    Unauthorized,
    KoshaNotFound(String),
    FileNotFound(String),
    InvalidPath(String),
    IoError(String),
}
```

## Authentication

When a spoke connects:
1. Hub receives the spoke's public key from the connection
2. Hub checks if the public key is in `spokes.json`
3. If authorized, requests are processed
4. If not, `HubError::Unauthorized` is returned

## Hub-to-Hub Federation

Hubs can connect to other hubs using `fastn_net::Hub::connect()`.
This enables:
- Kosha replication between hubs
- Forwarding requests to remote koshas
- Distributed storage networks

## Example Usage

```rust
use fastn_hub::Hub;

#[tokio::main]
async fn main() -> Result<()> {
    // Load or create hub
    let hub = Hub::load_or_init()?;
    println!("Hub ID: {}", hub.id52());

    // Add a spoke
    hub.add_spoke("ABCD...XYZ")?;

    // Create a kosha
    hub.create_kosha("my-data")?;

    // Run the server
    hub.serve().await?;

    Ok(())
}
```
