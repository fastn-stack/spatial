//! Entity - Base class for all things in a RealityKit scene
//!
//! Matches RealityKit's Entity hierarchy:
//! - Entity: Base class with transform, can have children
//! - ModelEntity: Entity with mesh and materials
//!
//! # Example
//!
//! ```rust,ignore
//! use fastn::{Entity, ModelEntity, MeshResource, SimpleMaterial};
//!
//! // Empty entity (container/group)
//! let parent = Entity::new();
//!
//! // Model entity with mesh and material
//! let cube = ModelEntity::new(
//!     MeshResource::generate_box(0.5),
//!     SimpleMaterial::new().color(1.0, 0.0, 0.0)
//! );
//! cube.set_position([0.0, 1.0, -2.0]);
//! parent.add_child(cube);
//! ```

use crate::{MeshResource, SimpleMaterial};
use crate::{Command, SceneCommand, CreateVolumeData, Transform, VolumeSource, Primitive};

/// Base entity - a node in the scene hierarchy.
///
/// Equivalent to RealityKit's `Entity`.
#[derive(Debug, Clone)]
pub struct Entity {
    id: String,
    position: [f32; 3],
    orientation: [f32; 4],  // Quaternion
    scale: [f32; 3],
    children: Vec<EntityKind>,
}

/// Different kinds of entities.
#[derive(Debug, Clone)]
pub enum EntityKind {
    Entity(Entity),
    ModelEntity(ModelEntity),
}

impl Entity {
    /// Create a new empty entity.
    ///
    /// Equivalent to `Entity()` in RealityKit.
    pub fn new() -> Self {
        Self::with_id(generate_id())
    }

    /// Create a new entity with a specific ID.
    pub fn with_id(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            position: [0.0, 0.0, 0.0],
            orientation: [0.0, 0.0, 0.0, 1.0],
            scale: [1.0, 1.0, 1.0],
            children: Vec::new(),
        }
    }

    /// Get the entity's ID.
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Set the position in parent's coordinate space.
    ///
    /// Equivalent to `entity.position = SIMD3<Float>(x, y, z)` in RealityKit.
    pub fn set_position(&mut self, position: [f32; 3]) {
        self.position = position;
    }

    /// Set position with individual components.
    pub fn position(mut self, x: f32, y: f32, z: f32) -> Self {
        self.position = [x, y, z];
        self
    }

    /// Set the orientation as a quaternion.
    ///
    /// Equivalent to `entity.orientation` in RealityKit.
    pub fn set_orientation(&mut self, orientation: [f32; 4]) {
        self.orientation = orientation;
    }

    /// Set the scale.
    ///
    /// Equivalent to `entity.scale = SIMD3<Float>(x, y, z)` in RealityKit.
    pub fn set_scale(&mut self, scale: [f32; 3]) {
        self.scale = scale;
    }

    /// Set uniform scale.
    pub fn scale(mut self, s: f32) -> Self {
        self.scale = [s, s, s];
        self
    }

    /// Add a child entity.
    ///
    /// Equivalent to `entity.addChild(child)` in RealityKit.
    pub fn add_child(&mut self, child: impl Into<EntityKind>) {
        self.children.push(child.into());
    }

    /// Get children.
    pub fn children(&self) -> &[EntityKind] {
        &self.children
    }
}

impl Default for Entity {
    fn default() -> Self {
        Self::new()
    }
}

/// Model entity - an entity with a 3D mesh and materials.
///
/// Equivalent to RealityKit's `ModelEntity`.
///
/// # Example (Swift)
/// ```swift
/// let box = ModelEntity(
///     mesh: .generateBox(size: 0.5),
///     materials: [SimpleMaterial(color: .red, isMetallic: false)]
/// )
/// ```
///
/// # Example (Rust)
/// ```rust,ignore
/// let box_entity = ModelEntity::new(
///     MeshResource::generate_box(0.5),
///     SimpleMaterial::new().color(1.0, 0.0, 0.0)
/// );
/// ```
#[derive(Debug, Clone)]
pub struct ModelEntity {
    id: String,
    mesh: MeshResource,
    material: SimpleMaterial,
    position: [f32; 3],
    orientation: [f32; 4],
    scale: [f32; 3],
    children: Vec<EntityKind>,
}

impl ModelEntity {
    /// Create a new model entity with mesh and material.
    ///
    /// Equivalent to `ModelEntity(mesh:materials:)` in RealityKit.
    pub fn new(mesh: MeshResource, material: SimpleMaterial) -> Self {
        Self {
            id: generate_id(),
            mesh,
            material,
            position: [0.0, 0.0, 0.0],
            orientation: [0.0, 0.0, 0.0, 1.0],
            scale: [1.0, 1.0, 1.0],
            children: Vec::new(),
        }
    }

    /// Create a model entity with a specific ID.
    pub fn with_id(id: impl Into<String>, mesh: MeshResource, material: SimpleMaterial) -> Self {
        Self {
            id: id.into(),
            mesh,
            material,
            position: [0.0, 0.0, 0.0],
            orientation: [0.0, 0.0, 0.0, 1.0],
            scale: [1.0, 1.0, 1.0],
            children: Vec::new(),
        }
    }

    /// Get the entity's ID.
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Set the position in parent's coordinate space.
    pub fn set_position(&mut self, position: [f32; 3]) {
        self.position = position;
    }

    /// Set position with individual components (builder style).
    pub fn position(mut self, x: f32, y: f32, z: f32) -> Self {
        self.position = [x, y, z];
        self
    }

    /// Set the orientation as a quaternion.
    pub fn set_orientation(&mut self, orientation: [f32; 4]) {
        self.orientation = orientation;
    }

    /// Set the scale.
    pub fn set_scale(&mut self, scale: [f32; 3]) {
        self.scale = scale;
    }

    /// Set uniform scale (builder style).
    pub fn scale(mut self, s: f32) -> Self {
        self.scale = [s, s, s];
        self
    }

    /// Add a child entity.
    pub fn add_child(&mut self, child: impl Into<EntityKind>) {
        self.children.push(child.into());
    }

    /// Get children.
    pub fn children(&self) -> &[EntityKind] {
        &self.children
    }

    /// Convert to a CreateVolumeData command.
    pub(crate) fn to_command(&self) -> Command {
        let primitive = match &self.mesh {
            MeshResource::Box { size } => Primitive::Cube { size: *size },
            MeshResource::BoxWithDimensions { width, height, depth } => {
                Primitive::Box { width: *width, height: *height, depth: *depth }
            }
            MeshResource::Sphere { radius } => Primitive::Sphere { radius: *radius, segments: 32 },
            MeshResource::Plane { width, depth } => Primitive::Plane { width: *width, height: *depth },
            MeshResource::Cylinder { radius, height } => {
                Primitive::Cylinder { radius: *radius, height: *height, segments: 32 }
            }
        };

        Command::Scene(SceneCommand::CreateVolume(CreateVolumeData {
            volume_id: self.id.clone(),
            source: VolumeSource::Primitive(primitive),
            transform: Transform {
                position: self.position,
                rotation: self.orientation,
                scale: self.scale,
            },
            material: Some(self.material.to_override()),
        }))
    }
}

// Conversions to EntityKind
impl From<Entity> for EntityKind {
    fn from(e: Entity) -> Self {
        EntityKind::Entity(e)
    }
}

impl From<ModelEntity> for EntityKind {
    fn from(e: ModelEntity) -> Self {
        EntityKind::ModelEntity(e)
    }
}

// Simple ID generation (in real impl, use UUID)
fn generate_id() -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    format!("entity-{}", COUNTER.fetch_add(1, Ordering::Relaxed))
}
