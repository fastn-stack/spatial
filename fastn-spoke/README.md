# fastn-spoke

Spoke client for the fastn P2P network. Connects to hubs and accesses koshas.

## Overview

A **Spoke** is a P2P client that:
- Connects to one or more authorized hubs
- Reads and writes files through koshas
- Accesses key-value stores
- Maintains local cache of frequently accessed data (future)

## SPOKE_HOME Directory

The spoke stores its identity and configuration in `SPOKE_HOME`:
- Environment variable: `SPOKE_HOME`
- Default: `~/.fastn-spoke` (or platform-specific config directory)

```
$SPOKE_HOME/
├── spoke.key         # Spoke's secret key (Ed25519)
├── config.json       # Spoke configuration
└── hubs.json         # Known hubs and their aliases
```

## CLI Commands

### Initialize Spoke
```bash
fastn-spoke init
```
Creates `spoke.key` and initial config. Prints the spoke's ID52 (to share with hub admins).

### Show Spoke Info
```bash
fastn-spoke info
```
Displays spoke ID52 and configuration.

### Add Hub
```bash
fastn-spoke add-hub <hub-id52> [--alias <name>]
```
Adds a hub to the known hubs list.

### Remove Hub
```bash
fastn-spoke remove-hub <hub-id52-or-alias>
```
Removes a hub from the known hubs list.

### List Hubs
```bash
fastn-spoke list-hubs
```
Shows all known hubs.

### File Operations
```bash
# List files in a kosha
fastn-spoke ls <hub>/<kosha>/<path>

# Read a file
fastn-spoke cat <hub>/<kosha>/<path>

# Write a file
fastn-spoke write <hub>/<kosha>/<path> < input.txt

# Get file versions
fastn-spoke versions <hub>/<kosha>/<path>

# Read specific version
fastn-spoke cat <hub>/<kosha>/<path> --version <timestamp>
```

### KV Operations
```bash
# Get a value
fastn-spoke kv-get <hub>/<kosha>/<key>

# Set a value
fastn-spoke kv-set <hub>/<kosha>/<key> '{"json": "value"}'

# Delete a key
fastn-spoke kv-delete <hub>/<kosha>/<key>
```

## Configuration (config.json)

```json
{
  "spoke_id52": "...",
  "created_at": "2024-12-24T15:30:45Z"
}
```

## Hubs Configuration (hubs.json)

```json
{
  "hubs": [
    {
      "id52": "...",
      "alias": "work-hub",
      "added_at": "2024-12-24T15:30:45Z"
    }
  ]
}
```

## Library API

```rust
use fastn_spoke::Spoke;

#[tokio::main]
async fn main() -> Result<()> {
    // Load or create spoke
    let spoke = Spoke::load_or_init().await?;
    println!("Spoke ID: {}", spoke.id52());

    // Connect to a hub
    let hub = spoke.connect("HUB_ID52_HERE").await?;

    // List koshas on the hub
    let koshas = hub.list_koshas().await?;

    // Read a file
    let content = hub.read_file("my-kosha", "path/to/file.txt").await?;

    // Write a file
    hub.write_file("my-kosha", "path/to/file.txt", b"content").await?;

    // Get file versions
    let versions = hub.get_versions("my-kosha", "path/to/file.txt").await?;

    // KV operations
    hub.kv_set("my-kosha", "my-key", json!({"foo": "bar"})).await?;
    let value = hub.kv_get("my-kosha", "my-key").await?;

    Ok(())
}
```

## Authentication Flow

1. Spoke initializes and generates Ed25519 keypair
2. Spoke shares its ID52 with hub administrator
3. Hub admin runs `fastn-hub add-spoke <spoke-id52>`
4. Spoke can now connect and make requests

## Connection to Hub

When connecting to a hub:
1. Spoke uses fastn-net to establish encrypted connection
2. Hub receives spoke's public key from the connection
3. Hub verifies spoke is authorized
4. If authorized, hub processes requests

## Error Handling

```rust
enum SpokeError {
    NotInitialized,
    HubNotFound(String),
    ConnectionFailed(String),
    Unauthorized,
    KoshaNotFound(String),
    FileNotFound(String),
    IoError(String),
}
```

## Path Syntax

The spoke CLI uses a unified path syntax:
```
<hub-alias>/<kosha-alias>/<file-path>
```

Examples:
- `work-hub/documents/report.txt`
- `home/photos/2024/vacation.jpg`
- `backup/config/settings.json`
