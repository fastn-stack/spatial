//! Simplified API for common use cases
//!
//! This module provides high-level functions for creating spatial applications
//! without having to manually implement the `Core` trait.
//!
//! # Example
//!
//! ```rust,ignore
//! use fastn::{fastn_app, simple::RenderGlbApp};
//!
//! // Simple app that just renders a GLB file
//! fastn_app!(RenderGlbApp, RenderGlbApp::new("cube.glb"));
//! ```
//!
//! For more control, use the builder pattern:
//!
//! ```rust,ignore
//! use fastn::{fastn_app, simple::SimpleApp};
//!
//! fastn_app!(SimpleApp, SimpleApp::builder()
//!     .glb("cube.glb")
//!     .position(0.0, 1.0, -2.0)
//!     .scale(0.5)
//!     .background_color([0.1, 0.1, 0.2, 1.0])
//!     .build());
//! ```

use crate::{
    Command, Core, Event,
    LifecycleEvent, AssetEvent,
    AssetCommand, SceneCommand, EnvironmentCommand, DebugCommand,
    CreateVolumeData, Transform, VolumeSource, BackgroundData, LogLevel,
};

/// A simple app that renders a single GLB file
pub struct RenderGlbApp {
    glb_path: String,
    position: [f32; 3],
    scale: f32,
    background: [f32; 4],
    asset_loaded: bool,
}

impl RenderGlbApp {
    /// Create a new app that renders the given GLB file
    pub fn new(glb_path: impl Into<String>) -> Self {
        Self {
            glb_path: glb_path.into(),
            position: [0.0, 1.0, -2.0],
            scale: 0.5,
            background: [0.1, 0.1, 0.2, 1.0],
            asset_loaded: false,
        }
    }

    /// Set the position of the rendered object
    pub fn position(mut self, x: f32, y: f32, z: f32) -> Self {
        self.position = [x, y, z];
        self
    }

    /// Set the scale of the rendered object
    pub fn scale(mut self, scale: f32) -> Self {
        self.scale = scale;
        self
    }

    /// Set the background color
    pub fn background(mut self, color: [f32; 4]) -> Self {
        self.background = color;
        self
    }
}

impl Core for RenderGlbApp {
    fn handle(&mut self, event: Event) -> Vec<Command> {
        match event {
            Event::Lifecycle(LifecycleEvent::Init(_)) => {
                vec![
                    Command::Debug(DebugCommand::Log {
                        level: LogLevel::Info,
                        message: format!("Rendering: {}", self.glb_path),
                    }),
                    Command::Environment(EnvironmentCommand::SetBackground(
                        BackgroundData::Color(self.background),
                    )),
                    Command::Asset(AssetCommand::Load {
                        asset_id: "main".to_string(),
                        path: self.glb_path.clone(),
                    }),
                ]
            }

            Event::Asset(AssetEvent::Loaded(loaded)) if loaded.asset_id == "main" => {
                self.asset_loaded = true;
                vec![
                    Command::Scene(SceneCommand::CreateVolume(CreateVolumeData {
                        volume_id: "main-volume".to_string(),
                        source: VolumeSource::Asset {
                            asset_id: "main".to_string(),
                            mesh_index: None,
                        },
                        transform: Transform {
                            position: self.position,
                            rotation: [0.0, 0.0, 0.0, 1.0],
                            scale: [self.scale, self.scale, self.scale],
                        },
                        material: None,
                    })),
                ]
            }

            Event::Asset(AssetEvent::LoadFailed { asset_id, error }) if asset_id == "main" => {
                vec![
                    Command::Debug(DebugCommand::Log {
                        level: LogLevel::Error,
                        message: format!("Failed to load {}: {}", self.glb_path, error),
                    }),
                ]
            }

            _ => vec![],
        }
    }
}

/// Builder for creating simple apps with more options
pub struct SimpleAppBuilder {
    glb_path: Option<String>,
    position: [f32; 3],
    scale: f32,
    background: [f32; 4],
}

impl SimpleAppBuilder {
    /// Create a new builder
    pub fn new() -> Self {
        Self {
            glb_path: None,
            position: [0.0, 1.0, -2.0],
            scale: 0.5,
            background: [0.1, 0.1, 0.2, 1.0],
        }
    }

    /// Set the GLB file to render
    pub fn glb(mut self, path: impl Into<String>) -> Self {
        self.glb_path = Some(path.into());
        self
    }

    /// Set the position
    pub fn position(mut self, x: f32, y: f32, z: f32) -> Self {
        self.position = [x, y, z];
        self
    }

    /// Set the uniform scale
    pub fn scale(mut self, scale: f32) -> Self {
        self.scale = scale;
        self
    }

    /// Set the background color
    pub fn background_color(mut self, color: [f32; 4]) -> Self {
        self.background = color;
        self
    }

    /// Build the SimpleApp
    pub fn build(self) -> SimpleApp {
        SimpleApp {
            glb_path: self.glb_path,
            position: self.position,
            scale: self.scale,
            background: self.background,
            asset_loaded: false,
        }
    }
}

impl Default for SimpleAppBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// A simple app built with the builder pattern
pub struct SimpleApp {
    glb_path: Option<String>,
    position: [f32; 3],
    scale: f32,
    background: [f32; 4],
    asset_loaded: bool,
}

impl SimpleApp {
    /// Create a builder for a SimpleApp
    pub fn builder() -> SimpleAppBuilder {
        SimpleAppBuilder::new()
    }
}

impl Core for SimpleApp {
    fn handle(&mut self, event: Event) -> Vec<Command> {
        match event {
            Event::Lifecycle(LifecycleEvent::Init(_)) => {
                let mut commands = vec![
                    Command::Environment(EnvironmentCommand::SetBackground(
                        BackgroundData::Color(self.background),
                    )),
                ];

                if let Some(path) = &self.glb_path {
                    commands.push(Command::Debug(DebugCommand::Log {
                        level: LogLevel::Info,
                        message: format!("Loading: {}", path),
                    }));
                    commands.push(Command::Asset(AssetCommand::Load {
                        asset_id: "main".to_string(),
                        path: path.clone(),
                    }));
                }

                commands
            }

            Event::Asset(AssetEvent::Loaded(loaded)) if loaded.asset_id == "main" => {
                self.asset_loaded = true;
                vec![
                    Command::Scene(SceneCommand::CreateVolume(CreateVolumeData {
                        volume_id: "main-volume".to_string(),
                        source: VolumeSource::Asset {
                            asset_id: "main".to_string(),
                            mesh_index: None,
                        },
                        transform: Transform {
                            position: self.position,
                            rotation: [0.0, 0.0, 0.0, 1.0],
                            scale: [self.scale, self.scale, self.scale],
                        },
                        material: None,
                    })),
                ]
            }

            _ => vec![],
        }
    }
}

/// Convenience function to create a simple app that renders a GLB file
///
/// This is the simplest way to create a fastn app:
///
/// ```rust,ignore
/// use fastn::{fastn_app, simple::render_glb};
///
/// fastn_app!(fastn::simple::RenderGlbApp, render_glb("cube.glb"));
/// ```
pub fn render_glb(path: impl Into<String>) -> RenderGlbApp {
    RenderGlbApp::new(path)
}
