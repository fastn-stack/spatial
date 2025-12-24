//! Minimal hub-spoke P2P networking over Iroh
//!
//! This crate provides a simple hub-spoke model for P2P communication:
//! - Hub (server) listens on its private key
//! - Spokes (clients) connect via hub's public ID52
//! - Hubs can also connect to other hubs for hub-to-hub communication
//!
//! # Example
//!
//! ```rust,ignore
//! use fastn_net::{Hub, Spoke, SecretKey};
//! use serde::{Serialize, Deserialize};
//!
//! #[derive(Serialize, Deserialize)]
//! struct Request { message: String }
//!
//! #[derive(Serialize, Deserialize)]
//! struct Response { echo: String }
//!
//! // Hub side
//! let hub = Hub::new(SecretKey::generate()).await?;
//! println!("Hub ID: {}", hub.id52());
//!
//! loop {
//!     let (peer, request, responder) = hub.accept::<Request>().await?;
//!     let response = Response { echo: request.message };
//!     responder.respond::<Response, String>(Ok(response)).await?;
//! }
//!
//! // Spoke side
//! let spoke = Spoke::new(SecretKey::generate(), &hub_id52).await?;
//! let result: Result<Response, String> = spoke.call(Request { message: "hello".into() }).await?;
//! ```

use serde::{de::DeserializeOwned, Serialize};
use thiserror::Error;

/// ALPN protocol identifier for fastn-net connections
const ALPN: &[u8] = b"/fastn-net/0.1";

/// Response acknowledgment message
const ACK: &[u8] = b"ack\n";

// Re-export key types from iroh for convenience
pub use iroh::{PublicKey, SecretKey};

/// Error types for fastn-net operations
#[derive(Error, Debug)]
pub enum Error {
    #[error("Failed to create endpoint: {0}")]
    EndpointCreation(String),

    #[error("Failed to accept connection: {0}")]
    AcceptConnection(String),

    #[error("Failed to connect to hub: {0}")]
    Connect(String),

    #[error("Failed to read from stream: {0}")]
    Read(String),

    #[error("Failed to write to stream: {0}")]
    Write(String),

    #[error("JSON serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("Invalid ID52: {0}")]
    InvalidId52(String),

    #[error("Protocol error: {0}")]
    Protocol(String),
}

pub type Result<T> = std::result::Result<T, Error>;

/// Convert a public key to ID52 format (52-character base32 encoding)
pub fn to_id52(key: &PublicKey) -> String {
    data_encoding::BASE32_DNSSEC.encode(key.as_bytes())
}

/// Parse an ID52 string back to a PublicKey
pub fn from_id52(id52: &str) -> Result<PublicKey> {
    let bytes = data_encoding::BASE32_DNSSEC
        .decode(id52.to_uppercase().as_bytes())
        .map_err(|e| Error::InvalidId52(e.to_string()))?;

    let bytes: [u8; 32] = bytes
        .try_into()
        .map_err(|_| Error::InvalidId52("Expected 32 bytes".into()))?;

    PublicKey::from_bytes(&bytes).map_err(|e| Error::InvalidId52(e.to_string()))
}

/// Hub (server) that accepts connections from spokes
pub struct Hub {
    endpoint: iroh::Endpoint,
}

impl Hub {
    /// Create a new hub listening on the given secret key
    pub async fn new(secret_key: SecretKey) -> Result<Self> {
        let endpoint = iroh::Endpoint::builder()
            .discovery_n0()
            .discovery_local_network()
            .alpns(vec![ALPN.to_vec()])
            .secret_key(secret_key)
            .bind()
            .await
            .map_err(|e| Error::EndpointCreation(e.to_string()))?;

        Ok(Self { endpoint })
    }

    /// Get the ID52 of this hub (give this to spokes to connect)
    pub fn id52(&self) -> String {
        let node_id = self.endpoint.node_id();
        data_encoding::BASE32_DNSSEC.encode(node_id.as_bytes())
    }

    /// Get the public key of this hub
    pub fn public_key(&self) -> PublicKey {
        let node_id = self.endpoint.node_id();
        PublicKey::from_bytes(node_id.as_bytes()).expect("valid node id")
    }

    /// Connect to another hub for hub-to-hub communication
    ///
    /// Returns a `HubPeer` that can be used to make requests to the other hub.
    pub async fn connect(&self, other_hub_id52: &str) -> Result<HubPeer> {
        let hub_key = from_id52(other_hub_id52)?;
        let hub = iroh::NodeId::from(hub_key);
        Ok(HubPeer {
            endpoint: self.endpoint.clone(),
            hub,
        })
    }

    /// Accept a request from a spoke or another hub
    ///
    /// Returns the peer's public key, the deserialized request, and a responder
    /// that must be used to send exactly one response.
    pub async fn accept<Req: DeserializeOwned>(&self) -> Result<(PublicKey, Req, Responder)> {
        // Accept incoming connection
        let incoming = self
            .endpoint
            .accept()
            .await
            .ok_or_else(|| Error::AcceptConnection("Endpoint closed".into()))?;

        let conn = incoming
            .await
            .map_err(|e| Error::AcceptConnection(e.to_string()))?;

        // Get peer's public key
        let remote_node_id = conn
            .remote_node_id()
            .map_err(|e| Error::AcceptConnection(format!("Could not get remote node ID: {}", e)))?;
        let peer =
            PublicKey::from_bytes(remote_node_id.as_bytes()).map_err(|e| Error::AcceptConnection(e.to_string()))?;

        // Accept bidirectional stream
        let (send, mut recv) = conn
            .accept_bi()
            .await
            .map_err(|e| Error::AcceptConnection(e.to_string()))?;

        // Send ACK
        let mut send = send;
        send.write_all(ACK)
            .await
            .map_err(|e| Error::Write(e.to_string()))?;

        // Read request JSON (newline-terminated)
        let request_json = read_line(&mut recv).await?;
        let request: Req = serde_json::from_str(&request_json)?;

        Ok((peer, request, Responder { send }))
    }
}

/// Handle for sending a response back to the spoke
pub struct Responder {
    send: iroh::endpoint::SendStream,
}

impl Responder {
    /// Send a response (or error) back to the spoke
    pub async fn respond<Res: Serialize, Err: Serialize>(
        mut self,
        result: std::result::Result<Res, Err>,
    ) -> Result<()> {
        // Wrap in a Result envelope for the wire format
        let envelope = match result {
            Ok(res) => ResponseEnvelope::Ok(res),
            Err(err) => ResponseEnvelope::Err(err),
        };

        let json = serde_json::to_string(&envelope)?;
        self.send
            .write_all(json.as_bytes())
            .await
            .map_err(|e| Error::Write(e.to_string()))?;
        self.send
            .write_all(b"\n")
            .await
            .map_err(|e| Error::Write(e.to_string()))?;
        self.send
            .finish()
            .map_err(|e| Error::Write(e.to_string()))?;

        Ok(())
    }
}

/// Connection to another hub for hub-to-hub communication
///
/// Created via `Hub::connect()`. Allows one hub to make requests to another hub.
pub struct HubPeer {
    endpoint: iroh::Endpoint,
    hub: iroh::NodeId,
}

impl HubPeer {
    /// Get the ID52 of the connected hub
    pub fn id52(&self) -> String {
        data_encoding::BASE32_DNSSEC.encode(self.hub.as_bytes())
    }

    /// Get the public key of the connected hub
    pub fn public_key(&self) -> PublicKey {
        PublicKey::from_bytes(self.hub.as_bytes()).expect("valid node id")
    }

    /// Make a request to the connected hub and get a response
    pub async fn call<Req: Serialize, Res: DeserializeOwned, Err: DeserializeOwned>(
        &self,
        request: Req,
    ) -> Result<std::result::Result<Res, Err>> {
        // Connect to hub
        let conn = self
            .endpoint
            .connect(self.hub, ALPN)
            .await
            .map_err(|e| Error::Connect(e.to_string()))?;

        // Open bidirectional stream
        let (mut send, mut recv) = conn
            .open_bi()
            .await
            .map_err(|e| Error::Connect(e.to_string()))?;

        // Wait for ACK
        let ack = read_line(&mut recv).await?;
        if ack.trim() != "ack" {
            return Err(Error::Protocol(format!("Expected 'ack', got '{}'", ack)));
        }

        // Send request
        let request_json = serde_json::to_string(&request)?;
        send.write_all(request_json.as_bytes())
            .await
            .map_err(|e| Error::Write(e.to_string()))?;
        send.write_all(b"\n")
            .await
            .map_err(|e| Error::Write(e.to_string()))?;

        // Read response
        let response_json = read_line(&mut recv).await?;
        let envelope: ResponseEnvelope<Res, Err> = serde_json::from_str(&response_json)?;

        Ok(envelope.into_result())
    }
}

/// Spoke (client) that connects to a hub
pub struct Spoke {
    endpoint: iroh::Endpoint,
    hub: iroh::NodeId,
}

impl Spoke {
    /// Create a new spoke that will connect to the given hub
    pub async fn new(secret_key: SecretKey, hub_id52: &str) -> Result<Self> {
        let hub_key = from_id52(hub_id52)?;
        let hub = iroh::NodeId::from(hub_key);

        let endpoint = iroh::Endpoint::builder()
            .discovery_n0()
            .discovery_local_network()
            .alpns(vec![ALPN.to_vec()])
            .secret_key(secret_key)
            .bind()
            .await
            .map_err(|e| Error::EndpointCreation(e.to_string()))?;

        Ok(Self { endpoint, hub })
    }

    /// Get the ID52 of this spoke
    pub fn id52(&self) -> String {
        let node_id = self.endpoint.node_id();
        data_encoding::BASE32_DNSSEC.encode(node_id.as_bytes())
    }

    /// Make a request to the hub and get a response
    pub async fn call<Req: Serialize, Res: DeserializeOwned, Err: DeserializeOwned>(
        &self,
        request: Req,
    ) -> Result<std::result::Result<Res, Err>> {
        // Connect to hub
        let conn = self
            .endpoint
            .connect(self.hub, ALPN)
            .await
            .map_err(|e| Error::Connect(e.to_string()))?;

        // Open bidirectional stream
        let (mut send, mut recv) = conn
            .open_bi()
            .await
            .map_err(|e| Error::Connect(e.to_string()))?;

        // Wait for ACK
        let ack = read_line(&mut recv).await?;
        if ack.trim() != "ack" {
            return Err(Error::Protocol(format!("Expected 'ack', got '{}'", ack)));
        }

        // Send request
        let request_json = serde_json::to_string(&request)?;
        send.write_all(request_json.as_bytes())
            .await
            .map_err(|e| Error::Write(e.to_string()))?;
        send.write_all(b"\n")
            .await
            .map_err(|e| Error::Write(e.to_string()))?;

        // Read response
        let response_json = read_line(&mut recv).await?;
        let envelope: ResponseEnvelope<Res, Err> = serde_json::from_str(&response_json)?;

        Ok(envelope.into_result())
    }
}

/// Wire format for responses (wraps Ok/Err)
#[derive(Serialize, serde::Deserialize)]
#[serde(tag = "status", content = "data")]
enum ResponseEnvelope<T, E> {
    Ok(T),
    Err(E),
}

impl<T, E> ResponseEnvelope<T, E> {
    fn into_result(self) -> std::result::Result<T, E> {
        match self {
            ResponseEnvelope::Ok(t) => Ok(t),
            ResponseEnvelope::Err(e) => Err(e),
        }
    }
}

/// Read a newline-terminated line from a stream
async fn read_line(recv: &mut iroh::endpoint::RecvStream) -> Result<String> {
    let mut buf = Vec::new();
    let mut byte = [0u8; 1];

    loop {
        match recv.read(&mut byte).await {
            Ok(Some(1)) => {
                if byte[0] == b'\n' {
                    break;
                }
                buf.push(byte[0]);
            }
            Ok(_) => {
                // End of stream
                break;
            }
            Err(e) => return Err(Error::Read(e.to_string())),
        }

        // Safety limit
        if buf.len() > 10 * 1024 * 1024 {
            return Err(Error::Read("Message too large".into()));
        }
    }

    String::from_utf8(buf).map_err(|e| Error::Read(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_id52_roundtrip() {
        let key = SecretKey::generate(&mut rand::thread_rng()).public();
        let id52 = to_id52(&key);
        assert_eq!(id52.len(), 52);

        let parsed = from_id52(&id52).unwrap();
        assert_eq!(key, parsed);
    }
}
