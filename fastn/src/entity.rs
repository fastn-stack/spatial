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
//!
//! // Load entity from file (GLB, USDZ)
//! let robot = Entity::load("robot.glb")
//!     .position(0.0, 0.0, -2.0)
//!     .scale(0.5);
//! ```

use crate::{MeshResource, SimpleMaterial};
use crate::{Command, SceneCommand, CreateVolumeData, AssetCommand, Transform, VolumeSource, Primitive};

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
    LoadedEntity(LoadedEntity),
}

impl Entity {
    /// Create a new empty entity.
    ///
    /// Equivalent to `Entity()` in RealityKit.
    pub fn new() -> Self {
        Self::with_id(generate_id())
    }

    /// Load an entity from a 3D model file.
    ///
    /// Supported formats: GLB, glTF, USDZ, USD
    ///
    /// Equivalent to `Entity.load(named:)` in RealityKit.
    ///
    /// # Example
    /// ```rust,ignore
    /// // Swift equivalent:
    /// // let robot = try await Entity.load(named: "robot.usdz")
    /// // robot.position = [0, 0, -2]
    ///
    /// let robot = Entity::load("robot.glb")
    ///     .position(0.0, 0.0, -2.0)
    ///     .scale(0.5);
    /// content.add(robot);
    /// ```
    pub fn load(path: impl Into<String>) -> LoadedEntity {
        LoadedEntity::new(path)
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

/// Entity loaded from a 3D model file.
///
/// Created via `Entity::load()`. The shell handles async loading.
///
/// Equivalent to entities loaded via `Entity.load(named:)` in RealityKit.
///
/// # Example
/// ```rust,ignore
/// let robot = Entity::load("robot.glb")
///     .position(0.0, 0.0, -2.0)
///     .scale(0.5)
///     .with_material(SimpleMaterial::new().color(0.2, 0.8, 0.2));
/// content.add(robot);
/// ```
#[derive(Debug, Clone)]
pub struct LoadedEntity {
    id: String,
    asset_id: String,
    path: String,
    mesh_index: Option<u32>,
    position: [f32; 3],
    orientation: [f32; 4],
    scale: [f32; 3],
    material_override: Option<SimpleMaterial>,
    children: Vec<EntityKind>,
}

impl LoadedEntity {
    /// Create a new loaded entity from a file path.
    pub fn new(path: impl Into<String>) -> Self {
        let path = path.into();
        let id = generate_id();
        // Asset ID is derived from path for deduplication
        let asset_id = format!("asset:{}", path);
        Self {
            id,
            asset_id,
            path,
            mesh_index: None,
            position: [0.0, 0.0, 0.0],
            orientation: [0.0, 0.0, 0.0, 1.0],
            scale: [1.0, 1.0, 1.0],
            material_override: None,
            children: Vec::new(),
        }
    }

    /// Get the entity's ID.
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Get the asset path.
    pub fn path(&self) -> &str {
        &self.path
    }

    /// Get the asset ID (used for deduplication).
    pub fn asset_id(&self) -> &str {
        &self.asset_id
    }

    /// Load a specific mesh from a multi-mesh file.
    pub fn mesh(mut self, index: u32) -> Self {
        self.mesh_index = Some(index);
        self
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

    /// Override the material from the file.
    pub fn with_material(mut self, material: SimpleMaterial) -> Self {
        self.material_override = Some(material);
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

    /// Generate the asset load command.
    pub(crate) fn to_load_command(&self) -> Command {
        Command::Asset(AssetCommand::Load {
            asset_id: self.asset_id.clone(),
            path: self.path.clone(),
        })
    }

    /// Generate the create volume command.
    pub(crate) fn to_create_command(&self) -> Command {
        Command::Scene(SceneCommand::CreateVolume(CreateVolumeData {
            volume_id: self.id.clone(),
            source: VolumeSource::Asset {
                asset_id: self.asset_id.clone(),
                mesh_index: self.mesh_index,
            },
            transform: Transform {
                position: self.position,
                rotation: self.orientation,
                scale: self.scale,
            },
            material: self.material_override.as_ref().map(|m| m.to_override()),
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

impl From<LoadedEntity> for EntityKind {
    fn from(e: LoadedEntity) -> Self {
        EntityKind::LoadedEntity(e)
    }
}

// Simple ID generation (in real impl, use UUID)
fn generate_id() -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    format!("entity-{}", COUNTER.fetch_add(1, Ordering::Relaxed))
}
