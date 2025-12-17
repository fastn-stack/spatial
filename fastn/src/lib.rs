//! fastn - Build Spatial/XR Applications in Rust
//!
//! # Quick Start
//!
//! ```rust,ignore
//! #[fastn::app]
//! fn init() -> fastn::App {
//!     let mut app = fastn::init();
//!     app.add_volume_from_glb("cube.glb", 0);
//!     app
//! }
//! ```
//!
//! # Build and Run
//!
//! ```bash
//! cargo build --target wasm32-unknown-unknown --release
//! fastn-shell ./target/wasm32-unknown-unknown/release/your_app.wasm
//! ```

mod app;
mod protocol;

#[doc(hidden)]
pub mod wasm_bridge;

// Re-export the proc macro
pub use fastn_macros::app;

// Re-export App and init
pub use app::{App, init};

// Re-export protocol types for advanced usage
pub use protocol::*;
