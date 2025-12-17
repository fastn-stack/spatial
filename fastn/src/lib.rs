//! fastn - Build Spatial/XR Applications in Rust
//!
//! # Quick Start
//!
//! ```rust,ignore
//! use fastn::{app, Event, Command, LifecycleEvent};
//!
//! struct MyApp;
//!
//! impl MyApp {
//!     fn new() -> Self { Self }
//!
//!     fn handle(&mut self, event: Event) -> Vec<Command> {
//!         match event {
//!             Event::Lifecycle(LifecycleEvent::Init(_)) => {
//!                 vec![Command::Asset(AssetCommand::Load {
//!                     asset_id: "cube".into(),
//!                     path: "cube.glb".into(),
//!                 })]
//!             }
//!             _ => vec![],
//!         }
//!     }
//! }
//!
//! app!(MyApp);
//! ```
//!
//! # Build and Run
//!
//! ```bash
//! cargo build --target wasm32-unknown-unknown --release
//! fastn-shell ./target/wasm32-unknown-unknown/release/your_app.wasm
//! ```

mod protocol;

#[doc(hidden)]
pub mod wasm_bridge;

// Re-export everything from protocol
pub use protocol::*;
