//! Example Core implementation showing how to use the protocol

use crate::protocol::*;

/// Example application state
pub struct ExampleCore {
    /// Has the scene been initialized?
    initialized: bool,
    /// Asset ID for the cube GLB
    cube_asset_id: AssetId,
    /// Volume ID for the main cube
    cube_volume_id: VolumeId,
    /// Current rotation angle
    rotation: f32,
    /// Is XR mode active?
    xr_active: bool,
    /// Counter for generating unique IDs
    next_id: u64,
}

impl ExampleCore {
    pub fn new() -> Self {
        Self {
            initialized: false,
            cube_asset_id: String::new(),
            cube_volume_id: String::new(),
            rotation: 0.0,
            xr_active: false,
            next_id: 1,
        }
    }

    fn alloc_id(&mut self, prefix: &str) -> String {
        let id = format!("{}-{}", prefix, self.next_id);
        self.next_id += 1;
        id
    }

    fn log(&self, level: LogLevel, message: impl Into<String>) -> Command {
        Command::Debug(DebugCommand::Log {
            level,
            message: message.into(),
        })
    }
}

impl Core for ExampleCore {
    fn handle(&mut self, event: Event) -> Vec<Command> {
        match event {
            Event::Lifecycle(e) => self.handle_lifecycle(e),
            Event::Input(e) => self.handle_input(e),
            Event::Xr(e) => self.handle_xr(e),
            Event::Asset(e) => self.handle_asset(e),
            Event::Scene(e) => self.handle_scene(e),
            Event::Network(_) => Vec::new(), // Not used in example
            Event::Media(_) => Vec::new(),   // Not used in example
            Event::Timer(_) => Vec::new(),   // Not used in example
        }
    }
}

impl LifecycleHandler for ExampleCore {
    fn handle_lifecycle(&mut self, event: LifecycleEvent) -> Vec<Command> {
        let mut commands = Vec::new();

        match event {
            LifecycleEvent::Init(init) => {
                commands.push(self.log(
                    LogLevel::Info,
                    format!(
                        "Core initialized on {:?}, viewport: {}x{}, XR: {}",
                        init.platform,
                        init.viewport_width,
                        init.viewport_height,
                        init.xr_immersive_vr
                    ),
                ));

                // Set background color
                commands.push(Command::Environment(EnvironmentCommand::SetBackground(
                    BackgroundData::Color([0.1, 0.1, 0.2, 1.0]),
                )));

                // Load the cube asset
                self.cube_asset_id = self.alloc_id("asset");
                commands.push(Command::Asset(AssetCommand::Load {
                    asset_id: self.cube_asset_id.clone(),
                    path: "cube.glb".to_string(),
                }));

                self.initialized = true;
            }

            LifecycleEvent::Frame(frame) => {
                // Update rotation manually if not using animation
                self.rotation += frame.dt;
            }

            LifecycleEvent::Resize(_) => {
                // Shell handles resize, core can react if needed
            }

            LifecycleEvent::Pause => {
                commands.push(self.log(LogLevel::Info, "App paused"));
            }

            LifecycleEvent::Resume => {
                commands.push(self.log(LogLevel::Info, "App resumed"));
            }

            LifecycleEvent::Shutdown => {
                commands.push(self.log(LogLevel::Info, "App shutting down"));
            }
        }

        commands
    }
}

impl InputHandler for ExampleCore {
    fn handle_input(&mut self, event: InputEvent) -> Vec<Command> {
        let mut commands = Vec::new();

        match event {
            InputEvent::Keyboard(KeyboardEvent::KeyDown(key)) => {
                match key.key.as_str() {
                    "v" | "V" => {
                        // Toggle VR
                        if self.xr_active {
                            commands.push(Command::Xr(XrCommand::Exit));
                        } else {
                            commands.push(Command::Xr(XrCommand::Enter {
                                mode: XrMode::ImmersiveVr,
                            }));
                        }
                    }
                    "r" | "R" => {
                        // Reset cube position
                        if !self.cube_volume_id.is_empty() {
                            commands.push(Command::Scene(SceneCommand::SetTransform(
                                SetTransformData {
                                    volume_id: self.cube_volume_id.clone(),
                                    transform: Transform {
                                        position: [0.0, 1.0, -2.0],
                                        rotation: [0.0, 0.0, 0.0, 1.0],
                                        scale: [0.5, 0.5, 0.5],
                                    },
                                    animate: None,
                                },
                            )));
                        }
                    }
                    _ => {}
                }
            }

            InputEvent::Gamepad(GamepadEvent::Input(input)) => {
                // Move cube with gamepad
                if !self.cube_volume_id.is_empty() && input.axes.len() >= 2 {
                    let dx = input.axes[0];
                    let dy = input.axes[1];
                    if dx.abs() > 0.1 || dy.abs() > 0.1 {
                        // Could update position based on input
                    }
                }
            }

            _ => {}
        }

        commands
    }
}

impl XrHandler for ExampleCore {
    fn handle_xr(&mut self, event: XrEvent) -> Vec<Command> {
        let mut commands = Vec::new();

        match event {
            XrEvent::SessionChanged(state) => {
                self.xr_active = state == XrSessionState::Active;
                commands.push(self.log(LogLevel::Info, format!("XR session: {:?}", state)));

                // Adjust cube position for XR
                if self.xr_active && !self.cube_volume_id.is_empty() {
                    commands.push(Command::Scene(SceneCommand::SetTransform(SetTransformData {
                        volume_id: self.cube_volume_id.clone(),
                        transform: Transform {
                            position: [0.0, 1.2, -1.5], // Slightly closer in VR
                            rotation: [0.0, 0.0, 0.0, 1.0],
                            scale: [0.3, 0.3, 0.3], // Smaller in VR
                        },
                        animate: Some(AnimateTransform {
                            duration_ms: 500,
                            easing: Easing::EaseInOut,
                        }),
                    })));
                }
            }

            XrEvent::HeadPose(_pose) => {
                // Shell handles head tracking, core can react if needed
            }

            XrEvent::ControllerPose(controller) => {
                // Could spawn objects at controller position, etc.
                if controller.buttons.iter().any(|(_, pressed)| *pressed) {
                    commands.push(self.log(
                        LogLevel::Debug,
                        format!("{:?} controller button pressed", controller.hand),
                    ));
                }
            }

            _ => {}
        }

        commands
    }
}

impl AssetHandler for ExampleCore {
    fn handle_asset(&mut self, event: AssetEvent) -> Vec<Command> {
        let mut commands = Vec::new();

        match event {
            AssetEvent::Loaded(loaded) => {
                if loaded.asset_id == self.cube_asset_id {
                    commands.push(self.log(
                        LogLevel::Info,
                        format!(
                            "Cube loaded: {} meshes, {} animations",
                            loaded.meshes.len(),
                            loaded.animations.len()
                        ),
                    ));

                    // Create the cube volume
                    self.cube_volume_id = self.alloc_id("volume");
                    commands.push(Command::Scene(SceneCommand::CreateVolume(CreateVolumeData {
                        volume_id: self.cube_volume_id.clone(),
                        source: VolumeSource::Asset {
                            asset_id: self.cube_asset_id.clone(),
                            mesh_index: None, // use default/first mesh
                        },
                        transform: Transform {
                            position: [0.0, 1.0, -2.0], // 1m up, 2m in front
                            rotation: [0.0, 0.0, 0.0, 1.0],
                            scale: [0.5, 0.5, 0.5],
                        },
                        material: None,
                    })));

                    // Start rotating animation if the asset has one
                    if !loaded.animations.is_empty() {
                        commands.push(Command::Animation(AnimationCommand::Play(
                            PlayAnimationData {
                                volume_id: self.cube_volume_id.clone(),
                                animation_id: self.alloc_id("anim"),
                                animation_name: loaded.animations[0].name.clone(),
                                speed: 1.0,
                                loop_mode: LoopMode::Loop,
                                weight: 1.0,
                                start_time: 0.0,
                            },
                        )));
                    }
                }
            }

            AssetEvent::LoadFailed { asset_id, error } => {
                commands.push(self.log(
                    LogLevel::Error,
                    format!("Failed to load asset {}: {}", asset_id, error),
                ));
            }

            _ => {}
        }

        commands
    }
}

impl SceneHandler for ExampleCore {
    fn handle_scene(&mut self, event: SceneEvent) -> Vec<Command> {
        let mut commands = Vec::new();

        match event {
            SceneEvent::VolumeReady { volume_id } => {
                commands.push(self.log(LogLevel::Debug, format!("Volume ready: {}", volume_id)));
            }

            SceneEvent::VolumeAnimationComplete {
                volume_id,
                animation_id,
            } => {
                commands.push(self.log(
                    LogLevel::Debug,
                    format!(
                        "Animation {} completed on volume {}",
                        animation_id, volume_id
                    ),
                ));
            }

            _ => {}
        }

        commands
    }
}

impl Default for ExampleCore {
    fn default() -> Self {
        Self::new()
    }
}
