//! fastn-cli - CLI tools for building, serving, and running fastn apps
//!
//! # Usage
//!
//! In your app's `main.rs`:
//! ```rust,ignore
//! fn main() {
//!     fastn::main(); // or fastn_cli::main()
//! }
//! ```
//!
//! Then run:
//! - `cargo run` - Run native shell (default)
//! - `cargo run -- build` - Build for web (creates dist/)
//! - `cargo run -- serve` - Build and serve web version

mod web_shell;

use clap::{Parser, Subcommand};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Parser)]
#[command(name = "fastn")]
#[command(about = "Build and run spatial/XR applications", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Build the app for web (creates dist/ folder)
    Build {
        /// Build in release mode
        #[arg(long, default_value = "true")]
        release: bool,

        /// Output directory
        #[arg(short, long, default_value = "dist")]
        output: String,
    },
    /// Build and serve the web version
    Serve {
        /// Port to serve on
        #[arg(short, long, default_value = "8080")]
        port: u16,

        /// Build in release mode
        #[arg(long, default_value = "true")]
        release: bool,
    },
    /// Run the native shell (default if no subcommand)
    Run {
        /// Build in release mode
        #[arg(long, default_value = "true")]
        release: bool,
    },
}

/// Main entry point for fastn CLI
/// Called from app's main.rs: `fn main() { fastn::main(); }`
pub fn main() {
    let cli = Cli::parse();

    // Get crate info
    let crate_info = match get_crate_info() {
        Ok(info) => info,
        Err(e) => {
            eprintln!("Error: {}", e);
            eprintln!("Make sure you're running from a Cargo project directory.");
            std::process::exit(1);
        }
    };

    match cli.command {
        Some(Commands::Build { release, output }) => {
            if let Err(e) = cmd_build(&crate_info, release, &output) {
                eprintln!("Build failed: {}", e);
                std::process::exit(1);
            }
        }
        Some(Commands::Serve { port, release }) => {
            if let Err(e) = cmd_serve(&crate_info, release, port) {
                eprintln!("Serve failed: {}", e);
                std::process::exit(1);
            }
        }
        Some(Commands::Run { release }) => {
            if let Err(e) = cmd_run(&crate_info, release) {
                eprintln!("Run failed: {}", e);
                std::process::exit(1);
            }
        }
        None => {
            // Default: run with release=true
            if let Err(e) = cmd_run(&crate_info, true) {
                eprintln!("Run failed: {}", e);
                std::process::exit(1);
            }
        }
    }
}

struct CrateInfo {
    name: String,
    root: PathBuf,
    target_dir: PathBuf,
}

fn get_crate_info() -> Result<CrateInfo, String> {
    let cargo_toml = PathBuf::from("Cargo.toml");
    if !cargo_toml.exists() {
        return Err("Cargo.toml not found in current directory".to_string());
    }

    // Use cargo metadata to get complete information about the project
    let output = Command::new("cargo")
        .args(["metadata", "--format-version=1", "--no-deps"])
        .output()
        .map_err(|e| format!("Failed to run cargo metadata: {}", e))?;

    if !output.status.success() {
        return Err("Failed to get cargo metadata".to_string());
    }

    let metadata: serde_json::Value = serde_json::from_slice(&output.stdout)
        .map_err(|e| format!("Failed to parse cargo metadata: {}", e))?;

    let target_dir = metadata
        .get("target_directory")
        .and_then(|v| v.as_str())
        .map(PathBuf::from)
        .ok_or("Could not find target_directory in cargo metadata")?;

    let workspace_root = metadata
        .get("workspace_root")
        .and_then(|v| v.as_str())
        .map(PathBuf::from)
        .ok_or("Could not find workspace_root in cargo metadata")?;

    // Get packages - filter to only cdylib packages (WASM targets)
    let packages = metadata
        .get("packages")
        .and_then(|v| v.as_array())
        .ok_or("Could not find packages in cargo metadata")?;

    // Filter to packages that have cdylib target (i.e., are fastn apps)
    // Exclude "fastn" itself since it's a library, not an app
    let cdylib_packages: Vec<_> = packages
        .iter()
        .filter(|pkg| {
            // Skip the fastn library itself
            if pkg.get("name").and_then(|v| v.as_str()) == Some("fastn") {
                return false;
            }
            pkg.get("targets")
                .and_then(|t| t.as_array())
                .map(|targets| {
                    targets.iter().any(|target| {
                        target
                            .get("crate_types")
                            .and_then(|ct| ct.as_array())
                            .map(|types| types.iter().any(|t| t.as_str() == Some("cdylib")))
                            .unwrap_or(false)
                    })
                })
                .unwrap_or(false)
        })
        .collect();

    let (name, root) = if cdylib_packages.len() == 1 {
        // Single cdylib package - use it
        let pkg = cdylib_packages[0];
        let name = pkg
            .get("name")
            .and_then(|v| v.as_str())
            .ok_or("Could not find package name")?
            .to_string();
        let manifest_path = pkg
            .get("manifest_path")
            .and_then(|v| v.as_str())
            .ok_or("Could not find manifest_path")?;
        let root = PathBuf::from(manifest_path)
            .parent()
            .ok_or("Could not get package directory")?
            .to_path_buf();
        (name, root)
    } else if cdylib_packages.is_empty() {
        // No cdylib packages - try to find package from current Cargo.toml
        let content = fs::read_to_string(&cargo_toml)
            .map_err(|e| format!("Failed to read Cargo.toml: {}", e))?;
        let parsed: toml::Value = content
            .parse()
            .map_err(|e| format!("Failed to parse Cargo.toml: {}", e))?;
        let name = parsed
            .get("package")
            .and_then(|p| p.get("name"))
            .and_then(|n| n.as_str())
            .ok_or("No cdylib packages found. Make sure your Cargo.toml has [lib] crate-type = [\"cdylib\"]")?
            .to_string();
        (name, workspace_root)
    } else {
        // Multiple cdylib packages - list them
        let names: Vec<_> = cdylib_packages
            .iter()
            .filter_map(|pkg| pkg.get("name").and_then(|v| v.as_str()))
            .collect();
        return Err(format!(
            "Multiple fastn apps found: {}. Please run from the specific app directory.",
            names.join(", ")
        ));
    };

    Ok(CrateInfo {
        name,
        root,
        target_dir,
    })
}

fn cmd_build(crate_info: &CrateInfo, release: bool, output: &str) -> Result<(), String> {
    println!("Building {} for web...", crate_info.name);

    // Build WASM
    let wasm_path = build_wasm(crate_info, release)?;

    // Create dist directory
    let dist_dir = crate_info.root.join(output);
    fs::create_dir_all(&dist_dir)
        .map_err(|e| format!("Failed to create dist directory: {}", e))?;

    // Compute WASM hash for cache busting
    let wasm_content =
        fs::read(&wasm_path).map_err(|e| format!("Failed to read WASM file: {}", e))?;
    let hash = compute_hash(&wasm_content);
    let wasm_filename = format!("{}-{}.wasm", crate_info.name, &hash[..8]);

    // Copy WASM to dist
    let dist_wasm = dist_dir.join(&wasm_filename);
    fs::copy(&wasm_path, &dist_wasm).map_err(|e| format!("Failed to copy WASM: {}", e))?;
    println!("  Created {}", wasm_filename);

    // Write shell JS files
    fs::write(dist_dir.join("shell-common.js"), web_shell::SHELL_COMMON_JS)
        .map_err(|e| format!("Failed to write shell-common.js: {}", e))?;
    fs::write(dist_dir.join("shell-webgpu.js"), web_shell::SHELL_WEBGPU_JS)
        .map_err(|e| format!("Failed to write shell-webgpu.js: {}", e))?;
    fs::write(
        dist_dir.join("shell-webgl-xr.js"),
        web_shell::SHELL_WEBGL_XR_JS,
    )
    .map_err(|e| format!("Failed to write shell-webgl-xr.js: {}", e))?;
    println!("  Created shell JS files");

    // Generate index.html - use custom template if present, otherwise default
    let custom_template = crate_info.root.join("index.html.tmpl");
    let html_template = if custom_template.exists() {
        println!("  Using custom index.html.tmpl");
        fs::read_to_string(&custom_template)
            .map_err(|e| format!("Failed to read index.html.tmpl: {}", e))?
    } else {
        web_shell::INDEX_HTML_TEMPLATE.to_string()
    };
    let html = html_template
        .replace("{{APP_NAME}}", &crate_info.name)
        .replace("{{WASM_FILE}}", &format!("./{}", wasm_filename));
    fs::write(dist_dir.join("index.html"), html)
        .map_err(|e| format!("Failed to write index.html: {}", e))?;
    println!("  Created index.html");

    // Copy assets (look for assets/ directory)
    copy_assets(crate_info, &dist_dir)?;

    println!("\nBuild complete! Output in {}/", output);
    Ok(())
}

fn cmd_serve(crate_info: &CrateInfo, release: bool, port: u16) -> Result<(), String> {
    // First build
    cmd_build(crate_info, release, "dist")?;

    let dist_dir = crate_info.root.join("dist");

    println!("\nStarting HTTP server on http://localhost:{}", port);
    println!("Press Ctrl+C to stop\n");

    serve_directory(&dist_dir, port)
}

#[cfg(feature = "native-shell")]
fn cmd_run(crate_info: &CrateInfo, release: bool) -> Result<(), String> {
    println!("Building {} for native...", crate_info.name);

    // Build WASM first
    let wasm_path = build_wasm(crate_info, release)?;

    println!("Running native shell...\n");

    // Call fastn-shell directly as a library
    fastn_shell::run(wasm_path.to_str().ok_or("Invalid WASM path")?)
}

#[cfg(not(feature = "native-shell"))]
fn cmd_run(_crate_info: &CrateInfo, _release: bool) -> Result<(), String> {
    Err("Native shell support is not enabled. Build with --features native-shell or use default features.\n\
         For CI builds that only need 'build' or 'serve', use: cargo run --no-default-features -- build".to_string())
}

fn build_wasm(crate_info: &CrateInfo, release: bool) -> Result<PathBuf, String> {
    let mut cmd = Command::new("cargo");
    cmd.arg("build")
        .arg("--lib") // Only build library target (not binary)
        .arg("-p")
        .arg(&crate_info.name) // Specify the package to avoid building workspace deps
        .arg("--target")
        .arg("wasm32-unknown-unknown");

    if release {
        cmd.arg("--release");
    }

    println!(
        "  Running cargo build --lib -p {} --target wasm32-unknown-unknown{}",
        crate_info.name,
        if release { " --release" } else { "" }
    );

    let status = cmd
        .status()
        .map_err(|e| format!("Failed to run cargo: {}", e))?;

    if !status.success() {
        return Err("WASM build failed".to_string());
    }

    let profile = if release { "release" } else { "debug" };
    let wasm_path = crate_info
        .target_dir
        .join("wasm32-unknown-unknown")
        .join(profile)
        .join(format!("{}.wasm", crate_info.name));

    if !wasm_path.exists() {
        return Err(format!("WASM file not found at {:?}", wasm_path));
    }

    println!("  Built {:?}", wasm_path);
    Ok(wasm_path)
}

fn compute_hash(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    format!("{:x}", hasher.finalize())
}

fn copy_assets(crate_info: &CrateInfo, dist_dir: &Path) -> Result<(), String> {
    let assets_dir = crate_info.root.join("assets");
    if !assets_dir.exists() {
        return Ok(());
    }

    println!("  Copying assets...");

    for entry in walkdir::WalkDir::new(&assets_dir) {
        let entry = entry.map_err(|e| format!("Failed to walk assets: {}", e))?;
        let path = entry.path();

        if path.is_file() {
            let relative = path
                .strip_prefix(&assets_dir)
                .map_err(|e| format!("Failed to get relative path: {}", e))?;
            let dest = dist_dir.join(relative);

            if let Some(parent) = dest.parent() {
                fs::create_dir_all(parent)
                    .map_err(|e| format!("Failed to create directory: {}", e))?;
            }

            fs::copy(path, &dest)
                .map_err(|e| format!("Failed to copy asset {:?}: {}", path, e))?;
            println!("    {}", relative.display());
        }
    }

    Ok(())
}

fn serve_directory(dir: &Path, port: u16) -> Result<(), String> {
    let server = tiny_http::Server::http(format!("0.0.0.0:{}", port))
        .map_err(|e| format!("Failed to start HTTP server: {}", e))?;

    for request in server.incoming_requests() {
        let url = request.url().to_string();
        let path = if url == "/" { "/index.html" } else { &url };
        let file_path = dir.join(&path[1..]); // Remove leading /

        let response = if file_path.exists() && file_path.is_file() {
            let content = fs::read(&file_path).unwrap_or_default();
            let content_type = get_content_type(&file_path);

            tiny_http::Response::from_data(content)
                .with_header(
                    tiny_http::Header::from_bytes(&b"Content-Type"[..], content_type.as_bytes())
                        .unwrap(),
                )
                .with_header(
                    tiny_http::Header::from_bytes(
                        &b"Cross-Origin-Opener-Policy"[..],
                        &b"same-origin"[..],
                    )
                    .unwrap(),
                )
                .with_header(
                    tiny_http::Header::from_bytes(
                        &b"Cross-Origin-Embedder-Policy"[..],
                        &b"require-corp"[..],
                    )
                    .unwrap(),
                )
        } else {
            tiny_http::Response::from_string("404 Not Found").with_status_code(404)
        };

        let _ = request.respond(response);
    }

    Ok(())
}

fn get_content_type(path: &Path) -> String {
    match path.extension().and_then(|e| e.to_str()) {
        Some("html") => "text/html".to_string(),
        Some("js") => "application/javascript".to_string(),
        Some("wasm") => "application/wasm".to_string(),
        Some("css") => "text/css".to_string(),
        Some("json") => "application/json".to_string(),
        Some("glb") => "model/gltf-binary".to_string(),
        Some("gltf") => "model/gltf+json".to_string(),
        Some("png") => "image/png".to_string(),
        Some("jpg") | Some("jpeg") => "image/jpeg".to_string(),
        _ => "application/octet-stream".to_string(),
    }
}
