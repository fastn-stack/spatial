//! fastn-shell CLI binary
//!
//! Usage: fastn-shell <path-to-wasm>

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let wasm_path = args.get(1).cloned().unwrap_or_else(|| {
        eprintln!("Usage: fastn-shell <path-to-wasm>");
        eprintln!("Example: fastn-shell ./app.wasm");
        std::process::exit(1);
    });

    if let Err(e) = fastn_shell::run(&wasm_path) {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }
}
