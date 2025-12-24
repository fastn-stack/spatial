# fastn-hub

Hub server for the fastn P2P network. Manages koshas (versioned storage) and authenticated spokes.

## Overview

A **Hub** is a P2P server that:
- Listens for connections from authenticated spokes
- Manages one or more koshas (storage units)
- Can connect to other hubs for federation
- Exposes APIs for file and key-value operations

## FASTN_HOME Directory

The hub stores its configuration and data in `FASTN_HOME`:
- Environment variable: `FASTN_HOME`
- Default: `~/.fastn` (or platform-specific config directory)

```
$FASTN_HOME/
├── hub.key           # Hub's secret key (Ed25519)
├── config.json       # Hub configuration
├── spokes.json       # Authorized spoke public keys
└── koshas/           # Kosha storage
    ├── my-kosha/
    │   ├── src/
    │   ├── history/
    │   └── kv/
    └── another-kosha/
        └── ...
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
Displays hub ID52 and configuration.

### Add Spoke
```bash
fastn-hub add-spoke <spoke-id52>
```
Authorizes a spoke to connect to this hub.

### Remove Spoke
```bash
fastn-hub remove-spoke <spoke-id52>
```
Revokes spoke authorization.

### List Spokes
```bash
fastn-hub list-spokes
```
Shows all authorized spokes.

### Create Kosha
```bash
fastn-hub kosha-create <alias>
```
Creates a new kosha with the given alias.

### List Koshas
```bash
fastn-hub kosha-list
```
Lists all koshas on this hub.

### Run Hub Server
```bash
fastn-hub serve
```
Starts the hub server, listening for spoke connections.

## Configuration (config.json)

```json
{
  "hub_id52": "...",
  "created_at": "2024-12-24T15:30:45Z",
  "koshas": ["my-kosha", "another-kosha"]
}
```

## Spokes Configuration (spokes.json)

```json
{
  "authorized": [
    {
      "id52": "...",
      "name": "Alice's Laptop",
      "added_at": "2024-12-24T15:30:45Z"
    }
  ]
}
```

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
