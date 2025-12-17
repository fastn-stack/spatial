//! Cube Example
//!
//! Build: cargo build -p cube --target wasm32-unknown-unknown --release
//! Run:   fastn-shell ./target/wasm32-unknown-unknown/release/cube.wasm

use fastn::{
    app, Command, Event,
    LifecycleEvent, AssetEvent, SceneEvent,
    AssetCommand, SceneCommand, EnvironmentCommand, DebugCommand,
    CreateVolumeData, Transform, VolumeSource, BackgroundData, LogLevel,
};

struct CubeApp {
    cube_loaded: bool,
}

impl CubeApp {
    fn new() -> Self {
        Self { cube_loaded: false }
    }

    fn handle(&mut self, event: Event) -> Vec<Command> {
        match event {
            Event::Lifecycle(LifecycleEvent::Init(init)) => {
                vec![
                    Command::Debug(DebugCommand::Log {
                        level: LogLevel::Info,
                        message: format!("Cube on {:?}, {}x{}", init.platform, init.viewport_width, init.viewport_height),
                    }),
                    Command::Environment(EnvironmentCommand::SetBackground(
                        BackgroundData::Color([0.1, 0.1, 0.2, 1.0]),
                    )),
                    Command::Asset(AssetCommand::Load {
                        asset_id: "cube".to_string(),
                        path: "cube.glb".to_string(),
                    }),
                ]
            }

            Event::Asset(AssetEvent::Loaded(loaded)) if loaded.asset_id == "cube" => {
                self.cube_loaded = true;
                vec![Command::Scene(SceneCommand::CreateVolume(CreateVolumeData {
                    volume_id: "main-cube".to_string(),
                    source: VolumeSource::Asset {
                        asset_id: "cube".to_string(),
                        mesh_index: None,
                    },
                    transform: Transform {
                        position: [0.0, 1.0, -2.0],
                        rotation: [0.0, 0.0, 0.0, 1.0],
                        scale: [0.5, 0.5, 0.5],
                    },
                    material: None,
                }))]
            }

            Event::Scene(SceneEvent::VolumeReady { volume_id }) => {
                vec![Command::Debug(DebugCommand::Log {
                    level: LogLevel::Info,
                    message: format!("Volume {} ready", volume_id),
                })]
            }

            _ => vec![],
        }
    }
}

app!(CubeApp);
