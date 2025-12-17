//! Cube Example
//!
//! Build: cargo build -p cube --target wasm32-unknown-unknown --release
//! Run:   fastn-shell ./target/wasm32-unknown-unknown/release/cube.wasm

#[fastn::app]
fn init() -> fastn::App {
    let mut app = fastn::init();
    app.add_volume_from_glb("cube.glb", 0);
    app
}
