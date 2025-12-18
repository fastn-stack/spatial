// fastn-shell-web - WebGPU Shell for fastn apps
// Uses shared code from shell-common.js for WASM/event handling

// Import shared modules (loaded via script tag in HTML)
// window.FastnCore, window.InputHandler, window.SceneState, window.MathUtils, window.CubeGeometry, window.WASM_PATH

class WebGPUShell {
    constructor(canvas) {
        this.canvas = canvas;
        this.device = null;
        this.context = null;
        this.pipeline = null;
        this.uniformBuffer = null;
        this.uniformBindGroup = null;
        this.depthTexture = null;
        this.vertexBuffer = null;
        this.indexBuffer = null;
        this.indexCount = 0;
        this.format = null;

        // Shared components
        this.core = new FastnCore();
        this.sceneState = new SceneState();
        this.inputHandler = new InputHandler(this.core);
        this.inputHandler.setCommandHandler((commands) => this.sceneState.processCommands(commands));

        // Set up callback for creating custom mesh buffers
        this.sceneState.onVolumeCreated = (volume, assetManager) => {
            this.createCustomMeshBuffers(volume, assetManager);
        };

        this.lastFrameTime = performance.now();
    }

    // Create GPU buffers for custom mesh from loaded asset
    createCustomMeshBuffers(volume, assetManager) {
        if (!this.device || volume.meshType !== 'asset') return;

        const mesh = assetManager.getMesh(volume.assetId);
        if (!mesh) {
            console.warn(`Asset ${volume.assetId} not found for volume ${volume.id}`);
            return;
        }

        // Interleave vertices and normals for our shader format
        const vertexCount = mesh.vertices.length / 3;
        const interleavedData = new Float32Array(vertexCount * 6);
        for (let i = 0; i < vertexCount; i++) {
            interleavedData[i * 6 + 0] = mesh.vertices[i * 3 + 0];
            interleavedData[i * 6 + 1] = mesh.vertices[i * 3 + 1];
            interleavedData[i * 6 + 2] = mesh.vertices[i * 3 + 2];
            interleavedData[i * 6 + 3] = mesh.normals[i * 3 + 0];
            interleavedData[i * 6 + 4] = mesh.normals[i * 3 + 1];
            interleavedData[i * 6 + 5] = mesh.normals[i * 3 + 2];
        }

        const vertexBuffer = this.device.createBuffer({
            size: interleavedData.byteLength,
            usage: GPUBufferUsage.VERTEX | GPUBufferUsage.COPY_DST,
        });
        this.device.queue.writeBuffer(vertexBuffer, 0, interleavedData);

        // Convert indices to Uint32 for WebGPU if needed
        const indices = mesh.indexType === 'uint32' ? mesh.indices : new Uint32Array(mesh.indices);
        const indexBuffer = this.device.createBuffer({
            size: indices.byteLength,
            usage: GPUBufferUsage.INDEX | GPUBufferUsage.COPY_DST,
        });
        this.device.queue.writeBuffer(indexBuffer, 0, indices);

        volume.customBuffers = {
            vertexBuffer,
            indexBuffer,
            indexCount: mesh.indices.length,
            indexFormat: 'uint32',
        };

        console.log(`Created WebGPU buffers for ${volume.id}: ${vertexCount} vertices, ${mesh.indices.length} indices`);
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

        await this.createPipeline();
        this.createDepthTexture();
        this.createCubeGeometry();
        this.inputHandler.setup(this.canvas);
        this.setupResizeHandler();
    }

    resizeCanvas() {
        const width = window.innerWidth;
        const height = window.innerHeight;

        this.canvas.width = width;
        this.canvas.height = height;

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
            if (this.depthTexture) {
                this.depthTexture.destroy();
            }
            this.createDepthTexture();
        });
    }

    async createPipeline() {
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
                targets: [{ format: this.format }],
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
        // Use shared geometry data
        const vertices = CubeGeometry.vertices;
        const indices = CubeGeometry.indices;

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
        const commands = await this.core.loadWasm(wasmPath);
        this.sceneState.processCommands(commands);
    }

    render() {
        // Calculate delta time
        const now = performance.now();
        const dt = (now - this.lastFrameTime) / 1000.0;
        this.lastFrameTime = now;

        // Send frame event to core (handles camera movement)
        const commands = this.core.sendFrameEvent(dt);
        this.sceneState.processCommands(commands);

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

        // Render each volume
        const camera = this.sceneState.camera;
        for (const volume of this.sceneState.volumes.values()) {
            const mvp = this.createMVP(volume, camera);
            const uniformData = new Float32Array(20);
            uniformData.set(mvp, 0);
            uniformData.set(volume.color, 16);

            this.device.queue.writeBuffer(this.uniformBuffer, 0, uniformData);
            renderPass.setBindGroup(0, this.uniformBindGroup);

            // Use custom buffers for asset meshes, primitive cube for others
            if (volume.customBuffers) {
                renderPass.setVertexBuffer(0, volume.customBuffers.vertexBuffer);
                renderPass.setIndexBuffer(volume.customBuffers.indexBuffer, volume.customBuffers.indexFormat);
                renderPass.drawIndexed(volume.customBuffers.indexCount);
            } else {
                renderPass.setVertexBuffer(0, this.vertexBuffer);
                renderPass.setIndexBuffer(this.indexBuffer, 'uint16');
                renderPass.drawIndexed(this.indexCount);
            }
        }

        renderPass.end();
        this.device.queue.submit([commandEncoder.finish()]);

        requestAnimationFrame(() => this.render());
    }

    createMVP(volume, camera) {
        const aspect = this.canvas.width / this.canvas.height;

        // Use shared math utilities
        const projection = MathUtils.perspectiveRH(camera.fov, aspect, camera.near, camera.far);
        const view = MathUtils.lookAtRH(camera.position, camera.target, camera.up);

        // For custom meshes, use the scale from transform; for primitives, use size
        const scale = volume.meshType === 'asset' ? volume.scale[0] : volume.size;
        const model = MathUtils.modelMatrix(volume.position, scale);

        // MVP = projection * view * model
        return MathUtils.multiplyMatrices(projection, MathUtils.multiplyMatrices(view, model));
    }
}

// Main entry point for WebGPU shell
// wasmPathArg: optional WASM path (from HTML data attribute or caller)
async function initWebGPU(wasmPathArg) {
    const canvas = document.getElementById('canvas');
    const errorDiv = document.getElementById('error');

    try {
        const shell = new WebGPUShell(canvas);
        await shell.init();

        // Get WASM path: argument > URL param > canvas data attribute > default
        const params = new URLSearchParams(window.location.search);
        const wasmPath = wasmPathArg || params.get('app') || canvas.dataset.wasm || WASM_PATH;

        await shell.loadWasm(wasmPath);
        shell.render();

        console.log('fastn-shell-web (WebGPU) running');
        console.log('Controls: WASD=move, IJKL=rotate, QE=up/down, 0=reset');
    } catch (e) {
        console.error(e);
        errorDiv.textContent = e.message;
        canvas.style.display = 'none';
    }
}

// Export for use by platform detector
if (typeof window !== 'undefined') {
    window.initWebGPU = initWebGPU;
}
