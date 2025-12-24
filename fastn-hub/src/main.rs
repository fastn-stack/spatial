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
        Some("info") => {
            match Hub::load().await {
                Ok(hub) => {
                    println!("Hub ID52: {}", hub.id52());
                    println!("Home:     {:?}", hub.home());
                    println!("Spokes:   {}", hub.list_spokes().len());
                }
                Err(e) => {
                    eprintln!("Failed to load hub: {}", e);
                    std::process::exit(1);
                }
            }
        }
        Some("add-spoke") => {
            let id52 = match args.get(2) {
                Some(id) => id,
                None => {
                    eprintln!("Usage: fastn-hub add-spoke <spoke-id52>");
                    eprintln!();
                    eprintln!("The spoke-id52 is the 52-character ID of the spoke to authorize.");
                    eprintln!("Get this from the spoke (output of 'fastn-spoke id').");
                    eprintln!();
                    eprintln!("The alias is taken from the spoke's pending connection.");
                    eprintln!("If no pending connection exists, uses first 8 chars of ID52.");
                    eprintln!("To change the alias, edit spokes.txt in the root kosha.");
                    std::process::exit(1);
                }
            };

            match Hub::load().await {
                Ok(mut hub) => {
                    match hub.add_spoke(id52).await {
                        Ok(alias) => {
                            println!("Spoke added successfully!");
                            println!("ID52:  {}", id52);
                            println!("Alias: {}", alias);
                        }
                        Err(e) => {
                            eprintln!("Failed to add spoke: {}", e);
                            std::process::exit(1);
                        }
                    }
                }
                Err(e) => {
                    eprintln!("Failed to load hub: {}", e);
                    std::process::exit(1);
                }
            }
        }
        Some("list-pending") => {
            match Hub::load().await {
                Ok(hub) => {
                    let pending = hub.list_pending_spokes();
                    if pending.is_empty() {
                        println!("No pending spokes.");
                    } else {
                        println!("Pending spokes (awaiting authorization):");
                        for spoke in pending {
                            println!("  {}: {} (first seen: {})", spoke.id52, spoke.alias, spoke.first_seen.format("%Y-%m-%d %H:%M:%S"));
                        }
                    }
                }
                Err(e) => {
                    eprintln!("Failed to load hub: {}", e);
                    std::process::exit(1);
                }
            }
        }
        Some("remove-spoke") => {
            let id52 = match args.get(2) {
                Some(id) => id,
                None => {
                    eprintln!("Usage: fastn-hub remove-spoke <spoke-id52>");
                    std::process::exit(1);
                }
            };

            match Hub::load().await {
                Ok(mut hub) => {
                    match hub.remove_spoke(id52).await {
                        Ok(true) => {
                            println!("Spoke removed: {}", id52);
                        }
                        Ok(false) => {
                            eprintln!("Spoke not found: {}", id52);
                            std::process::exit(1);
                        }
                        Err(e) => {
                            eprintln!("Failed to remove spoke: {}", e);
                            std::process::exit(1);
                        }
                    }
                }
                Err(e) => {
                    eprintln!("Failed to load hub: {}", e);
                    std::process::exit(1);
                }
            }
        }
        Some("list-spokes") => {
            match Hub::load().await {
                Ok(hub) => {
                    let spokes = hub.list_spokes();
                    if spokes.is_empty() {
                        println!("No authorized spokes.");
                    } else {
                        println!("Authorized spokes:");
                        for spoke in spokes {
                            println!("  {}: {}", spoke.id52, spoke.alias);
                        }
                    }
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
    println!("  fastn-hub init                   Initialize a new hub");
    println!("  fastn-hub                        Run the hub server");
    println!("  fastn-hub id                     Show the hub's ID52");
    println!("  fastn-hub info                   Show hub configuration");
    println!("  fastn-hub add-spoke <id52>       Authorize a spoke to connect");
    println!("  fastn-hub remove-spoke <id52>    Remove spoke authorization");
    println!("  fastn-hub list-spokes            List authorized spokes");
    println!("  fastn-hub list-pending           List pending (unauthorized) spokes");
    println!("  fastn-hub help                   Show this help message");
    println!();
    println!("Environment:");
    println!("  FASTN_HOME  Override the default home directory");
    println!("              Default: ~/.local/share/fastn (Linux)");
    println!("                       ~/Library/Application Support/com.fastn.fastn (macOS)");
    println!();
    println!("Workflow:");
    println!("  1. Initialize the hub: fastn-hub init");
    println!("  2. Share your hub ID52 with spoke users");
    println!("  3. When a spoke wants to connect, add it: fastn-hub add-spoke <spoke-id52>");
    println!("  4. Run the hub server: fastn-hub");
    println!();
    println!("Spokes Configuration (spokes.txt):");
    println!("  Authorized spokes are stored in the root kosha at:");
    println!("  FASTN_HOME/koshas/root/files/spokes.txt");
    println!("  Format: <id52>: <alias> (one per line)");
    println!();
    println!("  The alias defaults to the first 8 characters of the ID52.");
    println!("  To change aliases, edit spokes.txt directly.");
}
