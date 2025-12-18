//! Cube GLB Example - Loading a 3D model from a GLB file
//!
//! This example demonstrates loading a cube mesh from a GLB file
//! using the Entity::load() API.

use fastn::{Entity, RealityViewContent};

/// Build the 3D scene by loading a cube from a GLB file.
///
/// Equivalent Swift (visionOS):
/// ```swift
/// RealityView { content in
///     let cube = try await Entity.load(named: "cube.glb")
///     cube.position = [0, 0, -2]
///     cube.scale = [0.5, 0.5, 0.5]
///     content.add(cube)
/// }
/// ```
#[fastn::app]
fn make_content(content: &mut RealityViewContent) {
    // Load cube from GLB file
    let cube = Entity::load("cube.glb")
        .position(0.0, 0.0, -2.0)
        .scale(0.5);

    content.add(cube);
}
