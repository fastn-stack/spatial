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
        this.rotation = 0;
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

        const format = navigator.gpu.getPreferredCanvasFormat();
        this.context.configure({
            device: this.device,
            format: format,
            alphaMode: 'premultiplied',
        });

        await this.createPipeline(format);
        this.createDepthTexture();
        this.createCubeGeometry();
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

        // Empty imports - WASM uses its own memory
        const importObject = {};

        const { instance } = await WebAssembly.instantiate(wasmBytes, importObject);

        // Debug: log all exports
        console.log('WASM exports:', Object.keys(instance.exports));

        // Call init_core to populate the result buffer and get pointer
        const resultPtr = instance.exports.init_core();

        // Call get_result_len to get the length
        const resultLen = instance.exports.get_result_len();

        console.log(`Result ptr: ${resultPtr}, len: ${resultLen}`);

        // Read from WASM's exported memory
        const memory = instance.exports.memory;
        if (!memory) {
            throw new Error('WASM does not export memory');
        }
        console.log('Memory buffer size:', memory.buffer.byteLength);

        const jsonBytes = new Uint8Array(memory.buffer, resultPtr, resultLen);
        const jsonStr = new TextDecoder().decode(jsonBytes);

        console.log('WASM result:', jsonStr);

        if (jsonStr.length === 0) {
            console.warn('Empty result from WASM');
            return;
        }

        const commands = JSON.parse(jsonStr);
        this.processCommands(commands);
    }

    processCommands(commands) {
        for (const item of commands) {
            const cmd = item.command;
            if (cmd && cmd.action === 'CreateVolume') {
                console.log('CreateVolume:', cmd);

                // Extract color from material
                let color = [1.0, 1.0, 1.0, 1.0];
                if (cmd.material && cmd.material.color) {
                    color = cmd.material.color;
                }

                // Extract size from source
                let size = 0.5;
                if (cmd.source && cmd.source.Primitive && cmd.source.Primitive.Cube) {
                    size = cmd.source.Primitive.Cube.size;
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
        }
    }

    render() {
        // No automatic rotation - app controls animation via update loops

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
        const fov = Math.PI / 4;
        const near = 0.1;
        const far = 100.0;

        // Perspective projection (matches native: perspective_rh)
        const f = 1.0 / Math.tan(fov / 2);
        const projection = new Float32Array([
            f / aspect, 0, 0, 0,
            0, f, 0, 0,
            0, 0, (far + near) / (near - far), -1,
            0, 0, (2 * far * near) / (near - far), 0,
        ]);

        // View matrix using look_at_rh (matches native camera)
        // Camera at (0, 1.6, 3) looking at origin
        const view = this.lookAtRH([0, 1.6, 3], [0, 0, 0], [0, 1, 0]);

        // Model matrix (scale + position from volume, no rotation)
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
    } catch (e) {
        console.error(e);
        errorDiv.textContent = e.message;
        canvas.style.display = 'none';
    }
}

main();
