//! fastn-spoke CLI
//!
//! Usage:
//!   fastn-spoke init <hub-id52>  - Initialize spoke with a hub to connect to
//!   fastn-spoke                  - Run the spoke (connects to hub, retries until accepted)
//!   fastn-spoke id               - Show the spoke's ID52

use fastn_spoke::Spoke;
use std::env;
use std::time::Duration;

#[tokio::main]
async fn main() {
    let args: Vec<String> = env::args().collect();

    let command = args.get(1).map(|s| s.as_str());

    match command {
        Some("init") => {
            let hub_id52 = match args.get(2) {
                Some(id) => id,
                None => {
                    eprintln!("Usage: fastn-spoke init <hub-id52> <alias>");
                    eprintln!();
                    eprintln!("The hub-id52 is the 52-character ID of the hub you want to connect to.");
                    eprintln!("Get this from the hub admin (output of 'fastn-hub id').");
                    eprintln!();
                    eprintln!("The alias is a human-readable name for this spoke (e.g., 'laptop', 'phone').");
                    std::process::exit(1);
                }
            };

            let alias = match args.get(3) {
                Some(a) => a,
                None => {
                    eprintln!("Usage: fastn-spoke init <hub-id52> <alias>");
                    eprintln!();
                    eprintln!("Please provide an alias (human-readable name) for this spoke.");
                    eprintln!("Example: fastn-spoke init {} my-laptop", hub_id52);
                    std::process::exit(1);
                }
            };

            match Spoke::init(hub_id52, alias).await {
                Ok(spoke) => {
                    println!("Spoke initialized successfully!");
                    println!();
                    println!("Spoke ID52: {}", spoke.id52());
                    println!("Alias:      {}", spoke.alias());
                    println!("Hub ID52:   {}", spoke.hub_id52());
                    println!("Home:       {:?}", spoke.home());
                    println!();
                    println!("Next steps:");
                    println!("  1. Give your spoke ID52 to the hub admin");
                    println!("  2. Hub admin runs: fastn-hub add-spoke {}", spoke.id52());
                    println!("  3. Then run: fastn-spoke");
                }
                Err(e) => {
                    eprintln!("Failed to initialize spoke: {}", e);
                    std::process::exit(1);
                }
            }
        }
        Some("id") => {
            match Spoke::load().await {
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
            match Spoke::load().await {
                Ok(spoke) => {
                    println!("Spoke ID52: {}", spoke.id52());
                    println!("Alias:      {}", spoke.alias());
                    println!("Hub ID52:   {}", spoke.hub_id52());
                    println!("Home:       {:?}", spoke.home());
                }
                Err(e) => {
                    eprintln!("Failed to load spoke: {}", e);
                    std::process::exit(1);
                }
            }
        }
        Some("help") | Some("-h") | Some("--help") => {
            print_help();
        }
        None => {
            // Run the spoke - connect to hub with retry
            match Spoke::load().await {
                Ok(spoke) => {
                    println!("Spoke ID52: {}", spoke.id52());
                    println!("Hub ID52:   {}", spoke.hub_id52());
                    println!();
                    println!("Connecting to hub...");

                    let retry_interval = Duration::from_secs(5);

                    loop {
                        match spoke.connect().await {
                            Ok(_conn) => {
                                println!("ONLINE - Connected to hub!");
                                // For now, just stay connected
                                // In the future, we'd keep the connection alive
                                // and handle requests/events
                                loop {
                                    tokio::time::sleep(Duration::from_secs(30)).await;
                                    println!("Still connected...");
                                }
                            }
                            Err(e) => {
                                eprintln!("Connection failed: {}", e);
                                eprintln!("Retrying in {:?}...", retry_interval);
                                eprintln!();
                                eprintln!("If hub is rejecting your connection, ask admin to run:");
                                eprintln!("  fastn-hub add-spoke {}", spoke.id52());
                                eprintln!();
                                tokio::time::sleep(retry_interval).await;
                            }
                        }
                    }
                }
                Err(e) => {
                    eprintln!("Failed to load spoke: {}", e);
                    eprintln!();
                    eprintln!("Run 'fastn-spoke init <hub-id52>' first to initialize the spoke.");
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
    println!("fastn-spoke - Spoke client for fastn P2P network");
    println!();
    println!("Usage:");
    println!("  fastn-spoke init <hub-id52> <alias>  Initialize spoke with a hub");
    println!("  fastn-spoke                          Run the spoke (connects to hub)");
    println!("  fastn-spoke id                       Show the spoke's ID52");
    println!("  fastn-spoke info                     Show spoke configuration");
    println!("  fastn-spoke help                     Show this help message");
    println!();
    println!("Arguments:");
    println!("  hub-id52    The 52-character ID of the hub to connect to");
    println!("  alias       A human-readable name for this spoke (e.g., 'laptop', 'phone')");
    println!();
    println!("Environment:");
    println!("  SPOKE_HOME  Override the default home directory");
    println!("              Default: ~/.fastn-spoke (or platform-specific)");
    println!();
    println!("Workflow:");
    println!("  1. Get the hub's ID52 from the hub admin");
    println!("  2. Run: fastn-spoke init <hub-id52> <alias>");
    println!("  3. Give your spoke ID52 to the hub admin");
    println!("  4. Hub admin runs: fastn-hub add-spoke <your-spoke-id52>");
    println!("  5. Run: fastn-spoke");
}
