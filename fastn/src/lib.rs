//! fastn - Build Spatial/XR Applications in Rust
//!
//! This library provides everything you need to build spatial/XR applications
//! that run on desktop (wgpu), web (WebGL/WebXR), and native XR devices.
//!
//! ## Quick Start
//!
//! The simplest way to get started is using the `fastn_app!` macro:
//!
//! ```rust,ignore
//! use fastn::{fastn_app, Core, Command, Event, LifecycleEvent};
//!
//! struct MyApp {
//!     initialized: bool,
//! }
//!
//! impl MyApp {
//!     fn new() -> Self {
//!         Self { initialized: false }
//!     }
//! }
//!
//! impl Core for MyApp {
//!     fn handle(&mut self, event: Event) -> Vec<Command> {
//!         match event {
//!             Event::Lifecycle(LifecycleEvent::Init(_)) => {
//!                 self.initialized = true;
//!                 vec![
//!                     // Load a 3D model
//!                     Command::Asset(AssetCommand::Load {
//!                         asset_id: "cube".into(),
//!                         path: "cube.glb".into(),
//!                     }),
//!                 ]
//!             }
//!             _ => vec![],
//!         }
//!     }
//! }
//!
//! // This macro generates all the WASM exports
//! fastn_app!(MyApp, MyApp::new());
//! ```
//!
//! ## Architecture
//!
//! fastn uses a shell-core architecture that separates platform-specific code
//! from your application logic:
//!
//! ```text
//! ┌────────────────────────────────────────────────────────────┐
//! │                    fastn-shell                             │
//! │  (Rust/wgpu, JS/WebXR, Swift/visionOS)                    │
//! │                                                            │
//! │  - Platform APIs (wgpu, WebGL, Metal)                      │
//! │  - Input capture (keyboard, mouse, controllers)            │
//! │  - Asset loading (GLB/glTF)                                │
//! │  - Rendering                                               │
//! │  - Loads your app as WASM                                  │
//! └────────────────────┬───────────────────────────────────────┘
//!                      │
//!                      │ Events (Shell → Core) - JSON
//!                      │ Commands (Core → Shell) - JSON
//!                      │
//! ┌────────────────────▼───────────────────────────────────────┐
//! │                    Your App (WASM)                         │
//! │  Compiled from: your crate + fastn                         │
//! │                                                            │
//! │  - Your application logic                                  │
//! │  - Scene setup (which GLB files, positions, etc.)          │
//! │  - Event handling                                          │
//! └────────────────────┬───────────────────────────────────────┘
//!                      │
//!                      │ Uses fastn types
//!                      │
//! ┌────────────────────▼───────────────────────────────────────┐
//! │                    fastn (this crate)                      │
//! │                                                            │
//! │  - Protocol types (Event, Command)                         │
//! │  - Core trait definition                                   │
//! │  - WASM bridge (fastn_app! macro)                          │
//! │  - JSON serialization                                      │
//! └────────────────────────────────────────────────────────────┘
//! ```
//!
//! ## Building and Running
//!
//! 1. Build your app to WASM:
//!    ```bash
//!    cargo build --target wasm32-unknown-unknown --release
//!    ```
//!
//! 2. Run with fastn-shell:
//!    ```bash
//!    fastn-shell ./target/wasm32-unknown-unknown/release/your_app.wasm
//!    ```

// Shell-Core protocol types
pub mod protocol;

// WASM bridge for shell-core communication
pub mod wasm_bridge;

// Simplified API for common use cases
pub mod simple;

// Example Core implementation
pub mod example_core;

// Re-exports for convenience
pub use protocol::{Command, Core, Event};
pub use protocol::{
    // Event categories
    AssetEvent, InputEvent, LifecycleEvent, MediaEvent, NetworkEvent, SceneEvent, TimerEvent,
    XrEvent,
    // Lifecycle event data
    InitEvent, FrameEvent, ResizeEvent, Platform,
    // Input event data
    KeyboardEvent, KeyEventData, MouseEvent, TouchEvent, GamepadEvent,
    // Asset event data
    AssetLoadedData, MeshInfo, AnimationInfo, SkeletonInfo, AssetType,
    // Scene event data
    CreateVolumeData, SetTransformData,
    // Command categories
    AnimationCommand, AssetCommand, DebugCommand, EnvironmentCommand, MaterialCommand,
    MediaCommand, NetworkCommand, SceneCommand, TimerCommand, XrCommand,
    // Common types
    Transform, VolumeSource, Primitive, BackgroundData, LogLevel,
    // Handler traits
    AssetHandler, InputHandler, LifecycleHandler, MediaHandler, NetworkHandler, SceneHandler,
    TimerHandler, XrHandler,
};
