//! Kosha subcommand handlers
//!
//! Usage: fastn-spoke kosha <operation> <hub> <kosha> [args...]
//!
//! Operations:
//!   read-file <hub> <kosha> <path>                  - Read a file
//!   write-file <hub> <kosha> <path> <local-file>    - Write a file
//!   list-dir <hub> <kosha> <path>                   - List directory contents
//!   ... more to be implemented
//!
//! Hub aliases:
//!   self     - Access your own hub directly (no ACL checks)
//!   <alias>  - Access a remote hub via hub-to-hub forwarding (ACL applies)

use fastn_spoke::Spoke;
use std::io::Write;
use std::path::Path;

/// Run the kosha subcommand
pub async fn run(args: &[String], home: &Path) {
    let op = args.first().map(|s| s.as_str());

    match op {
        Some("read-file") => read_file(&args[1..], home).await,
        Some("write-file") => write_file(&args[1..], home).await,
        Some("list-dir") | Some("get-versions") | Some("read-version")
        | Some("rename") | Some("delete") | Some("kv-get") | Some("kv-set") | Some("kv-delete") => {
            eprintln!("Not implemented yet: {}", op.unwrap());
            std::process::exit(1);
        }
        Some("help") | Some("-h") | Some("--help") => print_help(),
        Some(cmd) => {
            eprintln!("Unknown kosha operation: {}", cmd);
            print_help();
            std::process::exit(1);
        }
        None => {
            eprintln!("Missing kosha operation");
            print_help();
            std::process::exit(1);
        }
    }
}

fn print_help() {
    println!("fastn-spoke kosha - Kosha operations");
    println!();
    println!("Usage: fastn-spoke kosha <operation> <hub> <kosha> [args...]");
    println!();
    println!("Operations:");
    println!("  read-file <hub> <kosha> <path>                Read a file");
    println!("  write-file <hub> <kosha> <path> <local-file>  Write a file from local path");
    println!("  list-dir <hub> <kosha> <path>                 List directory contents");
    println!("  get-versions <hub> <kosha> <path>             Get file version history");
    println!("  read-version <hub> <kosha> <path> <timestamp> Read a specific version");
    println!("  rename <hub> <kosha> <from> <to>              Rename a file");
    println!("  delete <hub> <kosha> <path>                   Delete a file");
    println!("  kv-get <hub> <kosha> <key>                    Get a key-value");
    println!("  kv-set <hub> <kosha> <key> <value>            Set a key-value");
    println!("  kv-delete <hub> <kosha> <key>                 Delete a key-value");
    println!();
    println!("Hub aliases:");
    println!("  self      Access your own hub directly (no ACL checks)");
    println!("  <alias>   Access a remote hub via hub-to-hub forwarding");
    println!();
    println!("Examples:");
    println!("  fastn-spoke kosha read-file self root spokes.txt");
    println!("  fastn-spoke kosha write-file self my-kosha docs/note.txt ./local.txt");
    println!("  fastn-spoke kosha list-dir self root /");
}

/// Read a file from a kosha
/// Usage: read-file <hub> <kosha> <path>
async fn read_file(args: &[String], home: &Path) {
    if args.len() < 3 {
        eprintln!("Usage: fastn-spoke kosha read-file <hub> <kosha> <path>");
        eprintln!();
        eprintln!("Arguments:");
        eprintln!("  hub     Hub alias ('self' for local hub, or remote hub alias)");
        eprintln!("  kosha   Kosha name (e.g., 'root', 'my-data')");
        eprintln!("  path    File path within the kosha");
        eprintln!();
        eprintln!("Example:");
        eprintln!("  fastn-spoke kosha read-file self root spokes.txt");
        std::process::exit(1);
    }

    let hub = &args[0];
    let kosha = &args[1];
    let path = &args[2];

    // Load the spoke
    let spoke = match Spoke::load(home).await {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Failed to load spoke: {}", e);
            eprintln!("Run 'fastn-spoke init <hub-id52> <alias>' first.");
            std::process::exit(1);
        }
    };

    // Create connection (HTTP client)
    let conn = spoke.connect();

    eprintln!("Reading file: {}/{}/{}", hub, kosha, path);

    // Read the file
    match conn.read_file(hub, kosha, path).await {
        Ok(response) => {
            // Response should be { "content": "<base64>" }
            if let Some(content) = response.get("content").and_then(|v| v.as_str()) {
                // Decode base64 and print
                match base64::Engine::decode(&base64::prelude::BASE64_STANDARD, content) {
                    Ok(bytes) => {
                        // Try to print as UTF-8, otherwise print as hex
                        match String::from_utf8(bytes.clone()) {
                            Ok(text) => {
                                // Handle broken pipe gracefully (e.g., when piped to head)
                                if let Err(e) = std::io::stdout().write_all(text.as_bytes()) {
                                    if e.kind() != std::io::ErrorKind::BrokenPipe {
                                        eprintln!("Failed to write output: {}", e);
                                        std::process::exit(1);
                                    }
                                } else {
                                    let _ = std::io::stdout().write_all(b"\n");
                                }
                            }
                            Err(_) => {
                                eprintln!("(binary file, {} bytes)", bytes.len());
                                for byte in &bytes {
                                    print!("{:02x}", byte);
                                }
                                println!();
                            }
                        }
                    }
                    Err(e) => {
                        eprintln!("Failed to decode base64 content: {}", e);
                        std::process::exit(1);
                    }
                }
            } else {
                eprintln!("Unexpected response format: {:?}", response);
                std::process::exit(1);
            }
        }
        Err(e) => {
            eprintln!("Failed to read file: {}", e);
            std::process::exit(1);
        }
    }
}

/// Write a file to a kosha
/// Usage: write-file <hub> <kosha> <path> <local-file>
async fn write_file(args: &[String], home: &Path) {
    if args.len() < 4 {
        eprintln!("Usage: fastn-spoke kosha write-file <hub> <kosha> <path> <local-file>");
        eprintln!();
        eprintln!("Arguments:");
        eprintln!("  hub         Hub alias ('self' for local hub, or remote hub alias)");
        eprintln!("  kosha       Kosha name (e.g., 'root', 'my-data')");
        eprintln!("  path        Destination file path within the kosha");
        eprintln!("  local-file  Path to local file to upload");
        eprintln!();
        eprintln!("Example:");
        eprintln!("  fastn-spoke kosha write-file self my-kosha docs/note.txt ./local.txt");
        std::process::exit(1);
    }

    let hub = &args[0];
    let kosha = &args[1];
    let path = &args[2];
    let local_file = &args[3];

    // Read local file
    let content = match std::fs::read(local_file) {
        Ok(bytes) => bytes,
        Err(e) => {
            eprintln!("Failed to read local file '{}': {}", local_file, e);
            std::process::exit(1);
        }
    };

    // Base64 encode the content
    let content_base64 = base64::Engine::encode(&base64::prelude::BASE64_STANDARD, &content);

    // Load the spoke
    let spoke = match Spoke::load(home).await {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Failed to load spoke: {}", e);
            eprintln!("Run 'fastn-spoke init <hub-id52> <alias>' first.");
            std::process::exit(1);
        }
    };

    // Create connection (HTTP client)
    let conn = spoke.connect();

    eprintln!("Writing file: {}/{}/{} ({} bytes)", hub, kosha, path, content.len());

    // Write the file (no base_version for new files)
    match conn.write_file(hub, kosha, path, &content_base64, None).await {
        Ok(_) => {
            eprintln!("File written successfully");
        }
        Err(e) => {
            eprintln!("Failed to write file: {}", e);
            std::process::exit(1);
        }
    }
}
