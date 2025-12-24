# fastn-spoke

Spoke client for the fastn P2P network. Connects to hubs and accesses koshas.

## Overview

A **Spoke** is a P2P client that:
- Connects to an authorized hub
- Sends a human-readable alias on connection for identification
- Reads and writes files through koshas
- Accesses key-value stores
- Retries connection until hub accepts

## Quick Start

```bash
# 1. Get the hub's ID52 from the hub admin
# 2. Initialize your spoke with the hub ID and an alias
fastn-spoke init <hub-id52> my-laptop

# 3. Share your spoke ID52 with the hub admin
# 4. Hub admin runs: fastn-hub add-spoke <your-id52> [alias]

# 5. Connect to the hub (will retry until accepted)
fastn-spoke
```

## SPOKE_HOME Directory

The spoke stores its identity and configuration in `SPOKE_HOME`:
- Environment variable: `SPOKE_HOME`
- Default: `~/.fastn-spoke` (or platform-specific config directory)

```
$SPOKE_HOME/
├── spoke.key         # Spoke's secret key (Ed25519)
├── config.json       # Spoke configuration (includes hub_id52 and alias)
└── hubs.json         # Known hubs (for future multi-hub support)
```

## CLI Commands

### Initialize Spoke
```bash
fastn-spoke init <hub-id52> <alias>
```
Creates `spoke.key` and config. The alias is a human-readable name for this
spoke (e.g., 'laptop', 'phone', 'work-pc'). Prints the spoke's ID52 to share
with the hub admin.

### Show Spoke Info
```bash
fastn-spoke info
```
Displays spoke ID52, alias, and hub ID52.

### Run Spoke
```bash
fastn-spoke
```
Connects to the configured hub. Will retry every 5 seconds until the hub
accepts the connection.

### Show Spoke ID
```bash
fastn-spoke id
```
Prints just the spoke's ID52 (useful for scripting).

## Configuration (config.json)

```json
{
  "spoke_id52": "ABCD...XYZ",
  "hub_id52": "EFGH...ABC",
  "alias": "my-laptop",
  "created_at": "2024-12-24T15:30:45Z"
}
```

## Library API

```rust
use fastn_spoke::Spoke;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Load existing spoke
    let spoke = Spoke::load().await?;
    println!("Spoke ID: {}", spoke.id52());
    println!("Alias: {}", spoke.alias());

    // Connect to the configured hub
    let conn = spoke.connect().await?;

    // Read a file from a kosha
    let content = conn.read_file("my-kosha", "path/to/file.txt").await?;

    // Write a file
    conn.write_file("my-kosha", "path/to/file.txt", "base64content", None).await?;

    // KV operations
    conn.kv_set("my-kosha", "my-key", serde_json::json!({"foo": "bar"})).await?;
    let value = conn.kv_get("my-kosha", "my-key").await?;

    Ok(())
}
```

## Authentication Flow

1. Spoke initializes with hub ID52 and a human-readable alias
2. Spoke shares its ID52 with hub administrator
3. Hub admin runs `fastn-hub add-spoke <spoke-id52> [alias]`
4. Spoke connects and can now make requests

## Connection Behavior

When running `fastn-spoke`:
1. Spoke attempts to connect to the configured hub
2. If hub rejects (spoke not authorized), retry every 5 seconds
3. Once accepted, spoke prints "ONLINE - Connected to hub!"
4. Maintains connection and can send requests

## Environment Variables

- `SPOKE_HOME` - Override the default spoke data directory
