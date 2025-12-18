// fastn-shell-web - Shared code for WASM interaction and event handling
// Used by both WebGPU and WebGL+XR shells

const WASM_PATH = './cube_glb.wasm';

// ============================================================================
// WASM Core - Handles WASM loading and event communication
// ============================================================================

class FastnCore {
    constructor() {
        this.wasm = null;
        this.appPtr = null;
        this.frameNumber = 0;
    }

    async loadWasm(wasmPath) {
        console.log(`Loading WASM: ${wasmPath}`);

        const response = await fetch(wasmPath);
        const wasmBytes = await response.arrayBuffer();

        const { instance } = await WebAssembly.instantiate(wasmBytes, {});

        console.log('WASM exports:', Object.keys(instance.exports));

        this.wasm = instance.exports;
        this.appPtr = this.wasm.init_core();
        console.log(`App pointer: ${this.appPtr}`);

        // Read initial commands
        const resultPtr = this.wasm.get_result_ptr(this.appPtr);
        const resultLen = this.wasm.get_result_len(this.appPtr);

        console.log(`Result ptr: ${resultPtr}, len: ${resultLen}`);

        if (resultLen === 0) {
            console.warn('Empty result from WASM');
            return [];
        }

        const jsonBytes = new Uint8Array(this.wasm.memory.buffer, resultPtr, resultLen);
        const jsonStr = new TextDecoder().decode(jsonBytes);

        console.log('WASM result:', jsonStr);
        return JSON.parse(jsonStr);
    }

    sendEvent(event) {
        if (!this.wasm || this.appPtr === null) return [];

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

    // Convenience methods for common events
    sendKeyEvent(type, code, key, modifiers = {}) {
        return this.sendEvent({
            category: "Input",
            event: {
                type: "Keyboard",
                action: type,
                device_id: "keyboard-0",
                key: key,
                code: code,
                shift: modifiers.shift || false,
                ctrl: modifiers.ctrl || false,
                alt: modifiers.alt || false,
                meta: modifiers.meta || false,
                repeat: modifiers.repeat || false
            }
        });
    }

    sendFrameEvent(dt) {
        this.frameNumber++;
        return this.sendEvent({
            category: "Lifecycle",
            event: {
                type: "Frame",
                time: performance.now() / 1000.0,
                dt: dt,
                frame: this.frameNumber
            }
        });
    }

    sendXrSessionEvent(state) {
        return this.sendEvent({
            category: "Xr",
            event: {
                type: "SessionChanged",
                state: state
            }
        });
    }

    sendHeadPoseEvent(position, orientation) {
        return this.sendEvent({
            category: "Xr",
            event: {
                type: "HeadPose",
                position: position,
                orientation: orientation
            }
        });
    }

    sendControllerPoseEvent(hand, pose, gripPose, buttons, axes) {
        return this.sendEvent({
            category: "Xr",
            event: {
                type: "ControllerPose",
                hand: hand,
                pose: pose,
                grip_pose: gripPose,
                buttons: buttons,
                axes: axes
            }
        });
    }

    sendGamepadEvent(axes, buttons) {
        return this.sendEvent({
            category: "Input",
            event: {
                type: "Gamepad",
                action: "Input",
                device_id: "gamepad-0",
                axes: axes,
                buttons: buttons
            }
        });
    }
}

// ============================================================================
// Input Handler - Keyboard input tracking
// ============================================================================

class InputHandler {
    constructor(core) {
        this.core = core;
        this.pressedKeys = new Set();
        this.commandHandler = null;
    }

    setCommandHandler(handler) {
        this.commandHandler = handler;
    }

    setup(canvas) {
        window.addEventListener('keydown', (e) => {
            if (!this.pressedKeys.has(e.code)) {
                this.pressedKeys.add(e.code);
                const commands = this.core.sendKeyEvent('KeyDown', e.code, e.key, {
                    shift: e.shiftKey,
                    ctrl: e.ctrlKey,
                    alt: e.altKey,
                    meta: e.metaKey,
                    repeat: e.repeat
                });
                if (this.commandHandler) {
                    this.commandHandler(commands);
                }
            }
        });

        window.addEventListener('keyup', (e) => {
            this.pressedKeys.delete(e.code);
            const commands = this.core.sendKeyEvent('KeyUp', e.code, e.key, {
                shift: e.shiftKey,
                ctrl: e.ctrlKey,
                alt: e.altKey,
                meta: e.metaKey
            });
            if (this.commandHandler) {
                this.commandHandler(commands);
            }
        });

        // Focus canvas for keyboard events
        canvas.tabIndex = 0;
        canvas.focus();
    }
}

// ============================================================================
// Scene State - Tracks volumes and camera from commands
// ============================================================================

class SceneState {
    constructor() {
        this.volumes = new Map();
        this.camera = {
            position: [0, 1.6, 3],
            target: [0, 0, 0],
            up: [0, 1, 0],
            fov: Math.PI / 4,
            near: 0.1,
            far: 100.0,
        };
        this.assetManager = new AssetManager();
        this.pendingAssets = []; // Assets to be loaded
        this.onVolumeCreated = null; // Callback for custom mesh creation
    }

    async processCommands(commands) {
        for (const cmd of commands) {
            if (cmd.category === "Asset" && cmd.command) {
                if (cmd.command.action === "Load") {
                    // Queue asset for loading
                    this.pendingAssets.push({
                        asset_id: cmd.command.asset_id,
                        path: cmd.command.path,
                    });
                }
                continue;
            }

            if (cmd.category === "Environment" && cmd.command) {
                if (cmd.command.action === "SetCamera") {
                    this.camera.position = cmd.command.position;
                    this.camera.target = cmd.command.target;
                    this.camera.up = cmd.command.up;
                    this.camera.fov = cmd.command.fov_degrees * Math.PI / 180;
                    this.camera.near = cmd.command.near;
                    this.camera.far = cmd.command.far;
                }
                continue;
            }

            if (cmd.category === "Scene" && cmd.command) {
                if (cmd.command.action === "CreateVolume") {
                    this.handleCreateVolume(cmd.command);
                } else if (cmd.command.action === "DestroyVolume") {
                    this.volumes.delete(cmd.command.volume_id);
                }
                continue;
            }
        }

        // Load any pending assets
        await this.loadPendingAssets();
    }

    async loadPendingAssets() {
        for (const asset of this.pendingAssets) {
            await this.assetManager.load(asset.asset_id, asset.path);
        }
        this.pendingAssets = [];
    }

    handleCreateVolume(cmd) {
        console.log('CreateVolume:', cmd);

        let color = [1.0, 1.0, 1.0, 1.0];
        if (cmd.material && cmd.material.color) {
            color = cmd.material.color;
        }

        let size = 0.5;
        let meshType = 'primitive';
        let assetId = null;

        if (cmd.source) {
            if (cmd.source.Primitive) {
                if (cmd.source.Primitive.Cube) {
                    size = cmd.source.Primitive.Cube.size;
                } else if (cmd.source.Primitive.Sphere) {
                    size = cmd.source.Primitive.Sphere.radius * 2;
                } else if (cmd.source.Primitive.Box) {
                    size = Math.max(cmd.source.Primitive.Box.width,
                                   cmd.source.Primitive.Box.height,
                                   cmd.source.Primitive.Box.depth);
                }
            } else if (cmd.source.Asset) {
                meshType = 'asset';
                assetId = cmd.source.Asset.asset_id;
                // Get color from loaded mesh if available
                const mesh = this.assetManager.getMesh(assetId);
                if (mesh) {
                    color = mesh.color;
                }
            }
        }

        const transform = cmd.transform || {};
        const position = transform.position || [0, 0, 0];
        const scale = transform.scale || [1, 1, 1];

        const volume = {
            id: cmd.volume_id,
            position: position,
            scale: scale,
            size: size,
            color: color,
            meshType: meshType,
            assetId: assetId,
            // These will be set by renderer for custom meshes
            customBuffers: null,
        };

        this.volumes.set(cmd.volume_id, volume);

        // Notify renderer to create custom buffers if needed
        if (meshType === 'asset' && this.onVolumeCreated) {
            this.onVolumeCreated(volume, this.assetManager);
        }

        console.log('Added volume:', volume);
    }
}

// ============================================================================
// Math Utilities - Shared between renderers
// ============================================================================

const MathUtils = {
    normalize(v) {
        const len = Math.sqrt(v[0]*v[0] + v[1]*v[1] + v[2]*v[2]);
        if (len === 0) return [0, 0, 0];
        return [v[0]/len, v[1]/len, v[2]/len];
    },

    subtract(a, b) {
        return [a[0]-b[0], a[1]-b[1], a[2]-b[2]];
    },

    cross(a, b) {
        return [
            a[1]*b[2] - a[2]*b[1],
            a[2]*b[0] - a[0]*b[2],
            a[0]*b[1] - a[1]*b[0],
        ];
    },

    dot(a, b) {
        return a[0]*b[0] + a[1]*b[1] + a[2]*b[2];
    },

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
    },

    // Perspective projection matrix
    perspectiveRH(fov, aspect, near, far) {
        const f = 1.0 / Math.tan(fov / 2);
        return new Float32Array([
            f / aspect, 0, 0, 0,
            0, f, 0, 0,
            0, 0, (far + near) / (near - far), -1,
            0, 0, (2 * far * near) / (near - far), 0,
        ]);
    },

    // Model matrix from position and scale
    modelMatrix(position, scale) {
        const s = typeof scale === 'number' ? scale : scale[0];
        return new Float32Array([
            s, 0, 0, 0,
            0, s, 0, 0,
            0, 0, s, 0,
            position[0], position[1], position[2], 1,
        ]);
    },

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
    },

    // Identity matrix
    identity() {
        return new Float32Array([
            1, 0, 0, 0,
            0, 1, 0, 0,
            0, 0, 1, 0,
            0, 0, 0, 1,
        ]);
    },

    // Create view matrix from XR pose (position + quaternion orientation)
    viewMatrixFromPose(position, orientation) {
        // Convert quaternion to rotation matrix, then combine with translation
        const [x, y, z, w] = orientation;

        const x2 = x + x, y2 = y + y, z2 = z + z;
        const xx = x * x2, xy = x * y2, xz = x * z2;
        const yy = y * y2, yz = y * z2, zz = z * z2;
        const wx = w * x2, wy = w * y2, wz = w * z2;

        // Rotation matrix (transposed for view matrix)
        const r00 = 1 - (yy + zz);
        const r01 = xy + wz;
        const r02 = xz - wy;
        const r10 = xy - wz;
        const r11 = 1 - (xx + zz);
        const r12 = yz + wx;
        const r20 = xz + wy;
        const r21 = yz - wx;
        const r22 = 1 - (xx + yy);

        // View matrix = inverse of pose (transpose rotation, negate translated position)
        const tx = -(r00 * position[0] + r01 * position[1] + r02 * position[2]);
        const ty = -(r10 * position[0] + r11 * position[1] + r12 * position[2]);
        const tz = -(r20 * position[0] + r21 * position[1] + r22 * position[2]);

        return new Float32Array([
            r00, r10, r20, 0,
            r01, r11, r21, 0,
            r02, r12, r22, 0,
            tx, ty, tz, 1,
        ]);
    }
};

// ============================================================================
// Cube Geometry Data - Shared between renderers
// ============================================================================

const CubeGeometry = {
    // Vertices with normals (position xyz, normal xyz)
    vertices: new Float32Array([
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
    ]),

    indices: new Uint16Array([
        0, 1, 2, 0, 2, 3,       // front
        4, 5, 6, 4, 6, 7,       // back
        8, 9, 10, 8, 10, 11,    // top
        12, 13, 14, 12, 14, 15, // bottom
        16, 17, 18, 16, 18, 19, // right
        20, 21, 22, 20, 22, 23, // left
    ]),

    // For WebGL: interleaved positions only (for simpler shader)
    getPositions() {
        const positions = [];
        for (let i = 0; i < this.vertices.length; i += 6) {
            positions.push(this.vertices[i], this.vertices[i+1], this.vertices[i+2]);
        }
        return new Float32Array(positions);
    },

    getNormals() {
        const normals = [];
        for (let i = 0; i < this.vertices.length; i += 6) {
            normals.push(this.vertices[i+3], this.vertices[i+4], this.vertices[i+5]);
        }
        return new Float32Array(normals);
    }
};

// ============================================================================
// Asset Manager - Loads and caches GLB/glTF files
// ============================================================================

class AssetManager {
    constructor() {
        this.meshes = new Map(); // asset_id -> LoadedMesh
        this.basePath = './';
    }

    setBasePath(path) {
        this.basePath = path.endsWith('/') ? path : path + '/';
    }

    async load(assetId, path) {
        if (this.meshes.has(assetId)) {
            console.log(`Asset ${assetId} already loaded, skipping`);
            return true;
        }

        const fullPath = this.basePath + path;
        console.log(`Loading asset ${assetId} from ${fullPath}`);

        try {
            const response = await fetch(fullPath);
            if (!response.ok) {
                throw new Error(`HTTP ${response.status}: ${response.statusText}`);
            }

            const arrayBuffer = await response.arrayBuffer();
            const mesh = this.parseGLB(arrayBuffer);

            this.meshes.set(assetId, mesh);
            console.log(`Loaded mesh: ${mesh.vertices.length / 3} vertices, ${mesh.indices.length} indices, color: [${mesh.color.join(', ')}]`);
            return true;
        } catch (e) {
            console.error(`Failed to load asset ${assetId}: ${e.message}`);
            return false;
        }
    }

    getMesh(assetId) {
        return this.meshes.get(assetId);
    }

    parseGLB(arrayBuffer) {
        const dataView = new DataView(arrayBuffer);

        // GLB header
        const magic = dataView.getUint32(0, true);
        if (magic !== 0x46546C67) { // 'glTF'
            throw new Error('Invalid GLB magic number');
        }

        const version = dataView.getUint32(4, true);
        if (version !== 2) {
            throw new Error(`Unsupported glTF version: ${version}`);
        }

        // const length = dataView.getUint32(8, true);

        // Read JSON chunk
        let offset = 12;
        const jsonChunkLength = dataView.getUint32(offset, true);
        const jsonChunkType = dataView.getUint32(offset + 4, true);
        offset += 8;

        if (jsonChunkType !== 0x4E4F534A) { // 'JSON'
            throw new Error('First chunk is not JSON');
        }

        const jsonBytes = new Uint8Array(arrayBuffer, offset, jsonChunkLength);
        const json = JSON.parse(new TextDecoder().decode(jsonBytes));
        offset += jsonChunkLength;

        // Read binary chunk
        const binChunkLength = dataView.getUint32(offset, true);
        const binChunkType = dataView.getUint32(offset + 4, true);
        offset += 8;

        if (binChunkType !== 0x004E4942) { // 'BIN\0'
            throw new Error('Second chunk is not BIN');
        }

        const binData = new Uint8Array(arrayBuffer, offset, binChunkLength);

        // Extract mesh data from first primitive
        const mesh = json.meshes[0];
        const primitive = mesh.primitives[0];

        // Get accessors
        const posAccessor = json.accessors[primitive.attributes.POSITION];
        const normalAccessor = primitive.attributes.NORMAL !== undefined
            ? json.accessors[primitive.attributes.NORMAL]
            : null;
        const indexAccessor = json.accessors[primitive.indices];

        // Get buffer views
        const posView = json.bufferViews[posAccessor.bufferView];
        const indexView = json.bufferViews[indexAccessor.bufferView];

        // Extract positions
        const posOffset = (posView.byteOffset || 0) + (posAccessor.byteOffset || 0);
        const positions = new Float32Array(binData.buffer, binData.byteOffset + posOffset, posAccessor.count * 3);

        // Extract normals (or generate defaults)
        let normals;
        if (normalAccessor) {
            const normalView = json.bufferViews[normalAccessor.bufferView];
            const normalOffset = (normalView.byteOffset || 0) + (normalAccessor.byteOffset || 0);
            normals = new Float32Array(binData.buffer, binData.byteOffset + normalOffset, normalAccessor.count * 3);
        } else {
            // Default normals pointing up
            normals = new Float32Array(posAccessor.count * 3);
            for (let i = 0; i < posAccessor.count; i++) {
                normals[i * 3 + 1] = 1.0; // Y-up
            }
        }

        // Extract indices
        const indexOffset = (indexView.byteOffset || 0) + (indexAccessor.byteOffset || 0);
        let indices;
        if (indexAccessor.componentType === 5123) { // UNSIGNED_SHORT
            indices = new Uint16Array(binData.buffer, binData.byteOffset + indexOffset, indexAccessor.count);
        } else if (indexAccessor.componentType === 5125) { // UNSIGNED_INT
            indices = new Uint32Array(binData.buffer, binData.byteOffset + indexOffset, indexAccessor.count);
        } else {
            throw new Error(`Unsupported index type: ${indexAccessor.componentType}`);
        }

        // Extract base color from material
        let color = [0.8, 0.8, 0.8, 1.0]; // Default light gray
        if (primitive.material !== undefined) {
            const material = json.materials[primitive.material];
            if (material.pbrMetallicRoughness && material.pbrMetallicRoughness.baseColorFactor) {
                color = material.pbrMetallicRoughness.baseColorFactor;
            }
        }

        // Copy arrays since they reference the original buffer
        return {
            vertices: new Float32Array(positions),
            normals: new Float32Array(normals),
            indices: indexAccessor.componentType === 5125
                ? new Uint32Array(indices)
                : new Uint16Array(indices),
            indexType: indexAccessor.componentType === 5125 ? 'uint32' : 'uint16',
            color: color,
        };
    }
}

// ============================================================================
// Platform Detection
// ============================================================================

async function detectPlatform() {
    const isOculusBrowser = /OculusBrowser/.test(navigator.userAgent);
    const hasWebXR = navigator.xr !== undefined;

    let supportsImmersiveVR = false;
    if (hasWebXR) {
        try {
            supportsImmersiveVR = await navigator.xr.isSessionSupported('immersive-vr');
        } catch (e) {
            console.warn('WebXR check failed:', e);
        }
    }

    console.log('Platform detection:', {
        isOculusBrowser,
        hasWebXR,
        supportsImmersiveVR,
        userAgent: navigator.userAgent
    });

    // Use WebGL+XR shell for VR headsets
    if (isOculusBrowser || supportsImmersiveVR) {
        return 'webgl-xr';
    }
    return 'webgpu';
}

// Export for use in shell implementations
if (typeof window !== 'undefined') {
    window.FastnCore = FastnCore;
    window.InputHandler = InputHandler;
    window.SceneState = SceneState;
    window.MathUtils = MathUtils;
    window.CubeGeometry = CubeGeometry;
    window.AssetManager = AssetManager;
    window.detectPlatform = detectPlatform;
    window.WASM_PATH = WASM_PATH;
}
