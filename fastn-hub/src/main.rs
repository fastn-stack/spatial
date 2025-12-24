//! fastn-hub CLI
//!
//! Usage:
//!   fastn-hub init     - Initialize a new hub (creates FASTN_HOME with secret key)
//!   fastn-hub          - Run the hub server (requires init first)
//!   fastn-hub id       - Show the hub's ID52

use fastn_hub::Hub;
use std::env;

#[tokio::main]
async fn main() {
    let args: Vec<String> = env::args().collect();

    let command = args.get(1).map(|s| s.as_str());

    match command {
        Some("init") => {
            match Hub::init().await {
                Ok(hub) => {
                    println!("Hub initialized successfully!");
                    println!("ID52: {}", hub.id52());
                    println!("Home: {:?}", hub.home());
                }
                Err(e) => {
                    eprintln!("Failed to initialize hub: {}", e);
                    std::process::exit(1);
                }
            }
        }
        Some("id") => {
            match Hub::load().await {
                Ok(hub) => {
                    println!("{}", hub.id52());
                }
                Err(e) => {
                    eprintln!("Failed to load hub: {}", e);
                    std::process::exit(1);
                }
            }
        }
        Some("help") | Some("-h") | Some("--help") => {
            print_help();
        }
        None => {
            // Run the hub server
            match Hub::load().await {
                Ok(hub) => {
                    println!("Starting hub server...");
                    if let Err(e) = hub.serve().await {
                        eprintln!("Hub server error: {}", e);
                        std::process::exit(1);
                    }
                }
                Err(e) => {
                    eprintln!("Failed to load hub: {}", e);
                    eprintln!("Run 'fastn-hub init' first to initialize the hub.");
                    std::process::exit(1);
                }
            }
        }
        Some(cmd) => {
            eprintln!("Unknown command: {}", cmd);
            print_help();
            std::process::exit(1);
        }
    }
}

fn print_help() {
    println!("fastn-hub - Hub server for fastn P2P network");
    println!();
    println!("Usage:");
    println!("  fastn-hub init     Initialize a new hub (creates FASTN_HOME with secret key)");
    println!("  fastn-hub          Run the hub server (requires init first)");
    println!("  fastn-hub id       Show the hub's ID52");
    println!("  fastn-hub help     Show this help message");
    println!();
    println!("Environment:");
    println!("  FASTN_HOME         Override the default home directory");
    println!("                     Default: ~/.local/share/fastn (Linux)");
    println!("                              ~/Library/Application Support/com.fastn.fastn (macOS)");
}
