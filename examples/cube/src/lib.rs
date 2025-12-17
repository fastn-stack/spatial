//! Cube Example - Simple 3D cube rendered using fastn
//!
//! This example shows how to use the fastn_app! macro to create a spatial app.
//!
//! Build with:
//! ```bash
//! cargo build -p cube --target wasm32-unknown-unknown --release
//! ```
//!
//! Run with:
//! ```bash
//! fastn-shell ./target/wasm32-unknown-unknown/release/cube.wasm
//! ```

use fastn::{
    fastn_app, Command, Core, Event,
    // Events
    LifecycleEvent, AssetEvent, SceneEvent,
    // Commands
    AssetCommand, SceneCommand, EnvironmentCommand, DebugCommand,
    // Types
    CreateVolumeData, Transform, VolumeSource, BackgroundData, LogLevel,
};

/// Simple cube application state
struct CubeApp {
    /// Has the cube asset been loaded?
    cube_loaded: bool,
}

impl CubeApp {
    fn new() -> Self {
        Self { cube_loaded: false }
    }

    fn log(&self, level: LogLevel, message: impl Into<String>) -> Command {
        Command::Debug(DebugCommand::Log {
            level,
            message: message.into(),
        })
    }
}

impl Core for CubeApp {
    fn handle(&mut self, event: Event) -> Vec<Command> {
        match event {
            Event::Lifecycle(LifecycleEvent::Init(init)) => {
                vec![
                    self.log(LogLevel::Info, format!(
                        "Cube example initialized on {:?}, viewport: {}x{}",
                        init.platform, init.viewport_width, init.viewport_height
                    )),
                    // Set a nice dark blue background
                    Command::Environment(EnvironmentCommand::SetBackground(
                        BackgroundData::Color([0.1, 0.1, 0.2, 1.0]),
                    )),
                    // Load the cube model
                    Command::Asset(AssetCommand::Load {
                        asset_id: "cube".to_string(),
                        path: "cube.glb".to_string(),
                    }),
                ]
            }

            Event::Asset(AssetEvent::Loaded(loaded)) if loaded.asset_id == "cube" => {
                self.cube_loaded = true;
                vec![
                    self.log(LogLevel::Info, "Cube asset loaded!"),
                    // Create the cube volume in the scene
                    Command::Scene(SceneCommand::CreateVolume(CreateVolumeData {
                        volume_id: "main-cube".to_string(),
                        source: VolumeSource::Asset {
                            asset_id: "cube".to_string(),
                            mesh_index: None,
                        },
                        transform: Transform {
                            position: [0.0, 1.0, -2.0], // 1m up, 2m in front
                            rotation: [0.0, 0.0, 0.0, 1.0],
                            scale: [0.5, 0.5, 0.5],
                        },
                        material: None,
                    })),
                ]
            }

            Event::Scene(SceneEvent::VolumeReady { volume_id }) => {
                vec![self.log(LogLevel::Info, format!("Volume {} is ready", volume_id))]
            }

            Event::Lifecycle(LifecycleEvent::Shutdown) => {
                vec![self.log(LogLevel::Info, "Cube example shutting down")]
            }

            _ => vec![],
        }
    }
}

// Generate WASM exports
fastn_app!(CubeApp, CubeApp::new());
