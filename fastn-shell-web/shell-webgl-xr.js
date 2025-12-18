// fastn-shell-web - WebGL + WebXR Shell for fastn apps
// Uses shared code from shell-common.js for WASM/event handling
// Supports immersive-vr mode for VR headsets like Oculus Quest

class WebGLXRShell {
    constructor(canvas) {
        this.canvas = canvas;
        this.gl = null;
        this.program = null;
        this.positionBuffer = null;
        this.normalBuffer = null;
        this.indexBuffer = null;
        this.indexCount = 0;

        // Shader uniform locations
        this.uniforms = {};

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

        // WebXR state
        this.xrSession = null;
        this.xrRefSpace = null;
        this.xrGLLayer = null;
        this.inVR = false;
    }

    // Create GL buffers for custom mesh from loaded asset
    createCustomMeshBuffers(volume, assetManager) {
        if (!this.gl || volume.meshType !== 'asset') return;

        const mesh = assetManager.getMesh(volume.assetId);
        if (!mesh) {
            console.warn(`Asset ${volume.assetId} not found for volume ${volume.id}`);
            return;
        }

        const gl = this.gl;

        // Position buffer
        const positionBuffer = gl.createBuffer();
        gl.bindBuffer(gl.ARRAY_BUFFER, positionBuffer);
        gl.bufferData(gl.ARRAY_BUFFER, mesh.vertices, gl.STATIC_DRAW);

        // Normal buffer
        const normalBuffer = gl.createBuffer();
        gl.bindBuffer(gl.ARRAY_BUFFER, normalBuffer);
        gl.bufferData(gl.ARRAY_BUFFER, mesh.normals, gl.STATIC_DRAW);

        // Index buffer
        const indexBuffer = gl.createBuffer();
        gl.bindBuffer(gl.ELEMENT_ARRAY_BUFFER, indexBuffer);
        gl.bufferData(gl.ELEMENT_ARRAY_BUFFER, mesh.indices, gl.STATIC_DRAW);

        // Determine index type
        const indexType = mesh.indexType === 'uint32' ? gl.UNSIGNED_INT : gl.UNSIGNED_SHORT;

        volume.customBuffers = {
            positionBuffer,
            normalBuffer,
            indexBuffer,
            indexCount: mesh.indices.length,
            indexType,
        };

        console.log(`Created WebGL buffers for ${volume.id}: ${mesh.vertices.length / 3} vertices, ${mesh.indices.length} indices`);
    }

    async init() {
        // Initialize WebGL
        this.gl = this.canvas.getContext('webgl2', { xrCompatible: true });
        if (!this.gl) {
            this.gl = this.canvas.getContext('webgl', { xrCompatible: true });
        }
        if (!this.gl) {
            throw new Error('WebGL not supported');
        }

        const gl = this.gl;

        // Set initial canvas size
        this.resizeCanvas();

        // Create shader program
        this.createShaderProgram();

        // Create geometry buffers
        this.createCubeGeometry();

        // Setup input handlers
        this.inputHandler.setup(this.canvas);
        this.setupResizeHandler();

        // Enable depth testing
        gl.enable(gl.DEPTH_TEST);
        gl.enable(gl.CULL_FACE);
        gl.cullFace(gl.BACK);

        // Setup VR button if WebXR is available
        await this.setupXRButton();
    }

    async setupXRButton() {
        if (!navigator.xr) {
            console.log('WebXR not available');
            return;
        }

        const supported = await navigator.xr.isSessionSupported('immersive-vr');
        if (!supported) {
            console.log('Immersive VR not supported');
            return;
        }

        // Create VR button
        const vrButton = document.createElement('button');
        vrButton.id = 'vr-button';
        vrButton.textContent = 'Enter VR';
        vrButton.style.cssText = `
            position: absolute;
            bottom: 20px;
            left: 50%;
            transform: translateX(-50%);
            padding: 12px 24px;
            font-size: 16px;
            font-family: sans-serif;
            background: #4CAF50;
            color: white;
            border: none;
            border-radius: 8px;
            cursor: pointer;
            z-index: 100;
        `;
        vrButton.onclick = () => this.toggleVR();
        document.body.appendChild(vrButton);
        this.vrButton = vrButton;
    }

    async toggleVR() {
        if (this.xrSession) {
            await this.xrSession.end();
            return;
        }

        try {
            const session = await navigator.xr.requestSession('immersive-vr', {
                requiredFeatures: ['local-floor'],
            });

            this.xrSession = session;
            this.inVR = true;
            this.vrButton.textContent = 'Exit VR';

            // Create XR WebGL layer
            this.xrGLLayer = new XRWebGLLayer(session, this.gl);
            session.updateRenderState({ baseLayer: this.xrGLLayer });

            // Get reference space
            this.xrRefSpace = await session.requestReferenceSpace('local-floor');

            // Notify core that we entered VR
            const commands = this.core.sendXrSessionEvent('Entered');
            this.sceneState.processCommands(commands);

            // Handle session end
            session.addEventListener('end', () => {
                this.xrSession = null;
                this.xrRefSpace = null;
                this.xrGLLayer = null;
                this.inVR = false;
                this.vrButton.textContent = 'Enter VR';

                // Notify core that we exited VR
                const commands = this.core.sendXrSessionEvent('Exited');
                this.sceneState.processCommands(commands);

                // Resume non-VR rendering
                this.lastFrameTime = performance.now();
                requestAnimationFrame(() => this.render());
            });

            // Start XR render loop
            session.requestAnimationFrame((time, frame) => this.renderXR(time, frame));

        } catch (e) {
            console.error('Failed to start VR session:', e);
        }
    }

    resizeCanvas() {
        const width = window.innerWidth;
        const height = window.innerHeight;
        this.canvas.width = width;
        this.canvas.height = height;
        if (this.gl) {
            this.gl.viewport(0, 0, width, height);
        }
    }

    setupResizeHandler() {
        window.addEventListener('resize', () => {
            if (!this.inVR) {
                this.resizeCanvas();
            }
        });
    }

    createShaderProgram() {
        const gl = this.gl;

        const vsSource = `
            attribute vec3 aPosition;
            attribute vec3 aNormal;

            uniform mat4 uMVP;
            uniform mat4 uModel;

            varying vec3 vNormal;

            void main() {
                gl_Position = uMVP * vec4(aPosition, 1.0);
                vNormal = mat3(uModel) * aNormal;
            }
        `;

        const fsSource = `
            precision mediump float;

            uniform vec4 uColor;

            varying vec3 vNormal;

            void main() {
                vec3 lightDir = normalize(vec3(0.5, 1.0, 0.3));
                float ambient = 0.3;
                float diffuse = max(dot(normalize(vNormal), lightDir), 0.0);
                float brightness = ambient + diffuse * 0.7;
                gl_FragColor = vec4(uColor.rgb * brightness, uColor.a);
            }
        `;

        const vs = this.compileShader(gl.VERTEX_SHADER, vsSource);
        const fs = this.compileShader(gl.FRAGMENT_SHADER, fsSource);

        this.program = gl.createProgram();
        gl.attachShader(this.program, vs);
        gl.attachShader(this.program, fs);
        gl.linkProgram(this.program);

        if (!gl.getProgramParameter(this.program, gl.LINK_STATUS)) {
            throw new Error('Shader link failed: ' + gl.getProgramInfoLog(this.program));
        }

        // Get attribute locations
        this.attribs = {
            position: gl.getAttribLocation(this.program, 'aPosition'),
            normal: gl.getAttribLocation(this.program, 'aNormal'),
        };

        // Get uniform locations
        this.uniforms = {
            mvp: gl.getUniformLocation(this.program, 'uMVP'),
            model: gl.getUniformLocation(this.program, 'uModel'),
            color: gl.getUniformLocation(this.program, 'uColor'),
        };
    }

    compileShader(type, source) {
        const gl = this.gl;
        const shader = gl.createShader(type);
        gl.shaderSource(shader, source);
        gl.compileShader(shader);

        if (!gl.getShaderParameter(shader, gl.COMPILE_STATUS)) {
            const info = gl.getShaderInfoLog(shader);
            gl.deleteShader(shader);
            throw new Error('Shader compile failed: ' + info);
        }

        return shader;
    }

    createCubeGeometry() {
        const gl = this.gl;

        // Use shared geometry data
        const positions = CubeGeometry.getPositions();
        const normals = CubeGeometry.getNormals();
        const indices = CubeGeometry.indices;

        // Position buffer
        this.positionBuffer = gl.createBuffer();
        gl.bindBuffer(gl.ARRAY_BUFFER, this.positionBuffer);
        gl.bufferData(gl.ARRAY_BUFFER, positions, gl.STATIC_DRAW);

        // Normal buffer
        this.normalBuffer = gl.createBuffer();
        gl.bindBuffer(gl.ARRAY_BUFFER, this.normalBuffer);
        gl.bufferData(gl.ARRAY_BUFFER, normals, gl.STATIC_DRAW);

        // Index buffer
        this.indexBuffer = gl.createBuffer();
        gl.bindBuffer(gl.ELEMENT_ARRAY_BUFFER, this.indexBuffer);
        gl.bufferData(gl.ELEMENT_ARRAY_BUFFER, indices, gl.STATIC_DRAW);

        this.indexCount = indices.length;
    }

    async loadWasm(wasmPath) {
        const commands = await this.core.loadWasm(wasmPath);
        this.sceneState.processCommands(commands);
    }

    render() {
        if (this.inVR) return; // XR render loop handles VR rendering

        const gl = this.gl;

        // Calculate delta time
        const now = performance.now();
        const dt = (now - this.lastFrameTime) / 1000.0;
        this.lastFrameTime = now;

        // Send frame event to core
        const commands = this.core.sendFrameEvent(dt);
        this.sceneState.processCommands(commands);

        // Clear
        gl.viewport(0, 0, this.canvas.width, this.canvas.height);
        gl.clearColor(0.1, 0.1, 0.15, 1.0);
        gl.clear(gl.COLOR_BUFFER_BIT | gl.DEPTH_BUFFER_BIT);

        // Render scene
        const camera = this.sceneState.camera;
        const aspect = this.canvas.width / this.canvas.height;
        const projection = MathUtils.perspectiveRH(camera.fov, aspect, camera.near, camera.far);
        const view = MathUtils.lookAtRH(camera.position, camera.target, camera.up);

        this.renderScene(projection, view);

        requestAnimationFrame(() => this.render());
    }

    renderXR(time, frame) {
        const session = this.xrSession;
        if (!session) return;

        // Queue next frame
        session.requestAnimationFrame((t, f) => this.renderXR(t, f));

        const gl = this.gl;
        const glLayer = this.xrGLLayer;

        // Calculate dt
        const now = performance.now();
        const dt = (now - this.lastFrameTime) / 1000.0;
        this.lastFrameTime = now;

        // Get viewer pose
        const pose = frame.getViewerPose(this.xrRefSpace);
        if (!pose) return;

        // Send head pose to core
        const headPos = pose.transform.position;
        const headOri = pose.transform.orientation;
        const headCommands = this.core.sendHeadPoseEvent(
            [headPos.x, headPos.y, headPos.z],
            [headOri.x, headOri.y, headOri.z, headOri.w]
        );
        this.sceneState.processCommands(headCommands);

        // Send frame event
        const frameCommands = this.core.sendFrameEvent(dt);
        this.sceneState.processCommands(frameCommands);

        // Get input sources (controllers)
        for (const inputSource of session.inputSources) {
            if (inputSource.gripSpace) {
                const gripPose = frame.getPose(inputSource.gripSpace, this.xrRefSpace);
                const targetPose = frame.getPose(inputSource.targetRaySpace, this.xrRefSpace);

                if (gripPose && targetPose) {
                    const hand = inputSource.handedness === 'left' ? 'Left' : 'Right';
                    const gamepad = inputSource.gamepad;

                    // Build button/axes data
                    let buttons = [];
                    let axes = [];
                    if (gamepad) {
                        buttons = gamepad.buttons.map(b => [b.value, b.pressed]);
                        axes = Array.from(gamepad.axes);
                    }

                    const ctrlCommands = this.core.sendControllerPoseEvent(
                        hand,
                        {
                            position: [targetPose.transform.position.x, targetPose.transform.position.y, targetPose.transform.position.z],
                            orientation: [targetPose.transform.orientation.x, targetPose.transform.orientation.y, targetPose.transform.orientation.z, targetPose.transform.orientation.w]
                        },
                        {
                            position: [gripPose.transform.position.x, gripPose.transform.position.y, gripPose.transform.position.z],
                            orientation: [gripPose.transform.orientation.x, gripPose.transform.orientation.y, gripPose.transform.orientation.z, gripPose.transform.orientation.w]
                        },
                        buttons,
                        axes
                    );
                    this.sceneState.processCommands(ctrlCommands);
                }
            }
        }

        // Bind XR framebuffer
        gl.bindFramebuffer(gl.FRAMEBUFFER, glLayer.framebuffer);
        gl.clearColor(0.1, 0.1, 0.15, 1.0);
        gl.clear(gl.COLOR_BUFFER_BIT | gl.DEPTH_BUFFER_BIT);

        // Render for each eye
        for (const view of pose.views) {
            const viewport = glLayer.getViewport(view);
            gl.viewport(viewport.x, viewport.y, viewport.width, viewport.height);

            // Get projection and view matrices from XR
            const projection = view.projectionMatrix;
            const viewMatrix = view.transform.inverse.matrix;

            this.renderScene(projection, viewMatrix);
        }
    }

    renderScene(projection, view) {
        const gl = this.gl;

        gl.useProgram(this.program);

        // Render each volume
        for (const volume of this.sceneState.volumes.values()) {
            // For custom meshes, use the scale from transform; for primitives, use size
            const scale = volume.meshType === 'asset' ? volume.scale[0] : volume.size;
            const model = MathUtils.modelMatrix(volume.position, scale);

            // MVP = projection * view * model
            const vp = MathUtils.multiplyMatrices(projection, view);
            const mvp = MathUtils.multiplyMatrices(vp, model);

            gl.uniformMatrix4fv(this.uniforms.mvp, false, mvp);
            gl.uniformMatrix4fv(this.uniforms.model, false, model);
            gl.uniform4fv(this.uniforms.color, volume.color);

            // Use custom buffers for asset meshes, primitive cube for others
            if (volume.customBuffers) {
                gl.bindBuffer(gl.ARRAY_BUFFER, volume.customBuffers.positionBuffer);
                gl.enableVertexAttribArray(this.attribs.position);
                gl.vertexAttribPointer(this.attribs.position, 3, gl.FLOAT, false, 0, 0);

                gl.bindBuffer(gl.ARRAY_BUFFER, volume.customBuffers.normalBuffer);
                gl.enableVertexAttribArray(this.attribs.normal);
                gl.vertexAttribPointer(this.attribs.normal, 3, gl.FLOAT, false, 0, 0);

                gl.bindBuffer(gl.ELEMENT_ARRAY_BUFFER, volume.customBuffers.indexBuffer);
                gl.drawElements(gl.TRIANGLES, volume.customBuffers.indexCount, volume.customBuffers.indexType, 0);
            } else {
                gl.bindBuffer(gl.ARRAY_BUFFER, this.positionBuffer);
                gl.enableVertexAttribArray(this.attribs.position);
                gl.vertexAttribPointer(this.attribs.position, 3, gl.FLOAT, false, 0, 0);

                gl.bindBuffer(gl.ARRAY_BUFFER, this.normalBuffer);
                gl.enableVertexAttribArray(this.attribs.normal);
                gl.vertexAttribPointer(this.attribs.normal, 3, gl.FLOAT, false, 0, 0);

                gl.bindBuffer(gl.ELEMENT_ARRAY_BUFFER, this.indexBuffer);
                gl.drawElements(gl.TRIANGLES, this.indexCount, gl.UNSIGNED_SHORT, 0);
            }
        }
    }
}

// Main entry point for WebGL+XR shell
// wasmPathArg: optional WASM path (from HTML data attribute or caller)
async function initWebGLXR(wasmPathArg) {
    const canvas = document.getElementById('canvas');
    const errorDiv = document.getElementById('error');

    try {
        const shell = new WebGLXRShell(canvas);
        await shell.init();

        // Get WASM path: argument > URL param > canvas data attribute > default
        const params = new URLSearchParams(window.location.search);
        const wasmPath = wasmPathArg || params.get('app') || canvas.dataset.wasm || WASM_PATH;

        await shell.loadWasm(wasmPath);
        shell.render();

        console.log('fastn-shell-web (WebGL+XR) running');
        console.log('Controls: WASD=move, IJKL=rotate, QE=up/down, 0=reset');
        console.log('Click "Enter VR" button for immersive VR mode');
    } catch (e) {
        console.error(e);
        errorDiv.textContent = e.message;
        canvas.style.display = 'none';
    }
}

// Export for use by platform detector
if (typeof window !== 'undefined') {
    window.initWebGLXR = initWebGLXR;
}
