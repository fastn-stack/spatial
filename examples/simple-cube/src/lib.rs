//! Super Simple Cube Example
//!
//! This example shows the simplest possible fastn app - just 3 lines of code!
//!
//! Build with:
//! ```bash
//! cargo build -p simple-cube --target wasm32-unknown-unknown --release
//! ```
//!
//! Run with:
//! ```bash
//! fastn-shell ./target/wasm32-unknown-unknown/release/simple_cube.wasm
//! ```

use fastn::{fastn_app, simple::RenderGlbApp};

// That's it! This renders cube.glb at position (0, 1, -2) with a dark blue background
fastn_app!(RenderGlbApp, RenderGlbApp::new("cube.glb"));
