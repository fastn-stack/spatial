# fastn - Spatial/XR Application Framework

Build spatial and XR applications in Rust with a RealityKit-inspired API. Write once, run on native (macOS/Windows/Linux) and web (WebGPU/WebXR).

## Quick Start

```rust
// lib.rs
use fastn::{ModelEntity, MeshResource, SimpleMaterial, RealityViewContent};

#[fastn::app]
fn app(content: &mut RealityViewContent) {
    let cube = ModelEntity::new(
        MeshResource::generate_box(0.5),
        SimpleMaterial::new().color(0.8, 0.2, 0.2)
    );
    content.add(cube);
}
```

```rust
// main.rs
fn main() {
    fastn::main();
}
```

## Commands

```bash
cargo run              # Run native shell (default)
cargo run -- build     # Build for web (creates dist/)
cargo run -- serve     # Build and serve web version
```

## Project Structure

```
my-app/
  Cargo.toml
  src/
    lib.rs             # Your app code with #[fastn::app]
    main.rs            # Just calls fastn::main()
  assets/              # Optional: GLB models, textures, etc.
  index.html.tmpl      # Optional: custom HTML template
```

### Cargo.toml

```toml
[package]
name = "my-app"
edition = "2024"

[lib]
crate-type = ["cdylib"]

[[bin]]
name = "my-app"
path = "src/main.rs"

[dependencies]
fastn = { git = "https://github.com/fastn-stack/spatial" }
```

## Custom HTML Template

Create `index.html.tmpl` in your project root to customize the web shell:

```html
<!DOCTYPE html>
<html>
<head>
    <title>{{APP_NAME}}</title>
    <!-- Your custom styles -->
</head>
<body>
    <canvas id="canvas" data-wasm="{{WASM_FILE}}"></canvas>
    <!-- Standard shell scripts -->
    <script src="shell-common.js"></script>
    <script>
        // Platform detection and initialization
    </script>
</body>
</html>
```

Available placeholders:
- `{{APP_NAME}}` - Your crate name
- `{{WASM_FILE}}` - Path to the WASM file (includes hash for cache busting)

## Examples

See the [examples/](examples/) directory:

- **cube** - Simple red cube using programmatic mesh generation
- **cube-glb** - Loading a 3D model from a GLB file

Run an example:
```bash
cargo run -p cube          # Native
cargo run -p cube -- build # Web
```

## Architecture

- **fastn** - Core API crate (RealityKit-like types and #[fastn::app] macro)
- **fastn-cli** - CLI tools embedded in fastn (build/serve/run commands)
- **fastn-shell** - Native runtime (WebGPU + wgpu)
- **fastn-shell-web** - Web runtime (WebGPU or WebGL+WebXR)

## License

MIT OR Apache-2.0
