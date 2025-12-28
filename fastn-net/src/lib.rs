//! Minimal hub-spoke P2P networking with HTTP transport and signed JSON
//!
//! This crate provides a simple hub-spoke model for P2P communication:
//! - Hub (server) listens on HTTP at `/_fastn` endpoint
//! - Spokes (clients) connect via HTTP POST with signed JSON
//! - All requests are cryptographically signed, so HTTPS is optional
//!
//! # Identity
//!
//! Each node has an Ed25519 keypair. The public key is encoded as ID52
//! (52-character base32 lowercase). This ID52 uniquely identifies the node.
//!
//! # Wire Format
//!
//! Requests are POST to `/_fastn` with JSON body:
//! ```json
//! {
//!   "sender": "<id52>",
//!   "payload": { ... },
//!   "signature": "<base64 signature>"
//! }
//! ```
//!
//! The signature covers `sender + "|" + canonical_json(payload)`.
//!
//! # Example
//!
//! ```rust,ignore
//! use fastn_net::{SecretKey, SignedRequest, verify_request};
//! use serde::{Serialize, Deserialize};
//!
//! #[derive(Serialize, Deserialize)]
//! struct MyRequest { message: String }
//!
//! // Create and sign a request
//! let key = SecretKey::generate();
//! let request = MyRequest { message: "hello".into() };
//! let signed = SignedRequest::new(&key, &request)?;
//!
//! // Verify and extract
//! let (sender_id52, payload): (String, MyRequest) = signed.verify()?;
//! ```

use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use thiserror::Error;

/// HTTP endpoint path for fastn protocol
pub const ENDPOINT: &str = "/_fastn";

/// Error types for fastn-net operations
#[derive(Error, Debug)]
pub enum Error {
    #[error("JSON serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("Invalid ID52: {0}")]
    InvalidId52(String),

    #[error("Invalid signature")]
    InvalidSignature,

    #[error("Signature verification failed")]
    VerificationFailed,

    #[error("Base64 decode error: {0}")]
    Base64Decode(String),

    #[cfg(any(feature = "client", target_arch = "wasm32"))]
    #[error("HTTP request failed: {0}")]
    HttpRequest(String),

    #[cfg(feature = "server")]
    #[error("Server error: {0}")]
    Server(String),
}

pub type Result<T> = std::result::Result<T, Error>;

/// Secret key for signing (Ed25519)
#[derive(Clone)]
pub struct SecretKey(SigningKey);

impl SecretKey {
    /// Generate a new random secret key
    pub fn generate() -> Self {
        let mut rng = rand::thread_rng();
        Self(SigningKey::generate(&mut rng))
    }

    /// Create from raw bytes
    pub fn from_bytes(bytes: &[u8; 32]) -> Self {
        Self(SigningKey::from_bytes(bytes))
    }

    /// Get raw bytes
    pub fn to_bytes(&self) -> [u8; 32] {
        self.0.to_bytes()
    }

    /// Get the public key
    pub fn public(&self) -> PublicKey {
        PublicKey(self.0.verifying_key())
    }

    /// Get the ID52 of this key
    pub fn id52(&self) -> String {
        self.public().id52()
    }

    /// Sign a message
    pub fn sign(&self, message: &[u8]) -> Vec<u8> {
        self.0.sign(message).to_bytes().to_vec()
    }
}

/// Public key for verification (Ed25519)
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PublicKey(VerifyingKey);

impl PublicKey {
    /// Create from raw bytes
    pub fn from_bytes(bytes: &[u8; 32]) -> Result<Self> {
        VerifyingKey::from_bytes(bytes)
            .map(Self)
            .map_err(|e| Error::InvalidId52(e.to_string()))
    }

    /// Get raw bytes
    pub fn to_bytes(&self) -> [u8; 32] {
        self.0.to_bytes()
    }

    /// Get the ID52 (52-character base32 lowercase)
    pub fn id52(&self) -> String {
        to_id52(self)
    }

    /// Verify a signature
    pub fn verify(&self, message: &[u8], signature: &[u8]) -> Result<()> {
        let sig_bytes: [u8; 64] = signature
            .try_into()
            .map_err(|_| Error::InvalidSignature)?;
        let sig = Signature::from_bytes(&sig_bytes);
        self.0.verify(message, &sig).map_err(|_| Error::VerificationFailed)
    }
}

/// Convert a public key to ID52 format (52-character base32 lowercase)
pub fn to_id52(key: &PublicKey) -> String {
    data_encoding::BASE32_DNSSEC.encode(&key.to_bytes())
}

/// Parse an ID52 string back to a PublicKey
pub fn from_id52(id52: &str) -> Result<PublicKey> {
    let bytes = data_encoding::BASE32_DNSSEC
        .decode(id52.to_uppercase().as_bytes())
        .map_err(|e| Error::InvalidId52(e.to_string()))?;

    let bytes: [u8; 32] = bytes
        .try_into()
        .map_err(|_| Error::InvalidId52("Expected 32 bytes".into()))?;

    PublicKey::from_bytes(&bytes)
}

/// A signed request envelope
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignedRequest {
    /// Sender's ID52
    pub sender: String,
    /// The payload (as JSON value for flexibility)
    pub payload: serde_json::Value,
    /// Base64-encoded signature
    pub signature: String,
}

impl SignedRequest {
    /// Create a new signed request
    pub fn new<T: Serialize>(secret_key: &SecretKey, payload: &T) -> Result<Self> {
        let sender = secret_key.id52();
        let payload_json = serde_json::to_value(payload)?;

        // Create message to sign: sender|payload_json
        let message = format!("{}|{}", sender, serde_json::to_string(&payload_json)?);
        let signature = secret_key.sign(message.as_bytes());
        let signature_b64 = data_encoding::BASE64.encode(&signature);

        Ok(Self {
            sender,
            payload: payload_json,
            signature: signature_b64,
        })
    }

    /// Verify the signature and extract the payload
    pub fn verify<T: DeserializeOwned>(&self) -> Result<(String, T)> {
        // Decode sender's public key
        let public_key = from_id52(&self.sender)?;

        // Reconstruct the signed message
        let message = format!("{}|{}", self.sender, serde_json::to_string(&self.payload)?);

        // Decode and verify signature
        let signature = data_encoding::BASE64
            .decode(self.signature.as_bytes())
            .map_err(|e| Error::Base64Decode(e.to_string()))?;

        public_key.verify(message.as_bytes(), &signature)?;

        // Deserialize payload
        let payload: T = serde_json::from_value(self.payload.clone())?;

        Ok((self.sender.clone(), payload))
    }

    /// Get the sender's ID52 without verifying
    pub fn sender_id52(&self) -> &str {
        &self.sender
    }
}

/// A signed response envelope
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignedResponse {
    /// Responder's ID52
    pub responder: String,
    /// The payload (Ok or Err)
    pub payload: serde_json::Value,
    /// Base64-encoded signature
    pub signature: String,
}

impl SignedResponse {
    /// Create a new signed response
    pub fn new<T: Serialize>(secret_key: &SecretKey, payload: &T) -> Result<Self> {
        let responder = secret_key.id52();
        let payload_json = serde_json::to_value(payload)?;

        let message = format!("{}|{}", responder, serde_json::to_string(&payload_json)?);
        let signature = secret_key.sign(message.as_bytes());
        let signature_b64 = data_encoding::BASE64.encode(&signature);

        Ok(Self {
            responder,
            payload: payload_json,
            signature: signature_b64,
        })
    }

    /// Verify the signature and extract the payload
    pub fn verify<T: DeserializeOwned>(&self) -> Result<(String, T)> {
        let public_key = from_id52(&self.responder)?;
        let message = format!("{}|{}", self.responder, serde_json::to_string(&self.payload)?);

        let signature = data_encoding::BASE64
            .decode(self.signature.as_bytes())
            .map_err(|e| Error::Base64Decode(e.to_string()))?;

        public_key.verify(message.as_bytes(), &signature)?;

        let payload: T = serde_json::from_value(self.payload.clone())?;
        Ok((self.responder.clone(), payload))
    }

    /// Verify that the response came from a specific hub
    pub fn verify_from<T: DeserializeOwned>(&self, expected_id52: &str) -> Result<T> {
        if self.responder != expected_id52 {
            return Err(Error::VerificationFailed);
        }
        let (_, payload) = self.verify()?;
        Ok(payload)
    }
}

/// Response envelope for Ok/Err results
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "status", content = "data")]
pub enum ResponseEnvelope<T, E> {
    Ok(T),
    Err(E),
}

impl<T, E> ResponseEnvelope<T, E> {
    pub fn into_result(self) -> std::result::Result<T, E> {
        match self {
            ResponseEnvelope::Ok(t) => Ok(t),
            ResponseEnvelope::Err(e) => Err(e),
        }
    }
}

// ============================================================================
// Hub Protocol Types (used by both hub and spoke)
// ============================================================================

/// Request envelope from spokes to hub
/// Hub routes based on (app, instance) and does ACL check before forwarding
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HubRequest {
    /// Target hub alias: "self" for local hub, or alias of a remote hub
    /// If not specified, defaults to "self"
    #[serde(default = "default_target_hub")]
    pub target_hub: String,
    /// Application type (e.g., "kosha", "chat", "sync")
    pub app: String,
    /// Application instance (e.g., "my-kosha", "work-chat")
    pub instance: String,
    /// Application-specific command name
    pub command: String,
    /// Application-specific payload (JSON)
    pub payload: serde_json::Value,
}

fn default_target_hub() -> String {
    "self".to_string()
}

/// Response envelope from hub to spokes
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HubResponse {
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
// HTTP Client (Spoke side)
// ============================================================================

#[cfg(feature = "client")]
pub mod client {
    use super::*;

    /// HTTP client for making signed requests to a hub
    pub struct Client {
        secret_key: SecretKey,
        hub_id52: String,
        hub_url: String,
        http: reqwest::Client,
    }

    impl Client {
        /// Create a new client
        pub fn new(secret_key: SecretKey, hub_id52: String, hub_url: String) -> Self {
            Self {
                secret_key,
                hub_id52,
                hub_url: hub_url.trim_end_matches('/').to_string(),
                http: reqwest::Client::new(),
            }
        }

        /// Get our ID52
        pub fn id52(&self) -> String {
            self.secret_key.id52()
        }

        /// Get the hub's ID52
        pub fn hub_id52(&self) -> &str {
            &self.hub_id52
        }

        /// Make a signed request and get a verified response
        pub async fn call<Req, Res, Err>(
            &self,
            request: &Req,
        ) -> Result<std::result::Result<Res, Err>>
        where
            Req: Serialize,
            Res: DeserializeOwned,
            Err: DeserializeOwned,
        {
            // Sign the request
            let signed_req = SignedRequest::new(&self.secret_key, request)?;

            // Send HTTP POST
            let url = format!("{}{}", self.hub_url, ENDPOINT);
            let response = self
                .http
                .post(&url)
                .json(&signed_req)
                .send()
                .await
                .map_err(|e| Error::HttpRequest(e.to_string()))?;

            if !response.status().is_success() {
                return Err(Error::HttpRequest(format!(
                    "HTTP {}: {}",
                    response.status(),
                    response.text().await.unwrap_or_default()
                )));
            }

            // Parse and verify response
            let signed_res: SignedResponse = response
                .json()
                .await
                .map_err(|e| Error::HttpRequest(e.to_string()))?;

            // Verify response came from the expected hub
            let envelope: ResponseEnvelope<Res, Err> = signed_res.verify_from(&self.hub_id52)?;

            Ok(envelope.into_result())
        }
    }
}

// ============================================================================
// HTTP Client for WASM (Spoke side)
// ============================================================================

#[cfg(target_arch = "wasm32")]
pub mod web_client {
    use super::*;

    /// HTTP client for making signed requests to a hub (WASM version using gloo-net)
    pub struct Client {
        secret_key: SecretKey,
        hub_id52: String,
        hub_url: String,
    }

    impl Client {
        /// Create a new client
        pub fn new(secret_key: SecretKey, hub_id52: String, hub_url: String) -> Self {
            Self {
                secret_key,
                hub_id52,
                hub_url: hub_url.trim_end_matches('/').to_string(),
            }
        }

        /// Get our ID52
        pub fn id52(&self) -> String {
            self.secret_key.id52()
        }

        /// Get the hub's ID52
        pub fn hub_id52(&self) -> &str {
            &self.hub_id52
        }

        /// Make a signed request and get a verified response
        pub async fn call<Req, Res, Err>(
            &self,
            request: &Req,
        ) -> Result<std::result::Result<Res, Err>>
        where
            Req: Serialize,
            Res: DeserializeOwned,
            Err: DeserializeOwned,
        {
            use gloo_net::http::Request;

            // Sign the request
            let signed_req = SignedRequest::new(&self.secret_key, request)?;

            // Send HTTP POST
            let url = format!("{}{}", self.hub_url, ENDPOINT);
            let response = Request::post(&url)
                .header("Content-Type", "application/json")
                .body(serde_json::to_string(&signed_req)?)
                .map_err(|e| Error::HttpRequest(e.to_string()))?
                .send()
                .await
                .map_err(|e| Error::HttpRequest(e.to_string()))?;

            if !response.ok() {
                let status = response.status();
                let text = response.text().await.unwrap_or_default();
                return Err(Error::HttpRequest(format!("HTTP {}: {}", status, text)));
            }

            // Parse and verify response
            let text = response
                .text()
                .await
                .map_err(|e| Error::HttpRequest(e.to_string()))?;
            let signed_res: SignedResponse = serde_json::from_str(&text)?;

            // Verify response came from the expected hub
            let envelope: ResponseEnvelope<Res, Err> = signed_res.verify_from(&self.hub_id52)?;

            Ok(envelope.into_result())
        }
    }
}

// ============================================================================
// HTTP Server (Hub side)
// ============================================================================

#[cfg(feature = "server")]
pub mod server {
    use super::*;
    use axum::{
        extract::State,
        http::StatusCode,
        response::IntoResponse,
        routing::post,
        Json, Router,
    };
    use std::future::Future;
    use std::sync::Arc;

    /// Handler function type
    pub type HandlerFn<Req, Res, Err> = Arc<
        dyn Fn(String, Req) -> std::pin::Pin<Box<dyn Future<Output = std::result::Result<Res, Err>> + Send>>
            + Send
            + Sync,
    >;

    /// Server state
    pub struct ServerState<Req, Res, Err> {
        pub secret_key: SecretKey,
        pub handler: HandlerFn<Req, Res, Err>,
    }

    /// Create an axum router for the fastn endpoint
    pub fn router<Req, Res, Err>(
        secret_key: SecretKey,
        handler: impl Fn(String, Req) -> std::pin::Pin<Box<dyn Future<Output = std::result::Result<Res, Err>> + Send>>
            + Send
            + Sync
            + 'static,
    ) -> Router
    where
        Req: DeserializeOwned + Send + 'static,
        Res: Serialize + Send + 'static,
        Err: Serialize + Send + 'static,
    {
        let state = Arc::new(ServerState {
            secret_key,
            handler: Arc::new(handler),
        });

        Router::new()
            .route(ENDPOINT, post(handle_request::<Req, Res, Err>))
            .with_state(state)
    }

    async fn handle_request<Req, Res, Err>(
        State(state): State<Arc<ServerState<Req, Res, Err>>>,
        Json(signed_req): Json<SignedRequest>,
    ) -> impl IntoResponse
    where
        Req: DeserializeOwned + Send,
        Res: Serialize + Send,
        Err: Serialize + Send,
    {
        // Verify and extract the request
        let (sender_id52, request): (String, Req) = match signed_req.verify() {
            Ok(r) => r,
            Err(e) => {
                tracing::warn!("Request verification failed: {}", e);
                return (
                    StatusCode::BAD_REQUEST,
                    Json(serde_json::json!({"error": e.to_string()})),
                );
            }
        };

        // Call the handler
        let result = (state.handler)(sender_id52, request).await;

        // Wrap in envelope and sign response
        let envelope = match result {
            Ok(res) => ResponseEnvelope::Ok(res),
            Err(err) => ResponseEnvelope::Err(err),
        };

        let signed_res = match SignedResponse::new(&state.secret_key, &envelope) {
            Ok(r) => r,
            Err(e) => {
                tracing::error!("Failed to sign response: {}", e);
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(serde_json::json!({"error": "Failed to sign response"})),
                );
            }
        };

        (StatusCode::OK, Json(serde_json::to_value(signed_res).unwrap()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_id52_roundtrip() {
        let key = SecretKey::generate();
        let public = key.public();
        let id52 = public.id52();

        assert_eq!(id52.len(), 52);

        let parsed = from_id52(&id52).unwrap();
        assert_eq!(public, parsed);
    }

    #[test]
    fn test_signed_request_roundtrip() {
        #[derive(Serialize, Deserialize, Debug, PartialEq)]
        struct TestPayload {
            message: String,
            count: u32,
        }

        let key = SecretKey::generate();
        let payload = TestPayload {
            message: "Hello".to_string(),
            count: 42,
        };

        let signed = SignedRequest::new(&key, &payload).unwrap();
        let (sender, extracted): (String, TestPayload) = signed.verify().unwrap();

        assert_eq!(sender, key.id52());
        assert_eq!(extracted, payload);
    }

    #[test]
    fn test_signature_tampering_detected() {
        #[derive(Serialize, Deserialize)]
        struct TestPayload {
            message: String,
        }

        let key = SecretKey::generate();
        let payload = TestPayload {
            message: "Hello".to_string(),
        };

        let mut signed = SignedRequest::new(&key, &payload).unwrap();

        // Tamper with the payload
        signed.payload = serde_json::json!({"message": "Tampered"});

        let result: Result<(String, TestPayload)> = signed.verify();
        assert!(result.is_err());
    }

    #[test]
    fn test_wrong_sender_detected() {
        #[derive(Serialize, Deserialize)]
        struct TestPayload {
            message: String,
        }

        let key1 = SecretKey::generate();
        let key2 = SecretKey::generate();
        let payload = TestPayload {
            message: "Hello".to_string(),
        };

        let mut signed = SignedRequest::new(&key1, &payload).unwrap();

        // Claim to be someone else
        signed.sender = key2.id52();

        let result: Result<(String, TestPayload)> = signed.verify();
        assert!(result.is_err());
    }

    #[test]
    fn test_signed_response_roundtrip() {
        #[derive(Serialize, Deserialize, Debug, PartialEq)]
        struct TestResponse {
            result: String,
        }

        let key = SecretKey::generate();
        let envelope: ResponseEnvelope<TestResponse, String> = ResponseEnvelope::Ok(TestResponse {
            result: "success".to_string(),
        });

        let signed = SignedResponse::new(&key, &envelope).unwrap();
        let extracted: ResponseEnvelope<TestResponse, String> = signed.verify_from(&key.id52()).unwrap();

        match extracted {
            ResponseEnvelope::Ok(res) => assert_eq!(res.result, "success"),
            ResponseEnvelope::Err(_) => panic!("Expected Ok"),
        }
    }
}
