//! MeshResource - Geometry generation for procedural primitives
//!
//! Matches RealityKit's MeshResource API for procedural geometry.
//!
//! # Example
//!
//! ```rust,ignore
//! use fastn::MeshResource;
//!
//! // Procedural geometry
//! let box_mesh = MeshResource::generate_box(0.5);
//! let sphere_mesh = MeshResource::generate_sphere(0.3);
//! let plane_mesh = MeshResource::generate_plane(1.0, 1.0);
//! ```
//!
//! For loading meshes from files (GLB, USDZ), use `Entity::load()` instead.

/// Mesh geometry resource for procedural primitives.
///
/// Equivalent to RealityKit's `MeshResource` for generated geometry.
///
/// For loading 3D models from files, use `Entity::load()` which returns
/// a `LoadedEntity` that handles async asset loading.
#[derive(Debug, Clone)]
pub enum MeshResource {
    Box { size: f32 },
    BoxWithDimensions { width: f32, height: f32, depth: f32 },
    Sphere { radius: f32 },
    Plane { width: f32, depth: f32 },
    Cylinder { radius: f32, height: f32 },
}

impl MeshResource {
    /// Generate a box mesh with uniform size.
    ///
    /// Equivalent to `MeshResource.generateBox(size:)` in RealityKit.
    pub fn generate_box(size: f32) -> Self {
        MeshResource::Box { size }
    }

    /// Generate a box mesh with specific dimensions.
    ///
    /// Equivalent to `MeshResource.generateBox(width:height:depth:)` in RealityKit.
    pub fn generate_box_with_dimensions(width: f32, height: f32, depth: f32) -> Self {
        MeshResource::BoxWithDimensions { width, height, depth }
    }

    /// Generate a sphere mesh.
    ///
    /// Equivalent to `MeshResource.generateSphere(radius:)` in RealityKit.
    pub fn generate_sphere(radius: f32) -> Self {
        MeshResource::Sphere { radius }
    }

    /// Generate a plane mesh.
    ///
    /// Equivalent to `MeshResource.generatePlane(width:depth:)` in RealityKit.
    pub fn generate_plane(width: f32, depth: f32) -> Self {
        MeshResource::Plane { width, depth }
    }

    /// Generate a cylinder mesh.
    ///
    /// Equivalent to `MeshResource.generateCylinder(radius:height:)` in RealityKit.
    pub fn generate_cylinder(radius: f32, height: f32) -> Self {
        MeshResource::Cylinder { radius, height }
    }
}
