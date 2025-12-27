//! Integration tests for ACL (Access Control List) scenarios
//!
//! Tests cross-hub authorization using .hubs files.

use fastn_hub::{Hub, Request, HubError};
use fastn_net::SecretKey;
use std::path::PathBuf;

/// Helper to create a test hub with its own temp directory
async fn create_test_hub(name: &str, _port: u16) -> (Hub, PathBuf, String) {
    let temp_dir = std::env::temp_dir().join(format!("fastn-acl-test-{}-{}", name, std::process::id()));

    // Clean up any previous test directory
    let _ = std::fs::remove_dir_all(&temp_dir);
    std::fs::create_dir_all(&temp_dir).expect("Failed to create test directory");

    // Use init to explicitly pass the path instead of relying on env var
    let hub = Hub::init(temp_dir.clone()).await.expect("Failed to init hub");
    let id52 = hub.id52().to_string();

    (hub, temp_dir, id52)
}

/// Helper to write a .hubs file
async fn write_hubs_file(hub_dir: &PathBuf, filename: &str, content: &str) {
    let hubs_dir = hub_dir.join("koshas/root/files/hubs");
    tokio::fs::create_dir_all(&hubs_dir).await.expect("Failed to create hubs dir");
    let file_path = hubs_dir.join(filename);
    tokio::fs::write(&file_path, content).await.expect("Failed to write .hubs file");
}

/// Helper to write a test file in the root kosha
async fn write_test_file(hub_dir: &PathBuf, filename: &str, content: &str) {
    let files_dir = hub_dir.join("koshas/root/files");
    tokio::fs::create_dir_all(&files_dir).await.expect("Failed to create files dir");
    let file_path = files_dir.join(filename);
    tokio::fs::write(&file_path, content).await.expect("Failed to write test file");
}

/// Helper to add a spoke to the hub's authorized spokes list
async fn authorize_spoke(hub_dir: &PathBuf, spoke_id52: &str, alias: &str) {
    let files_dir = hub_dir.join("koshas/root/files");
    tokio::fs::create_dir_all(&files_dir).await.expect("Failed to create files dir");
    let spokes_file = files_dir.join("spokes.txt");

    let content = format!(
        "# Authorized spokes\n\
         {}: {}\n",
        spoke_id52, alias
    );
    tokio::fs::write(&spokes_file, content).await.expect("Failed to write spokes.txt");
}

#[tokio::test]
async fn test_spoke_access_own_hub() {
    // Test: A spoke should be able to read files from its own hub

    let (mut hub, hub_dir, _hub_id52) = create_test_hub("own-hub", 4000).await;

    // Create a spoke key
    let spoke_key = SecretKey::generate();
    let spoke_id52 = spoke_key.public().id52();

    // Authorize the spoke using hub.add_spoke() to update in-memory state
    hub.add_spoke(&spoke_id52).await.expect("Failed to add spoke");

    // Write a test file
    write_test_file(&hub_dir, "hello.txt", "Hello, World!").await;

    // Create a read_file request
    // The spoke's identity is verified via cryptographic signature (spoke_id52)
    let request = Request {
        target_hub: "self".to_string(),
        app: "kosha".to_string(),
        instance: "root".to_string(),
        command: "read_file".to_string(),
        payload: serde_json::json!({ "path": "hello.txt" }),
    };

    // Handle the request - sender identity derived from spoke_id52
    let result = hub.handle_request(&spoke_id52, request).await;

    // Should succeed
    assert!(result.is_ok(), "Spoke should be able to read from its own hub");

    let response = result.unwrap();
    assert!(response.payload.get("content").is_some(), "Response should contain content");

    // Decode base64 content
    let content_b64 = response.payload["content"].as_str().unwrap();
    let content = String::from_utf8(
        base64::Engine::decode(&base64::engine::general_purpose::STANDARD, content_b64).unwrap()
    ).unwrap();
    assert_eq!(content, "Hello, World!");

    // Cleanup
    let _ = std::fs::remove_dir_all(&hub_dir);
}

#[tokio::test]
async fn test_cross_hub_access_authorized() {
    // Test: Hub1 (forwarding for its spoke) should be able to read files from Hub2 when Hub1 is authorized
    // In the new design, cross-hub requests are signed by the forwarding hub, so we verify hub1's identity

    let (hub2, hub2_dir, _hub2_id52) = create_test_hub("cross-hub2", 4002).await;

    // Create Hub1's identity (we just need the ID52 for authorization)
    let hub1_key = SecretKey::generate();
    let hub1_id52 = hub1_key.public().id52();

    // Authorize Hub1 in Hub2's .hubs file
    let hubs_content = format!(
        "# Authorized hubs\n\
         {}: hub1 http://localhost:4001\n",
        hub1_id52
    );
    write_hubs_file(&hub2_dir, "known.hubs", &hubs_content).await;

    // Write a test file in Hub2
    write_test_file(&hub2_dir, "secret.txt", "Hub2 Secret Data").await;

    // Create a request that Hub1 is forwarding to Hub2
    // The sender identity (hub1_id52) is verified from the cryptographic signature
    let request = Request {
        target_hub: "self".to_string(),  // Direct request to Hub2
        app: "kosha".to_string(),
        instance: "root".to_string(),
        command: "read_file".to_string(),
        payload: serde_json::json!({ "path": "secret.txt" }),
    };

    // Handle the request at Hub2
    // The sender (hub1_id52) is verified via signature, and we check if it's in .hubs files
    let result = hub2.handle_request(&hub1_id52, request).await;

    // Should succeed because Hub1 is authorized
    assert!(result.is_ok(), "Cross-hub access should succeed when hub is authorized: {:?}", result.err());

    let response = result.unwrap();
    let content_b64 = response.payload["content"].as_str().unwrap();
    let content = String::from_utf8(
        base64::Engine::decode(&base64::engine::general_purpose::STANDARD, content_b64).unwrap()
    ).unwrap();
    assert_eq!(content, "Hub2 Secret Data");

    // Cleanup
    let _ = std::fs::remove_dir_all(&hub2_dir);
}

#[tokio::test]
async fn test_cross_hub_access_denied() {
    // Test: An unauthorized hub should be denied access
    // In the new design, the sender identity is verified from the signature

    let (hub2, hub2_dir, _hub2_id52) = create_test_hub("deny-hub2", 4004).await;

    // Create an unauthorized hub's identity
    let unauthorized_hub_key = SecretKey::generate();
    let unauthorized_hub_id52 = unauthorized_hub_key.public().id52();

    // Note: We don't add the unauthorized hub to Hub2's .hubs file

    // Write a test file in Hub2
    write_test_file(&hub2_dir, "protected.txt", "Protected Data").await;

    // Create a request from the unauthorized hub to Hub2
    // The sender identity (unauthorized_hub_id52) is verified from the signature
    let request = Request {
        target_hub: "self".to_string(),
        app: "kosha".to_string(),
        instance: "root".to_string(),
        command: "read_file".to_string(),
        payload: serde_json::json!({ "path": "protected.txt" }),
    };

    // Handle the request at Hub2
    // The unauthorized hub's identity is verified via signature
    let result = hub2.handle_request(&unauthorized_hub_id52, request).await;

    // Should fail with Unauthorized (sender not recognized as spoke or authorized hub)
    assert!(result.is_err(), "Cross-hub access should be denied when hub is not authorized");

    match result.unwrap_err() {
        HubError::Unauthorized => {
            // Expected - the sender is not in spokes.txt or .hubs files
        }
        other => panic!("Expected Unauthorized error, got: {:?}", other),
    }

    // Cleanup
    let _ = std::fs::remove_dir_all(&hub2_dir);
}

#[tokio::test]
async fn test_hub_forwarding_lookup() {
    // Test: Hub should be able to look up another hub by alias from .hubs files

    let (hub, hub_dir, _hub_id52) = create_test_hub("forwarding", 4006).await;

    // Create a remote hub's identity
    let remote_hub_key = SecretKey::generate();
    let remote_hub_id52 = remote_hub_key.public().id52();

    // Add the remote hub to our .hubs file with its URL
    let hubs_content = format!(
        "# Known hubs for forwarding\n\
         {}: remote-hub http://localhost:4007\n",
        remote_hub_id52
    );
    write_hubs_file(&hub_dir, "known.hubs", &hubs_content).await;

    // Look up the remote hub by alias
    let result = hub.lookup_hub_by_alias("remote-hub").await;

    assert!(result.is_ok(), "Hub lookup should succeed");
    let hub_info = result.unwrap();
    assert!(hub_info.is_some(), "Hub should be found");

    let info = hub_info.unwrap();
    assert_eq!(info.id52, remote_hub_id52);
    assert_eq!(info.alias, "remote-hub");
    assert_eq!(info.url.as_deref(), Some("http://localhost:4007"));

    // Cleanup
    let _ = std::fs::remove_dir_all(&hub_dir);
}

#[tokio::test]
async fn test_is_hub_authorized() {
    // Test: Hub should correctly report authorization status

    let (hub, hub_dir, _hub_id52) = create_test_hub("auth-check", 4008).await;

    // Create two hubs - one authorized, one not
    let authorized_hub_key = SecretKey::generate();
    let authorized_hub_id52 = authorized_hub_key.public().id52();

    let unauthorized_hub_key = SecretKey::generate();
    let unauthorized_hub_id52 = unauthorized_hub_key.public().id52();

    // Add only the authorized hub
    let hubs_content = format!(
        "# Authorized hubs\n\
         {}: authorized http://localhost:9999\n",
        authorized_hub_id52
    );
    write_hubs_file(&hub_dir, "known.hubs", &hubs_content).await;

    // Check authorization
    let authorized_result = hub.is_hub_authorized(&authorized_hub_id52).await;
    assert!(authorized_result.is_ok());
    assert!(authorized_result.unwrap(), "Authorized hub should be authorized");

    let unauthorized_result = hub.is_hub_authorized(&unauthorized_hub_id52).await;
    assert!(unauthorized_result.is_ok());
    assert!(!unauthorized_result.unwrap(), "Unauthorized hub should not be authorized");

    // Cleanup
    let _ = std::fs::remove_dir_all(&hub_dir);
}
