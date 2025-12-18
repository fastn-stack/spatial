// fastn-shell-web - JavaScript WebGPU shell for fastn apps
// Loads and runs WASM apps compiled with fastn framework

const WASM_PATH = './cube.wasm';  // Default app to load

class FastnShell {
    constructor(canvas) {
        this.canvas = canvas;
        this.device = null;
        this.context = null;
        this.pipeline = null;
        this.volumes = new Map();
        this.uniformBuffer = null;
        this.uniformBindGroup = null;
        this.depthTexture = null;
        this.vertexBuffer = null;
        this.indexBuffer = null;

        // WASM state
        this.wasm = null;
        this.appPtr = null;

        // Camera state (updated by commands from core)
        this.camera = {
            position: [0, 1.6, 3],
            target: [0, 0, 0],
            up: [0, 1, 0],
            fov: Math.PI / 4,
            near: 0.1,
            far: 100.0,
        };

        // Input state
        this.pressedKeys = new Set();
        this.lastFrameTime = performance.now();
    }

    async init() {
        // Check WebGPU support
        if (!navigator.gpu) {
            throw new Error('WebGPU not supported. Try Chrome/Edge 113+ or enable WebGPU flags.');
        }

        const adapter = await navigator.gpu.requestAdapter();
        if (!adapter) {
            throw new Error('Failed to get GPU adapter');
        }

        this.device = await adapter.requestDevice();
        this.context = this.canvas.getContext('webgpu');

        this.format = navigator.gpu.getPreferredCanvasFormat();

        // Set initial canvas size
        this.resizeCanvas();

        await this.createPipeline(this.format);
        this.createDepthTexture();
        this.createCubeGeometry();
        this.setupInputHandlers();
        this.setupResizeHandler();
    }

    resizeCanvas() {
        const width = window.innerWidth;
        const height = window.innerHeight;

        // Update canvas buffer size
        this.canvas.width = width;
        this.canvas.height = height;

        // Reconfigure context with new size
        if (this.context && this.device) {
            this.context.configure({
                device: this.device,
                format: this.format,
                alphaMode: 'premultiplied',
            });
        }
    }

    setupResizeHandler() {
        window.addEventListener('resize', () => {
            this.resizeCanvas();

            // Recreate depth texture with new size
            if (this.depthTexture) {
                this.depthTexture.destroy();
            }
            this.createDepthTexture();
        });
    }

    setupInputHandlers() {
        // Keyboard events
        window.addEventListener('keydown', (e) => {
            if (!this.pressedKeys.has(e.code)) {
                this.pressedKeys.add(e.code);
                this.sendKeyEvent('KeyDown', e.code, e.key);
            }
        });

        window.addEventListener('keyup', (e) => {
            this.pressedKeys.delete(e.code);
            this.sendKeyEvent('KeyUp', e.code, e.key);
        });

        // Focus canvas for keyboard events
        this.canvas.tabIndex = 0;
        this.canvas.focus();
    }

    sendKeyEvent(type, code, key) {
        if (!this.wasm || this.appPtr === null) return;

        // Match Rust protocol: Event uses tag="category" content="event"
        // InputEvent uses tag="type", KeyboardEvent uses tag="action"
        const event = {
            category: "Input",
            event: {
                type: "Keyboard",
                action: type,
                device_id: "keyboard-0",
                key: key,
                code: code,
                shift: false,
                ctrl: false,
                alt: false,
                meta: false,
                repeat: false
            }
        };

        const commands = this.sendEvent(event);
        this.processCommands(commands);
    }

    sendFrameEvent(dt) {
        if (!this.wasm || this.appPtr === null) return;

        this.frameNumber = (this.frameNumber || 0) + 1;

        // Match Rust protocol: Event uses tag="category" content="event"
        // LifecycleEvent uses tag="type"
        const event = {
            category: "Lifecycle",
            event: {
                type: "Frame",
                time: performance.now() / 1000.0,
                dt: dt,
                frame: this.frameNumber
            }
        };

        const commands = this.sendEvent(event);
        this.processCommands(commands);
    }

    sendEvent(event) {
        const eventJson = JSON.stringify(event);
        const eventBytes = new TextEncoder().encode(eventJson);

        // Allocate memory in WASM for the event
        const eventPtr = this.wasm.alloc(eventBytes.length);

        // Write event to WASM memory
        const memory = new Uint8Array(this.wasm.memory.buffer);
        memory.set(eventBytes, eventPtr);

        // Call on_event
        this.wasm.on_event(this.appPtr, eventPtr, eventBytes.length);

        // Read result
        const resultPtr = this.wasm.get_result_ptr(this.appPtr);
        const resultLen = this.wasm.get_result_len(this.appPtr);

        if (resultLen === 0) return [];

        const resultBytes = new Uint8Array(this.wasm.memory.buffer, resultPtr, resultLen);
        const resultJson = new TextDecoder().decode(resultBytes);

        try {
            return JSON.parse(resultJson);
        } catch (e) {
            console.error('Failed to parse event result:', e);
            return [];
        }
    }

    async createPipeline(format) {
        const shaderCode = `
            struct Uniforms {
                mvp: mat4x4<f32>,
                color: vec4<f32>,
            };

            @group(0) @binding(0)
            var<uniform> uniforms: Uniforms;

            struct VertexInput {
                @location(0) position: vec3<f32>,
                @location(1) normal: vec3<f32>,
            };

            struct VertexOutput {
                @builtin(position) clip_position: vec4<f32>,
                @location(0) normal: vec3<f32>,
            };

            @vertex
            fn vs_main(in: VertexInput) -> VertexOutput {
                var out: VertexOutput;
                out.clip_position = uniforms.mvp * vec4<f32>(in.position, 1.0);
                out.normal = in.normal;
                return out;
            }

            @fragment
            fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
                let light_dir = normalize(vec3<f32>(0.5, 1.0, 0.3));
                let ambient = 0.3;
                let diffuse = max(dot(normalize(in.normal), light_dir), 0.0);
                let brightness = ambient + diffuse * 0.7;
                return vec4<f32>(uniforms.color.rgb * brightness, uniforms.color.a);
            }
        `;

        const shaderModule = this.device.createShaderModule({ code: shaderCode });

        // Uniform buffer for MVP matrix + color
        this.uniformBuffer = this.device.createBuffer({
            size: 64 + 16, // mat4x4 + vec4
            usage: GPUBufferUsage.UNIFORM | GPUBufferUsage.COPY_DST,
        });

        const bindGroupLayout = this.device.createBindGroupLayout({
            entries: [{
                binding: 0,
                visibility: GPUShaderStage.VERTEX | GPUShaderStage.FRAGMENT,
                buffer: { type: 'uniform' },
            }],
        });

        this.uniformBindGroup = this.device.createBindGroup({
            layout: bindGroupLayout,
            entries: [{
                binding: 0,
                resource: { buffer: this.uniformBuffer },
            }],
        });

        const pipelineLayout = this.device.createPipelineLayout({
            bindGroupLayouts: [bindGroupLayout],
        });

        this.pipeline = this.device.createRenderPipeline({
            layout: pipelineLayout,
            vertex: {
                module: shaderModule,
                entryPoint: 'vs_main',
                buffers: [{
                    arrayStride: 24, // 6 floats * 4 bytes
                    attributes: [
                        { shaderLocation: 0, offset: 0, format: 'float32x3' },  // position
                        { shaderLocation: 1, offset: 12, format: 'float32x3' }, // normal
                    ],
                }],
            },
            fragment: {
                module: shaderModule,
                entryPoint: 'fs_main',
                targets: [{ format: format }],
            },
            primitive: {
                topology: 'triangle-list',
                cullMode: 'back',
            },
            depthStencil: {
                format: 'depth24plus',
                depthWriteEnabled: true,
                depthCompare: 'less',
            },
        });
    }

    createDepthTexture() {
        this.depthTexture = this.device.createTexture({
            size: [this.canvas.width, this.canvas.height],
            format: 'depth24plus',
            usage: GPUTextureUsage.RENDER_ATTACHMENT,
        });
    }

    createCubeGeometry() {
        // Cube vertices with normals (position xyz, normal xyz)
        const vertices = new Float32Array([
            // Front face
            -0.5, -0.5,  0.5,  0, 0, 1,
             0.5, -0.5,  0.5,  0, 0, 1,
             0.5,  0.5,  0.5,  0, 0, 1,
            -0.5,  0.5,  0.5,  0, 0, 1,
            // Back face
             0.5, -0.5, -0.5,  0, 0, -1,
            -0.5, -0.5, -0.5,  0, 0, -1,
            -0.5,  0.5, -0.5,  0, 0, -1,
             0.5,  0.5, -0.5,  0, 0, -1,
            // Top face
            -0.5,  0.5,  0.5,  0, 1, 0,
             0.5,  0.5,  0.5,  0, 1, 0,
             0.5,  0.5, -0.5,  0, 1, 0,
            -0.5,  0.5, -0.5,  0, 1, 0,
            // Bottom face
            -0.5, -0.5, -0.5,  0, -1, 0,
             0.5, -0.5, -0.5,  0, -1, 0,
             0.5, -0.5,  0.5,  0, -1, 0,
            -0.5, -0.5,  0.5,  0, -1, 0,
            // Right face
             0.5, -0.5,  0.5,  1, 0, 0,
             0.5, -0.5, -0.5,  1, 0, 0,
             0.5,  0.5, -0.5,  1, 0, 0,
             0.5,  0.5,  0.5,  1, 0, 0,
            // Left face
            -0.5, -0.5, -0.5,  -1, 0, 0,
            -0.5, -0.5,  0.5,  -1, 0, 0,
            -0.5,  0.5,  0.5,  -1, 0, 0,
            -0.5,  0.5, -0.5,  -1, 0, 0,
        ]);

        const indices = new Uint16Array([
            0, 1, 2, 0, 2, 3,       // front
            4, 5, 6, 4, 6, 7,       // back
            8, 9, 10, 8, 10, 11,    // top
            12, 13, 14, 12, 14, 15, // bottom
            16, 17, 18, 16, 18, 19, // right
            20, 21, 22, 20, 22, 23, // left
        ]);

        this.vertexBuffer = this.device.createBuffer({
            size: vertices.byteLength,
            usage: GPUBufferUsage.VERTEX | GPUBufferUsage.COPY_DST,
        });
        this.device.queue.writeBuffer(this.vertexBuffer, 0, vertices);

        this.indexBuffer = this.device.createBuffer({
            size: indices.byteLength,
            usage: GPUBufferUsage.INDEX | GPUBufferUsage.COPY_DST,
        });
        this.device.queue.writeBuffer(this.indexBuffer, 0, indices);
        this.indexCount = indices.length;
    }

    async loadWasm(wasmPath) {
        console.log(`Loading WASM: ${wasmPath}`);

        const response = await fetch(wasmPath);
        const wasmBytes = await response.arrayBuffer();

        const { instance } = await WebAssembly.instantiate(wasmBytes, {});

        console.log('WASM exports:', Object.keys(instance.exports));

        // Store WASM exports
        this.wasm = instance.exports;

        // Call init_core to get app pointer
        this.appPtr = this.wasm.init_core();
        console.log(`App pointer: ${this.appPtr}`);

        // Read initial commands using app pointer
        const resultPtr = this.wasm.get_result_ptr(this.appPtr);
        const resultLen = this.wasm.get_result_len(this.appPtr);

        console.log(`Result ptr: ${resultPtr}, len: ${resultLen}`);

        if (resultLen === 0) {
            console.warn('Empty result from WASM');
            return;
        }

        const jsonBytes = new Uint8Array(this.wasm.memory.buffer, resultPtr, resultLen);
        const jsonStr = new TextDecoder().decode(jsonBytes);

        console.log('WASM result:', jsonStr);

        const commands = JSON.parse(jsonStr);
        this.processCommands(commands);
    }

    processCommands(commands) {
        for (const cmd of commands) {
            // Handle tagged enum format: {category: "Environment", command: {action: "SetCamera", ...}}
            if (cmd.category === "Environment" && cmd.command) {
                if (cmd.command.action === "SetCamera") {
                    this.camera.position = cmd.command.position;
                    this.camera.target = cmd.command.target;
                    this.camera.up = cmd.command.up;
                    this.camera.fov = cmd.command.fov_degrees * Math.PI / 180;
                    this.camera.near = cmd.command.near;
                    this.camera.far = cmd.command.far;
                    // console.log('Camera updated:', this.camera);
                }
                continue;
            }

            // Handle Scene commands: {category: "Scene", command: {action: "CreateVolume", ...}}
            if (cmd.category === "Scene" && cmd.command) {
                if (cmd.command.action === "CreateVolume") {
                    this.handleCreateVolume(cmd.command);
                }
                continue;
            }
        }
    }

    handleCreateVolume(cmd) {
        console.log('CreateVolume:', cmd);

        // Extract color from material (material.color is an array)
        let color = [1.0, 1.0, 1.0, 1.0];
        if (cmd.material && cmd.material.color) {
            color = cmd.material.color;
        }

        // Extract size from source - tagged enum format: {Primitive: {Cube: {size: 0.5}}}
        let size = 0.5;
        if (cmd.source && cmd.source.Primitive) {
            if (cmd.source.Primitive.Cube) {
                size = cmd.source.Primitive.Cube.size;
            } else if (cmd.source.Primitive.Sphere) {
                size = cmd.source.Primitive.Sphere.radius * 2;
            } else if (cmd.source.Primitive.Box) {
                size = Math.max(cmd.source.Primitive.Box.width,
                               cmd.source.Primitive.Box.height,
                               cmd.source.Primitive.Box.depth);
            }
        }

        // Extract transform
        const transform = cmd.transform || {};
        const position = transform.position || [0, 0, 0];
        const scale = transform.scale || [1, 1, 1];

        this.volumes.set(cmd.volume_id, {
            id: cmd.volume_id,
            position: position,
            scale: scale,
            size: size,
            color: color,
        });

        console.log('Added volume:', this.volumes.get(cmd.volume_id));
    }

    render() {
        // Calculate delta time
        const now = performance.now();
        const dt = (now - this.lastFrameTime) / 1000.0; // Convert to seconds
        this.lastFrameTime = now;

        // Send frame event to core (handles camera movement)
        this.sendFrameEvent(dt);

        const commandEncoder = this.device.createCommandEncoder();
        const textureView = this.context.getCurrentTexture().createView();

        const renderPass = commandEncoder.beginRenderPass({
            colorAttachments: [{
                view: textureView,
                clearValue: { r: 0.1, g: 0.1, b: 0.15, a: 1.0 },
                loadOp: 'clear',
                storeOp: 'store',
            }],
            depthStencilAttachment: {
                view: this.depthTexture.createView(),
                depthClearValue: 1.0,
                depthLoadOp: 'clear',
                depthStoreOp: 'store',
            },
        });

        renderPass.setPipeline(this.pipeline);
        renderPass.setVertexBuffer(0, this.vertexBuffer);
        renderPass.setIndexBuffer(this.indexBuffer, 'uint16');

        // Render each volume
        for (const volume of this.volumes.values()) {
            const mvp = this.createMVP(volume);
            const uniformData = new Float32Array(20);
            uniformData.set(mvp, 0);
            uniformData.set(volume.color, 16);

            this.device.queue.writeBuffer(this.uniformBuffer, 0, uniformData);
            renderPass.setBindGroup(0, this.uniformBindGroup);
            renderPass.drawIndexed(this.indexCount);
        }

        renderPass.end();
        this.device.queue.submit([commandEncoder.finish()]);

        requestAnimationFrame(() => this.render());
    }

    createMVP(volume) {
        const aspect = this.canvas.width / this.canvas.height;

        // Perspective projection using camera settings
        const f = 1.0 / Math.tan(this.camera.fov / 2);
        const near = this.camera.near;
        const far = this.camera.far;
        const projection = new Float32Array([
            f / aspect, 0, 0, 0,
            0, f, 0, 0,
            0, 0, (far + near) / (near - far), -1,
            0, 0, (2 * far * near) / (near - far), 0,
        ]);

        // View matrix using camera position and target
        const view = this.lookAtRH(this.camera.position, this.camera.target, this.camera.up);

        // Model matrix (scale + position from volume)
        const s = volume.size;
        const model = new Float32Array([
            s, 0, 0, 0,
            0, s, 0, 0,
            0, 0, s, 0,
            volume.position[0], volume.position[1], volume.position[2], 1,
        ]);

        // MVP = projection * view * model
        return this.multiplyMatrices(projection, this.multiplyMatrices(view, model));
    }

    // Right-handed look-at matrix (matches glam's Mat4::look_at_rh)
    lookAtRH(eye, target, up) {
        const zAxis = this.normalize(this.subtract(eye, target));
        const xAxis = this.normalize(this.cross(up, zAxis));
        const yAxis = this.cross(zAxis, xAxis);

        return new Float32Array([
            xAxis[0], yAxis[0], zAxis[0], 0,
            xAxis[1], yAxis[1], zAxis[1], 0,
            xAxis[2], yAxis[2], zAxis[2], 0,
            -this.dot(xAxis, eye), -this.dot(yAxis, eye), -this.dot(zAxis, eye), 1,
        ]);
    }

    normalize(v) {
        const len = Math.sqrt(v[0]*v[0] + v[1]*v[1] + v[2]*v[2]);
        return [v[0]/len, v[1]/len, v[2]/len];
    }

    subtract(a, b) {
        return [a[0]-b[0], a[1]-b[1], a[2]-b[2]];
    }

    cross(a, b) {
        return [
            a[1]*b[2] - a[2]*b[1],
            a[2]*b[0] - a[0]*b[2],
            a[0]*b[1] - a[1]*b[0],
        ];
    }

    dot(a, b) {
        return a[0]*b[0] + a[1]*b[1] + a[2]*b[2];
    }

    multiplyMatrices(a, b) {
        const result = new Float32Array(16);
        for (let i = 0; i < 4; i++) {
            for (let j = 0; j < 4; j++) {
                result[j * 4 + i] =
                    a[i] * b[j * 4] +
                    a[i + 4] * b[j * 4 + 1] +
                    a[i + 8] * b[j * 4 + 2] +
                    a[i + 12] * b[j * 4 + 3];
            }
        }
        return result;
    }
}

// Main entry point
async function main() {
    const canvas = document.getElementById('canvas');
    const errorDiv = document.getElementById('error');

    try {
        const shell = new FastnShell(canvas);
        await shell.init();

        // Get WASM path from URL params or use default
        const params = new URLSearchParams(window.location.search);
        const wasmPath = params.get('app') || WASM_PATH;

        await shell.loadWasm(wasmPath);
        shell.render();

        console.log('fastn-shell-web running');
        console.log('Controls: WASD=move, IJKL=rotate, QE=up/down, 0=reset');
    } catch (e) {
        console.error(e);
        errorDiv.textContent = e.message;
        canvas.style.display = 'none';
    }
}

main();
