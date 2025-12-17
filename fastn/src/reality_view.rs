//! RealityView - Container for 3D content
//!
//! Matches SwiftUI's RealityView for visionOS.
//!
//! # Swift Example
//! ```swift
//! RealityView { content in
//!     let box = ModelEntity(mesh: .generateBox(size: 0.5), materials: [material])
//!     content.add(box)
//! }
//! ```
//!
//! # Rust Example
//! ```rust,ignore
//! use fastn::{RealityViewContent, ModelEntity, MeshResource, SimpleMaterial};
//!
//! fn make_content(content: &mut RealityViewContent) {
//!     let cube = ModelEntity::new(
//!         MeshResource::generate_box(0.5),
//!         SimpleMaterial::new().color(1.0, 0.0, 0.0)
//!     );
//!     content.add(cube);
//! }
//! ```

use crate::{Command, EntityKind};

/// Content container for RealityView.
///
/// Equivalent to `RealityViewContent` in SwiftUI/RealityKit.
/// This is what you receive in the `make:` closure of a RealityView.
#[derive(Debug, Default)]
pub struct RealityViewContent {
    pub(crate) entities: Vec<EntityKind>,
}

impl RealityViewContent {
    /// Create new empty content.
    pub fn new() -> Self {
        Self::default()
    }

    /// Add an entity to the scene.
    ///
    /// Equivalent to `content.add(entity)` in RealityKit.
    pub fn add(&mut self, entity: impl Into<EntityKind>) {
        self.entities.push(entity.into());
    }

    /// Convert all entities to commands.
    pub(crate) fn to_commands(&self) -> Vec<Command> {
        let mut commands = Vec::new();
        for entity in &self.entities {
            Self::collect_commands(entity, &mut commands);
        }
        commands
    }

    fn collect_commands(entity: &EntityKind, commands: &mut Vec<Command>) {
        match entity {
            EntityKind::Entity(e) => {
                // Empty entities don't produce commands, but their children do
                for child in e.children() {
                    Self::collect_commands(child, commands);
                }
            }
            EntityKind::ModelEntity(m) => {
                commands.push(m.to_command());
                for child in m.children() {
                    Self::collect_commands(child, commands);
                }
            }
        }
    }
}
