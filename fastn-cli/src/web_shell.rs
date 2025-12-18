//! Embedded web shell files
//!
//! These files are included at compile time from fastn-shell-web/

/// Shell common JavaScript (shared between WebGPU and WebGL+XR)
pub const SHELL_COMMON_JS: &str = include_str!("../../fastn-shell-web/shell-common.js");

/// WebGPU shell JavaScript
pub const SHELL_WEBGPU_JS: &str = include_str!("../../fastn-shell-web/shell-webgpu.js");

/// WebGL+XR shell JavaScript
pub const SHELL_WEBGL_XR_JS: &str = include_str!("../../fastn-shell-web/shell-webgl-xr.js");

/// HTML template with placeholders:
/// - {{APP_NAME}} - Application name
/// - {{WASM_FILE}} - WASM file path (e.g., "./app-abc123.wasm")
pub const INDEX_HTML_TEMPLATE: &str = r#"<!DOCTYPE html>
<html>
<head>
    <meta charset="utf-8">
    <meta name="viewport" content="width=device-width, initial-scale=1">
    <title>{{APP_NAME}} - fastn</title>
    <style>
        * { margin: 0; padding: 0; box-sizing: border-box; }
        html, body {
            width: 100%;
            height: 100%;
            overflow: hidden;
            background: #1a1a2e;
        }
        canvas {
            display: block;
            width: 100%;
            height: 100%;
            background: #16213e;
        }
        #error {
            position: absolute;
            top: 50%;
            left: 50%;
            transform: translate(-50%, -50%);
            color: #ff6b6b;
            font-family: monospace;
            padding: 20px;
            max-width: 600px;
            text-align: center;
        }
        #loading {
            position: absolute;
            top: 50%;
            left: 50%;
            transform: translate(-50%, -50%);
            color: #888;
            font-family: monospace;
            font-size: 14px;
        }
    </style>
</head>
<body>
    <canvas id="canvas" data-wasm="{{WASM_FILE}}"></canvas>
    <div id="error"></div>
    <div id="loading">Detecting platform...</div>

    <!-- Load shared modules first -->
    <script src="shell-common.js"></script>

    <!-- Platform detection and shell loading -->
    <script>
        async function main() {
            const loadingDiv = document.getElementById('loading');
            const errorDiv = document.getElementById('error');

            try {
                // Get WASM path from canvas data attribute or URL params
                const canvas = document.getElementById('canvas');
                const params = new URLSearchParams(window.location.search);
                const wasmPath = params.get('app') || canvas.dataset.wasm || './app.wasm';

                // Detect platform
                const platform = await detectPlatform();
                console.log('Detected platform:', platform);
                loadingDiv.textContent = `Loading ${platform} shell...`;

                if (platform === 'webgl-xr') {
                    // Load WebGL+XR shell for VR headsets
                    await loadScript('shell-webgl-xr.js');
                    loadingDiv.style.display = 'none';
                    await initWebGLXR(wasmPath);
                } else {
                    // Load WebGPU shell for desktop/laptop
                    await loadScript('shell-webgpu.js');
                    loadingDiv.style.display = 'none';
                    await initWebGPU(wasmPath);
                }
            } catch (e) {
                console.error('Failed to initialize:', e);
                loadingDiv.style.display = 'none';
                errorDiv.textContent = e.message;
            }
        }

        function loadScript(src) {
            return new Promise((resolve, reject) => {
                const script = document.createElement('script');
                script.src = src;
                script.onload = resolve;
                script.onerror = () => reject(new Error(`Failed to load ${src}`));
                document.body.appendChild(script);
            });
        }

        main();
    </script>
</body>
</html>
"#;
