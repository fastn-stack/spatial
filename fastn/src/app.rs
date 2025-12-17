//! App - Simple declarative API for fastn applications
//!
//! # Usage
//!
//! ```rust,ignore
//! fn init() -> fastn::App {
//!     let mut app = fastn::init();
//!     app.add_volume_from_glb("cube.glb", 0);
//!     app
//! }
//! ```

use crate::{Command, AssetCommand, SceneCommand, CreateVolumeData, Transform, VolumeSource};

/// A fastn application.
///
/// Holds the initial scene configuration (volumes, assets, etc.)
#[derive(Debug, Default)]
pub struct App {
    pub(crate) commands: Vec<Command>,
    volume_counter: u32,
    asset_counter: u32,
}

impl App {
    /// Create a new empty App.
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a volume from a GLB file.
    ///
    /// - `path`: Path to the GLB file (relative to assets folder)
    /// - `mesh_index`: Which mesh to use from the GLB (usually 0)
    pub fn add_volume_from_glb(&mut self, path: &str, mesh_index: u32) -> &mut Self {
        let asset_id = format!("asset-{}", self.asset_counter);
        self.asset_counter += 1;

        let volume_id = format!("volume-{}", self.volume_counter);
        self.volume_counter += 1;

        // Load the asset
        self.commands.push(Command::Asset(AssetCommand::Load {
            asset_id: asset_id.clone(),
            path: path.to_string(),
        }));

        // Create volume from asset (shell will wait for asset to load)
        self.commands.push(Command::Scene(SceneCommand::CreateVolume(CreateVolumeData {
            volume_id,
            source: VolumeSource::Asset {
                asset_id,
                mesh_index: Some(mesh_index),
            },
            transform: Transform::default(),
            material: None,
        })));

        self
    }

    /// Get the commands to send to the shell.
    pub fn commands(&self) -> &[Command] {
        &self.commands
    }
}

/// Create a new fastn App.
///
/// This is the entry point for building your application.
///
/// # Example
///
/// ```rust,ignore
/// fn init() -> fastn::App {
///     let mut app = fastn::init();
///     app.add_volume_from_glb("cube.glb", 0);
///     app
/// }
/// ```
pub fn init() -> App {
    App::new()
}
