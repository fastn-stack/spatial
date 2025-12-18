//! fastn - Build Spatial/XR Applications in Rust
//!
//! A visionOS-inspired API for building spatial computing applications.
//!
//! # Quick Start
//!
//! ```rust,ignore
//! use fastn::{ModelEntity, MeshResource, SimpleMaterial, RealityViewContent};
//!
//! #[fastn::app]
//! fn make_content(content: &mut RealityViewContent) {
//!     // Create a red box - equivalent to Swift:
//!     // let box = ModelEntity(mesh: .generateBox(size: 0.5),
//!     //                       materials: [SimpleMaterial(color: .red, isMetallic: false)])
//!     let cube = ModelEntity::new(
//!         MeshResource::generate_box(0.5),
//!         SimpleMaterial::new().color(1.0, 0.0, 0.0)
//!     );
//!     content.add(cube);
//! }
//! ```
//!
//! # visionOS Mapping
//!
//! | visionOS (Swift) | fastn (Rust) |
//! |------------------|--------------|
//! | `ModelEntity` | `ModelEntity` |
//! | `Entity` | `Entity` |
//! | `Entity.load(named:)` | `Entity::load(path)` |
//! | `MeshResource.generateBox(size:)` | `MeshResource::generate_box(size)` |
//! | `SimpleMaterial` | `SimpleMaterial` |
//! | `RealityViewContent` | `RealityViewContent` |
//! | `content.add(entity)` | `content.add(entity)` |

mod camera;
mod entity;
mod material;
mod mesh;
mod reality_view;

#[doc(hidden)]
pub mod wasm_bridge;

// Camera controller for default input handling
pub use camera::CameraController;

// Re-export the proc macro
pub use fastn_macros::app;

// Entity types (like RealityKit)
pub use entity::{Entity, ModelEntity, EntityKind, LoadedEntity};

// Mesh generation (like MeshResource)
pub use mesh::MeshResource;

// Materials (like SimpleMaterial)
pub use material::SimpleMaterial;

// RealityView content
pub use reality_view::RealityViewContent;

// Protocol types for advanced usage
pub use fastn_protocol::*;

/// Create a new RealityViewContent.
///
/// This is the entry point for building your 3D scene.
pub fn content() -> RealityViewContent {
    RealityViewContent::new()
}

// Re-export CLI main function for native targets
#[cfg(all(not(target_arch = "wasm32"), feature = "cli"))]
pub use fastn_cli::main;
