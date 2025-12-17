//! Cube Example
//!
//! Build: cargo build -p cube --target wasm32-unknown-unknown --release
//! Run:   fastn-shell ./target/wasm32-unknown-unknown/release/cube.wasm
//!
//! # Swift equivalent:
//! ```swift
//! RealityView { content in
//!     let box = ModelEntity(
//!         mesh: .generateBox(size: 0.5),
//!         materials: [SimpleMaterial(color: .red, isMetallic: false)]
//!     )
//!     content.add(box)
//! }
//! ```

use fastn::{ModelEntity, MeshResource, SimpleMaterial, RealityViewContent};

#[fastn::app]
fn app(content: &mut RealityViewContent) {
    // Create a red box - matches RealityKit's ModelEntity API
    let cube = ModelEntity::new(
        MeshResource::generate_box(0.5),
        SimpleMaterial::new().color(0.8, 0.2, 0.2)
    );
    content.add(cube);
}
