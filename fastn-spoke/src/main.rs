//! fastn-spoke CLI
//!
//! Usage:
//!   fastn-spoke init <hub-id52>  - Initialize spoke with a hub to connect to
//!   fastn-spoke                  - Run the spoke (launches GUI if enabled, otherwise shows info)
//!   fastn-spoke id               - Show the spoke's ID52
//!   fastn-spoke kosha <op>       - Kosha operations (read-file, write-file, list-dir, etc.)

use fastn_spoke::Spoke;
use std::env;
use std::path::PathBuf;

mod kosha;

#[cfg(feature = "gui")]
mod gui;

/// Get the spoke home directory from SPOKE_HOME env var or use the default
fn get_home() -> PathBuf {
    if let Ok(home) = env::var("SPOKE_HOME") {
        PathBuf::from(home)
    } else {
        Spoke::default_home()
    }
}

#[tokio::main]
async fn main() {
    let args: Vec<String> = env::args().collect();
    let home = get_home();

    let command = args.get(1).map(|s| s.as_str());

    match command {
        Some("init") => {
            let hub_id52 = match args.get(2) {
                Some(id) => id,
                None => {
                    eprintln!("Usage: fastn-spoke init <hub-id52> <hub-url> <alias>");
                    eprintln!();
                    eprintln!("Arguments:");
                    eprintln!("  hub-id52  The 52-character ID of the hub you want to connect to.");
                    eprintln!("            Get this from the hub admin (output of 'fastn-hub id').");
                    eprintln!();
                    eprintln!("  hub-url   The HTTP URL of the hub (e.g., 'http://localhost:3000').");
                    eprintln!();
                    eprintln!("  alias     A human-readable name for this spoke (e.g., 'laptop', 'phone').");
                    std::process::exit(1);
                }
            };

            let hub_url = match args.get(3) {
                Some(url) => url,
                None => {
                    eprintln!("Usage: fastn-spoke init <hub-id52> <hub-url> <alias>");
                    eprintln!();
                    eprintln!("Please provide the hub's HTTP URL.");
                    eprintln!("Example: fastn-spoke init {} http://localhost:3000 my-laptop", hub_id52);
                    std::process::exit(1);
                }
            };

            let alias = match args.get(4) {
                Some(a) => a,
                None => {
                    eprintln!("Usage: fastn-spoke init <hub-id52> <hub-url> <alias>");
                    eprintln!();
                    eprintln!("Please provide an alias (human-readable name) for this spoke.");
                    eprintln!("Example: fastn-spoke init {} {} my-laptop", hub_id52, hub_url);
                    std::process::exit(1);
                }
            };

            match Spoke::init(home, hub_id52, hub_url, alias).await {
                Ok(spoke) => {
                    println!("Spoke initialized successfully!");
                    println!();
                    println!("Spoke ID52: {}", spoke.id52());
                    println!("Alias:      {}", spoke.alias());
                    println!("Hub ID52:   {}", spoke.hub_id52());
                    println!("Hub URL:    {}", spoke.hub_url());
                    println!("Home:       {:?}", spoke.home());
                    println!();
                    println!("Next steps:");
                    println!("  1. Give your spoke ID52 to the hub admin");
                    println!("  2. Hub admin runs: fastn-hub add-spoke {}", spoke.id52());
                    println!("  3. Then run: fastn-spoke kosha read-file self root spokes.txt");
                }
                Err(e) => {
                    eprintln!("Failed to initialize spoke: {}", e);
                    std::process::exit(1);
                }
            }
        }
        Some("id") => {
            match Spoke::load(&home).await {
                Ok(spoke) => {
                    println!("{}", spoke.id52());
                }
                Err(e) => {
                    eprintln!("Failed to load spoke: {}", e);
                    std::process::exit(1);
                }
            }
        }
        Some("info") => {
            match Spoke::load(&home).await {
                Ok(spoke) => {
                    println!("Spoke ID52: {}", spoke.id52());
                    println!("Alias:      {}", spoke.alias());
                    println!("Hub ID52:   {}", spoke.hub_id52());
                    println!("Hub URL:    {}", spoke.hub_url());
                    println!("Home:       {:?}", spoke.home());
                }
                Err(e) => {
                    eprintln!("Failed to load spoke: {}", e);
                    std::process::exit(1);
                }
            }
        }
        Some("kosha") => {
            kosha::run(&args[2..], &home).await;
        }
        Some("help") | Some("-h") | Some("--help") => {
            print_help();
        }
        None => {
            #[cfg(feature = "gui")]
            {
                // Launch Tauri GUI
                gui::run(home);
                return;
            }

            #[cfg(not(feature = "gui"))]
            {
                // With HTTP transport, there's no persistent connection
                // Just show info and suggest using kosha commands
                match Spoke::load(&home).await {
                    Ok(spoke) => {
                        println!("Spoke ID52: {}", spoke.id52());
                        println!("Hub ID52:   {}", spoke.hub_id52());
                        println!("Hub URL:    {}", spoke.hub_url());
                        println!();
                        println!("With HTTP transport, each request is independent.");
                        println!("Use 'fastn-spoke kosha' commands to interact with the hub.");
                        println!();
                        println!("Example:");
                        println!("  fastn-spoke kosha read-file self root spokes.txt");
                    }
                    Err(e) => {
                        eprintln!("Failed to load spoke: {}", e);
                        eprintln!();
                        eprintln!("Run 'fastn-spoke init <hub-id52> <hub-url> <alias>' first to initialize.");
                        std::process::exit(1);
                    }
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
    println!("fastn-spoke - Spoke client for fastn P2P network (HTTP transport)");
    println!();
    println!("Usage:");
    println!("  fastn-spoke init <hub-id52> <hub-url> <alias>  Initialize spoke with a hub");
    println!("  fastn-spoke                                    Show spoke info");
    println!("  fastn-spoke id                                 Show the spoke's ID52");
    println!("  fastn-spoke info                               Show spoke configuration");
    println!("  fastn-spoke kosha <operation> ...              Kosha operations (see below)");
    println!("  fastn-spoke help                               Show this help message");
    println!();
    println!("Kosha Operations:");
    println!("  fastn-spoke kosha read-file <hub> <kosha> <path>");
    println!("  fastn-spoke kosha write-file <hub> <kosha> <path> <file>");
    println!("  fastn-spoke kosha list-dir <hub> <kosha> <path>");
    println!("  fastn-spoke kosha get-versions <hub> <kosha> <path>");
    println!("  fastn-spoke kosha read-version <hub> <kosha> <path> <timestamp>");
    println!("  fastn-spoke kosha rename <hub> <kosha> <from> <to>");
    println!("  fastn-spoke kosha delete <hub> <kosha> <path>");
    println!("  fastn-spoke kosha kv-get <hub> <kosha> <key>");
    println!("  fastn-spoke kosha kv-set <hub> <kosha> <key> <value>");
    println!("  fastn-spoke kosha kv-delete <hub> <kosha> <key>");
    println!();
    println!("Hub Aliases:");
    println!("  self      Access your own hub directly (no ACL checks)");
    println!("  <alias>   Access a remote hub via hub-to-hub forwarding (ACL applies)");
    println!();
    println!("Arguments:");
    println!("  hub-id52  The 52-character ID of the hub to connect to");
    println!("  hub-url   The HTTP URL of the hub (e.g., 'http://localhost:3000')");
    println!("  alias     A human-readable name for this spoke (e.g., 'laptop', 'phone')");
    println!();
    println!("Environment:");
    println!("  SPOKE_HOME  Override the default home directory");
    println!("              Default: ~/.fastn-spoke (or platform-specific)");
    println!();
    println!("Workflow:");
    println!("  1. Get the hub's ID52 and HTTP URL from the hub admin");
    println!("  2. Run: fastn-spoke init <hub-id52> <hub-url> <alias>");
    println!("  3. Give your spoke ID52 to the hub admin");
    println!("  4. Hub admin runs: fastn-hub add-spoke <your-spoke-id52>");
    println!("  5. Run: fastn-spoke kosha read-file self root spokes.txt");
}
