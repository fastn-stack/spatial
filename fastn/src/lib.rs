use std::sync::Arc;
#[cfg(target_arch = "wasm32")]
use std::cell::RefCell;
#[cfg(target_arch = "wasm32")]
use std::rc::Rc;
use winit::{
    application::ApplicationHandler,
    event::WindowEvent,
    event_loop::{ActiveEventLoop, EventLoop},
    window::{Window, WindowId},
};

use bytemuck::{Pod, Zeroable};
use glam::{Mat4, Vec3};

// Vertex data for 3D rendering
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
struct Vertex {
    position: [f32; 3],
    normal: [f32; 3],
}

impl Vertex {
    fn desc() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Vertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 0,
                    format: wgpu::VertexFormat::Float32x3,
                },
                wgpu::VertexAttribute {
                    offset: std::mem::size_of::<[f32; 3]>() as wgpu::BufferAddress,
                    shader_location: 1,
                    format: wgpu::VertexFormat::Float32x3,
                },
            ],
        }
    }
}

// Camera uniforms
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
struct CameraUniform {
    view_proj: [[f32; 4]; 4],
    model: [[f32; 4]; 4],
}

// Camera controller
struct Camera {
    position: Vec3,
    yaw: f32,   // Rotation around Y axis
    pitch: f32, // Rotation around X axis
    aspect: f32,
    fov_y: f32,
    z_near: f32,
    z_far: f32,
}

impl Camera {
    fn new(aspect: f32) -> Self {
        Self {
            position: Vec3::new(0.0, 0.0, 5.0),
            yaw: 0.0,
            pitch: 0.0,
            aspect,
            fov_y: 45.0_f32.to_radians(),
            z_near: 0.1,
            z_far: 100.0,
        }
    }

    fn reset(&mut self) {
        self.position = Vec3::new(0.0, 0.0, 5.0);
        self.yaw = 0.0;
        self.pitch = 0.0;
    }

    fn view_matrix(&self) -> Mat4 {
        let (sin_yaw, cos_yaw) = self.yaw.sin_cos();
        let (sin_pitch, cos_pitch) = self.pitch.sin_cos();

        let forward = Vec3::new(cos_pitch * sin_yaw, sin_pitch, -cos_pitch * cos_yaw);
        let target = self.position + forward;
        let up = Vec3::Y;

        Mat4::look_at_rh(self.position, target, up)
    }

    fn projection_matrix(&self) -> Mat4 {
        Mat4::perspective_rh(self.fov_y, self.aspect, self.z_near, self.z_far)
    }

    fn view_proj_matrix(&self) -> Mat4 {
        self.projection_matrix() * self.view_matrix()
    }
}

// Input state for camera control
#[derive(Default)]
struct InputState {
    forward: bool,
    backward: bool,
    left: bool,
    right: bool,
    up: bool,
    down: bool,
    look_left: bool,
    look_right: bool,
    look_up: bool,
    look_down: bool,
    // Gamepad axes (normalized -1 to 1)
    left_stick_x: f32,
    left_stick_y: f32,
    right_stick_x: f32,
    right_stick_y: f32,
}

// Mesh data loaded from GLB
struct Mesh {
    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
    num_indices: u32,
}

// Parse GLB data from bytes
fn parse_glb_bytes(data: &[u8]) -> Result<(Vec<Vertex>, Vec<u32>), String> {
    let (document, buffers, _images) =
        gltf::import_slice(data).map_err(|e| format!("Failed to parse GLB: {:?}", e))?;

    let mut vertices = Vec::new();
    let mut indices = Vec::new();

    for mesh in document.meshes() {
        for primitive in mesh.primitives() {
            let reader = primitive.reader(|buffer| Some(&buffers[buffer.index()]));

            // Read positions
            let positions: Vec<[f32; 3]> = reader
                .read_positions()
                .ok_or("No positions found")?
                .collect();

            // Read normals (or generate default)
            let normals: Vec<[f32; 3]> = reader
                .read_normals()
                .map(|n| n.collect())
                .unwrap_or_else(|| vec![[0.0, 1.0, 0.0]; positions.len()]);

            let base_index = vertices.len() as u32;

            for (pos, norm) in positions.iter().zip(normals.iter()) {
                vertices.push(Vertex {
                    position: *pos,
                    normal: *norm,
                });
            }

            // Read indices
            if let Some(indices_reader) = reader.read_indices() {
                for idx in indices_reader.into_u32() {
                    indices.push(base_index + idx);
                }
            } else {
                // No indices, create sequential indices
                for i in 0..positions.len() as u32 {
                    indices.push(base_index + i);
                }
            }
        }
    }

    log::info!(
        "Loaded mesh: {} vertices, {} indices",
        vertices.len(),
        indices.len()
    );

    Ok((vertices, indices))
}

// Load GLB file from filesystem (native only)
#[cfg(not(target_arch = "wasm32"))]
fn load_glb(path: &str) -> Result<(Vec<Vertex>, Vec<u32>), String> {
    let data = std::fs::read(path).map_err(|e| format!("Failed to read file: {:?}", e))?;
    parse_glb_bytes(&data)
}

// Fetch GLB file via HTTP (WASM only)
#[cfg(target_arch = "wasm32")]
async fn fetch_glb(url: &str) -> Result<Vec<u8>, String> {
    use wasm_bindgen::JsCast;
    use wasm_bindgen_futures::JsFuture;

    let window = web_sys::window().ok_or("No window")?;
    let resp_value = JsFuture::from(window.fetch_with_str(url))
        .await
        .map_err(|e| format!("Fetch failed: {:?}", e))?;

    let resp: web_sys::Response = resp_value
        .dyn_into()
        .map_err(|_| "Response cast failed")?;

    if !resp.ok() {
        return Err(format!("HTTP error: {}", resp.status()));
    }

    let array_buffer = JsFuture::from(resp.array_buffer().map_err(|e| format!("{:?}", e))?)
        .await
        .map_err(|e| format!("ArrayBuffer failed: {:?}", e))?;

    let uint8_array = js_sys::Uint8Array::new(&array_buffer);
    let mut data = vec![0u8; uint8_array.length() as usize];
    uint8_array.copy_to(&mut data);

    Ok(data)
}

// Shader source
const SHADER_SOURCE: &str = r#"
struct CameraUniform {
    view_proj: mat4x4<f32>,
    model: mat4x4<f32>,
};
@group(0) @binding(0)
var<uniform> camera: CameraUniform;

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) world_normal: vec3<f32>,
    @location(1) world_position: vec3<f32>,
};

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    let world_pos = camera.model * vec4<f32>(in.position, 1.0);
    out.world_position = world_pos.xyz;
    out.clip_position = camera.view_proj * world_pos;
    // Transform normal (assuming uniform scale)
    out.world_normal = (camera.model * vec4<f32>(in.normal, 0.0)).xyz;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let light_dir = normalize(vec3<f32>(1.0, 1.0, 1.0));
    let normal = normalize(in.world_normal);

    // Simple diffuse lighting
    let diffuse = max(dot(normal, light_dir), 0.0);
    let ambient = 0.2;
    let brightness = ambient + diffuse * 0.8;

    // Orange-ish color for the cube
    let base_color = vec3<f32>(0.9, 0.6, 0.3);
    let color = base_color * brightness;

    return vec4<f32>(color, 1.0);
}
"#;

struct GfxState {
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    render_pipeline: wgpu::RenderPipeline,
    mesh: Option<Mesh>,
    camera: Camera,
    camera_uniform: CameraUniform,
    camera_buffer: wgpu::Buffer,
    camera_bind_group: wgpu::BindGroup,
    depth_texture: wgpu::TextureView,
    model_rotation: f32,
}

impl GfxState {
    async fn new(window: Arc<Window>, glb_path: Option<&str>) -> Result<Self, String> {
        let size = window.inner_size();
        let width = size.width.max(1);
        let height = size.height.max(1);

        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            #[cfg(target_arch = "wasm32")]
            backends: wgpu::Backends::BROWSER_WEBGPU | wgpu::Backends::GL,
            #[cfg(not(target_arch = "wasm32"))]
            backends: wgpu::Backends::all(),
            ..Default::default()
        });

        let surface = instance
            .create_surface(window)
            .map_err(|e| format!("Failed to create surface: {:?}", e))?;

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::default(),
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .map_err(|e| format!("Failed to find adapter: {:?}", e))?;

        log::info!("Adapter: {:?}", adapter.get_info());

        let (device, queue): (wgpu::Device, wgpu::Queue) = adapter
            .request_device(&wgpu::DeviceDescriptor::default())
            .await
            .map_err(|e| format!("Failed to create device: {:?}", e))?;

        let surface_caps = surface.get_capabilities(&adapter);
        let surface_format = surface_caps
            .formats
            .iter()
            .find(|f| f.is_srgb())
            .copied()
            .unwrap_or(surface_caps.formats[0]);

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width,
            height,
            present_mode: wgpu::PresentMode::AutoVsync,
            alpha_mode: surface_caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };

        surface.configure(&device, &config);

        // Create shader module
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Shader"),
            source: wgpu::ShaderSource::Wgsl(SHADER_SOURCE.into()),
        });

        // Create camera
        let camera = Camera::new(width as f32 / height as f32);
        let camera_uniform = CameraUniform {
            view_proj: camera.view_proj_matrix().to_cols_array_2d(),
            model: Mat4::IDENTITY.to_cols_array_2d(),
        };

        let camera_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Camera Buffer"),
            contents: bytemuck::cast_slice(&[camera_uniform]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let camera_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Camera Bind Group Layout"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
            });

        let camera_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Camera Bind Group"),
            layout: &camera_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: camera_buffer.as_entire_binding(),
            }],
        });

        // Create depth texture
        let depth_texture = Self::create_depth_texture(&device, width, height);

        // Create render pipeline
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Render Pipeline Layout"),
            bind_group_layouts: &[&camera_bind_group_layout],
            push_constant_ranges: &[],
        });

        let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Render Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[Vertex::desc()],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: config.format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: Some(wgpu::Face::Back),
                polygon_mode: wgpu::PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::Less,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            multiview: None,
            cache: None,
        });

        // Load mesh if path provided
        let mesh = if let Some(path) = glb_path.filter(|p| !p.is_empty()) {
            log::info!("Loading GLB from: {}", path);

            // Platform-specific loading
            #[cfg(not(target_arch = "wasm32"))]
            let load_result = load_glb(&path);

            #[cfg(target_arch = "wasm32")]
            let load_result = match fetch_glb(&path).await {
                Ok(data) => parse_glb_bytes(&data),
                Err(e) => Err(e),
            };

            match load_result {
                Ok((vertices, indices)) => {
                    let vertex_buffer =
                        device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                            label: Some("Vertex Buffer"),
                            contents: bytemuck::cast_slice(&vertices),
                            usage: wgpu::BufferUsages::VERTEX,
                        });

                    let index_buffer =
                        device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                            label: Some("Index Buffer"),
                            contents: bytemuck::cast_slice(&indices),
                            usage: wgpu::BufferUsages::INDEX,
                        });

                    Some(Mesh {
                        vertex_buffer,
                        index_buffer,
                        num_indices: indices.len() as u32,
                    })
                }
                Err(e) => {
                    log::error!("Failed to load mesh: {}", e);
                    None
                }
            }
        } else {
            None
        };

        Ok(Self {
            surface,
            device,
            queue,
            config,
            render_pipeline,
            mesh,
            camera,
            camera_uniform,
            camera_buffer,
            camera_bind_group,
            depth_texture,
            model_rotation: 0.0,
        })
    }

    fn create_depth_texture(device: &wgpu::Device, width: u32, height: u32) -> wgpu::TextureView {
        let size = wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        };
        let desc = wgpu::TextureDescriptor {
            label: Some("Depth Texture"),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Depth32Float,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        };
        let texture = device.create_texture(&desc);
        texture.create_view(&wgpu::TextureViewDescriptor::default())
    }

    fn resize(&mut self, width: u32, height: u32) {
        if width > 0 && height > 0 {
            self.config.width = width;
            self.config.height = height;
            self.surface.configure(&self.device, &self.config);
            self.depth_texture = Self::create_depth_texture(&self.device, width, height);
            self.camera.aspect = width as f32 / height as f32;
        }
    }

    fn update(&mut self, input: &InputState, dt: f32) {
        let move_speed = 5.0 * dt;
        let look_speed = 2.0 * dt;

        // Camera movement from keyboard
        let (sin_yaw, cos_yaw) = self.camera.yaw.sin_cos();
        let forward = Vec3::new(sin_yaw, 0.0, -cos_yaw);
        let right = Vec3::new(cos_yaw, 0.0, sin_yaw);

        if input.forward {
            self.camera.position += forward * move_speed;
        }
        if input.backward {
            self.camera.position -= forward * move_speed;
        }
        if input.left {
            self.camera.position -= right * move_speed;
        }
        if input.right {
            self.camera.position += right * move_speed;
        }
        if input.up {
            self.camera.position.y += move_speed;
        }
        if input.down {
            self.camera.position.y -= move_speed;
        }

        // Camera rotation from keyboard
        if input.look_left {
            self.camera.yaw -= look_speed;
        }
        if input.look_right {
            self.camera.yaw += look_speed;
        }
        if input.look_up {
            self.camera.pitch += look_speed;
        }
        if input.look_down {
            self.camera.pitch -= look_speed;
        }

        // Gamepad input
        self.camera.position += forward * (-input.left_stick_y * move_speed);
        self.camera.position += right * (input.left_stick_x * move_speed);
        self.camera.yaw += input.right_stick_x * look_speed;
        self.camera.pitch -= input.right_stick_y * look_speed;

        // Clamp pitch
        self.camera.pitch = self.camera.pitch.clamp(-1.5, 1.5);

        // Rotate model slowly
        self.model_rotation += dt * 0.5;

        // Update uniform
        self.camera_uniform.view_proj = self.camera.view_proj_matrix().to_cols_array_2d();
        self.camera_uniform.model =
            Mat4::from_rotation_y(self.model_rotation).to_cols_array_2d();
        self.queue.write_buffer(
            &self.camera_buffer,
            0,
            bytemuck::cast_slice(&[self.camera_uniform]),
        );
    }

    fn render(&self) {
        let output = match self.surface.get_current_texture() {
            Ok(output) => output,
            Err(wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated) => {
                self.surface.configure(&self.device, &self.config);
                return;
            }
            Err(e) => {
                log::error!("Failed to get surface texture: {:?}", e);
                return;
            }
        };

        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Render Encoder"),
            });

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.1,
                            g: 0.1,
                            b: 0.15,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.depth_texture,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            if let Some(mesh) = &self.mesh {
                render_pass.set_pipeline(&self.render_pipeline);
                render_pass.set_bind_group(0, &self.camera_bind_group, &[]);
                render_pass.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
                render_pass.set_index_buffer(mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                render_pass.draw_indexed(0..mesh.num_indices, 0, 0..1);
            }
        }

        self.queue.submit(std::iter::once(encoder.finish()));
        output.present();
    }
}

// We need this trait for buffer initialization
use wgpu::util::DeviceExt;

struct App {
    window: Option<Arc<Window>>,
    glb_path: Option<String>,
    #[cfg(not(target_arch = "wasm32"))]
    gfx: Option<GfxState>,
    #[cfg(target_arch = "wasm32")]
    gfx: Rc<RefCell<Option<GfxState>>>,
    input: InputState,
    #[cfg(not(target_arch = "wasm32"))]
    last_update: std::time::Instant,
    #[cfg(target_arch = "wasm32")]
    last_update_ms: f64,
    #[cfg(not(target_arch = "wasm32"))]
    sdl_context: Option<sdl2::Sdl>,
    #[cfg(not(target_arch = "wasm32"))]
    event_pump: Option<sdl2::EventPump>,
    #[cfg(not(target_arch = "wasm32"))]
    game_controller_subsystem: Option<sdl2::GameControllerSubsystem>,
    #[cfg(not(target_arch = "wasm32"))]
    controllers: std::collections::HashMap<u32, sdl2::controller::GameController>,
    #[cfg(target_arch = "wasm32")]
    gamepad_state: WebGamepadState,
}

#[cfg(target_arch = "wasm32")]
#[derive(Default)]
struct WebGamepadState {
    connected: std::collections::HashSet<u32>,
    button_states: std::collections::HashMap<(u32, u32), bool>,
    axis_states: std::collections::HashMap<(u32, u32), i32>,
}

impl App {
    #[cfg(target_arch = "wasm32")]
    fn new(glb_path: Option<String>) -> Self {
        let now_ms = web_sys::window()
            .and_then(|w| w.performance())
            .map(|p| p.now())
            .unwrap_or(0.0);
        Self {
            window: None,
            glb_path,
            gfx: Rc::new(RefCell::new(None)),
            input: InputState::default(),
            last_update_ms: now_ms,
            gamepad_state: WebGamepadState::default(),
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn new(glb_path: Option<String>) -> Self {
        let mut app = Self {
            window: None,
            glb_path,
            gfx: None,
            input: InputState::default(),
            last_update: std::time::Instant::now(),
            sdl_context: None,
            event_pump: None,
            game_controller_subsystem: None,
            controllers: std::collections::HashMap::new(),
        };
        app.init_sdl();
        app
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn init_sdl(&mut self) {
        log::info!("Initializing SDL2 for gamepad support...");
        match sdl2::init() {
            Ok(sdl) => {
                log::info!("SDL2 initialized successfully");
                match sdl.event_pump() {
                    Ok(pump) => self.event_pump = Some(pump),
                    Err(e) => log::warn!("Failed to create SDL event pump: {}", e),
                }
                match sdl.game_controller() {
                    Ok(gc_subsystem) => {
                        match gc_subsystem.num_joysticks() {
                            Ok(0) => log::info!("No gamepads found at startup"),
                            Ok(num) => {
                                log::info!("Found {} joystick(s) at startup", num);
                                for id in 0..num {
                                    if gc_subsystem.is_game_controller(id) {
                                        match gc_subsystem.open(id) {
                                            Ok(controller) => {
                                                log::info!(
                                                    "Gamepad connected: {} (id: {})",
                                                    controller.name(),
                                                    id
                                                );
                                                self.controllers.insert(id, controller);
                                            }
                                            Err(e) => log::warn!(
                                                "Failed to open controller {}: {}",
                                                id,
                                                e
                                            ),
                                        }
                                    }
                                }
                            }
                            Err(e) => log::warn!("Failed to get joystick count: {}", e),
                        }
                        self.game_controller_subsystem = Some(gc_subsystem);
                    }
                    Err(e) => log::warn!("Failed to init game controller subsystem: {}", e),
                }
                self.sdl_context = Some(sdl);
            }
            Err(e) => log::warn!("Failed to init SDL2: {}", e),
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn poll_gamepad_events(&mut self) {
        let events: Vec<_> = {
            let Some(event_pump) = &mut self.event_pump else {
                return;
            };
            event_pump.poll_iter().collect()
        };

        for event in events {
            use sdl2::event::Event;
            match event {
                Event::ControllerDeviceAdded { which, .. } => {
                    if let Some(gc_subsystem) = &self.game_controller_subsystem {
                        if gc_subsystem.is_game_controller(which) {
                            match gc_subsystem.open(which) {
                                Ok(controller) => {
                                    log::info!(
                                        "Gamepad connected: {} (id: {})",
                                        controller.name(),
                                        which
                                    );
                                    self.controllers.insert(which, controller);
                                }
                                Err(e) => log::warn!("Failed to open controller {}: {}", which, e),
                            }
                        }
                    }
                }
                Event::ControllerDeviceRemoved { which, .. } => {
                    if let Some(controller) = self.controllers.remove(&which) {
                        log::info!(
                            "Gamepad disconnected: {} (id: {})",
                            controller.name(),
                            which
                        );
                    }
                }
                Event::ControllerAxisMotion { axis, value, .. } => {
                    use sdl2::controller::Axis;
                    let normalized = (value as i32) as f32 / 32767.0;
                    // Apply deadzone
                    let normalized = if normalized.abs() < 0.15 {
                        0.0
                    } else {
                        normalized
                    };
                    match axis {
                        Axis::LeftX => self.input.left_stick_x = normalized,
                        Axis::LeftY => self.input.left_stick_y = normalized,
                        Axis::RightX => self.input.right_stick_x = normalized,
                        Axis::RightY => self.input.right_stick_y = normalized,
                        _ => {}
                    }
                }
                _ => {}
            }
        }
    }

    #[cfg(target_arch = "wasm32")]
    fn poll_gamepad_events(&mut self) {
        use wasm_bindgen::JsCast;

        let Some(window) = web_sys::window() else {
            return;
        };
        let navigator = window.navigator();
        let Ok(gamepads_js) = navigator.get_gamepads() else {
            return;
        };

        let mut current_connected: std::collections::HashSet<u32> =
            std::collections::HashSet::new();

        for i in 0..gamepads_js.length() {
            let gamepad_js: wasm_bindgen::JsValue = gamepads_js.get(i);
            if gamepad_js.is_null() || gamepad_js.is_undefined() {
                continue;
            }
            let Ok(gamepad) = gamepad_js.dyn_into::<web_sys::Gamepad>() else {
                continue;
            };

            let index = gamepad.index();
            current_connected.insert(index);

            if !self.gamepad_state.connected.contains(&index) {
                log::info!("Gamepad connected: {} (id: {})", gamepad.id(), index);
            }

            // Poll axes for camera control
            let axes = gamepad.axes();
            if axes.length() >= 4 {
                let get_axis = |idx: u32| -> f32 {
                    let val: wasm_bindgen::JsValue = axes.get(idx);
                    let v = val.as_f64().unwrap_or(0.0) as f32;
                    if v.abs() < 0.15 { 0.0 } else { v }
                };
                self.input.left_stick_x = get_axis(0);
                self.input.left_stick_y = get_axis(1);
                self.input.right_stick_x = get_axis(2);
                self.input.right_stick_y = get_axis(3);
            }
        }

        self.gamepad_state.connected = current_connected;
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }

        let window_attrs = Window::default_attributes().with_title("fastn");

        #[cfg(target_arch = "wasm32")]
        let window_attrs = {
            use winit::platform::web::WindowAttributesExtWebSys;
            window_attrs.with_append(true)
        };

        let window = Arc::new(
            event_loop
                .create_window(window_attrs)
                .expect("Failed to create window"),
        );

        #[cfg(target_arch = "wasm32")]
        {
            use winit::platform::web::WindowExtWebSys;

            let web_window = web_sys::window().unwrap();
            let document = web_window.document().unwrap();
            let body = document.body().unwrap();

            body.style().set_property("margin", "0").unwrap();
            body.style().set_property("padding", "0").unwrap();
            body.style().set_property("overflow", "hidden").unwrap();
            body.style().set_property("width", "100%").unwrap();
            body.style().set_property("height", "100%").unwrap();

            if let Some(html) = document.document_element() {
                let _ = html
                    .set_attribute("style", "margin: 0; padding: 0; width: 100%; height: 100%;");
            }

            let dpr = web_window.device_pixel_ratio();
            let width = web_window.inner_width().unwrap().as_f64().unwrap() as u32;
            let height = web_window.inner_height().unwrap().as_f64().unwrap() as u32;

            let canvas = window.canvas().unwrap();
            canvas
                .style()
                .set_property("width", &format!("{}px", width))
                .unwrap();
            canvas
                .style()
                .set_property("height", &format!("{}px", height))
                .unwrap();
            canvas.style().set_property("display", "block").unwrap();

            let physical_width = (width as f64 * dpr) as u32;
            let physical_height = (height as f64 * dpr) as u32;
            canvas.set_width(physical_width);
            canvas.set_height(physical_height);

            let _ = window
                .request_inner_size(winit::dpi::PhysicalSize::new(physical_width, physical_height));
        }

        self.window = Some(window.clone());

        #[cfg(target_arch = "wasm32")]
        {
            let gfx_ref = self.gfx.clone();
            let glb_path = self.glb_path.clone();
            wasm_bindgen_futures::spawn_local(async move {
                let gfx = GfxState::new(window.clone(), glb_path.as_deref()).await;
                match gfx {
                    Ok(gfx) => {
                        gfx.render();
                        *gfx_ref.borrow_mut() = Some(gfx);
                        log::info!("fastn initialized");
                    }
                    Err(e) => log::error!("Failed to initialize graphics: {}", e),
                }
            });
        }

        #[cfg(not(target_arch = "wasm32"))]
        {
            let gfx = pollster::block_on(GfxState::new(window.clone(), self.glb_path.as_deref()));
            match gfx {
                Ok(gfx) => {
                    self.gfx = Some(gfx);
                    log::info!("fastn initialized");
                }
                Err(e) => log::error!("Failed to initialize graphics: {}", e),
            }
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        self.poll_gamepad_events();

        // Calculate delta time - platform specific
        #[cfg(not(target_arch = "wasm32"))]
        let dt = {
            let now = std::time::Instant::now();
            let dt = (now - self.last_update).as_secs_f32();
            self.last_update = now;
            dt
        };

        #[cfg(target_arch = "wasm32")]
        let dt = {
            let now_ms = web_sys::window()
                .and_then(|w| w.performance())
                .map(|p| p.now())
                .unwrap_or(0.0);
            let dt = ((now_ms - self.last_update_ms) / 1000.0) as f32;
            self.last_update_ms = now_ms;
            dt.min(0.1) // Cap at 100ms to avoid huge jumps
        };

        // Update and render
        #[cfg(not(target_arch = "wasm32"))]
        if let Some(gfx) = &mut self.gfx {
            gfx.update(&self.input, dt);
        }

        #[cfg(target_arch = "wasm32")]
        if let Ok(mut gfx_opt) = self.gfx.try_borrow_mut() {
            if let Some(gfx) = gfx_opt.as_mut() {
                gfx.update(&self.input, dt);
            }
        }

        if let Some(window) = &self.window {
            window.request_redraw();
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => {
                event_loop.exit();
            }
            WindowEvent::Resized(size) => {
                #[cfg(not(target_arch = "wasm32"))]
                if let Some(gfx) = &mut self.gfx {
                    gfx.resize(size.width, size.height);
                }
                #[cfg(target_arch = "wasm32")]
                if let Ok(mut gfx_opt) = self.gfx.try_borrow_mut() {
                    if let Some(gfx) = gfx_opt.as_mut() {
                        gfx.resize(size.width, size.height);
                    }
                }
            }
            WindowEvent::RedrawRequested => {
                #[cfg(not(target_arch = "wasm32"))]
                if let Some(gfx) = &self.gfx {
                    gfx.render();
                }
                #[cfg(target_arch = "wasm32")]
                if let Ok(gfx_opt) = self.gfx.try_borrow() {
                    if let Some(gfx) = gfx_opt.as_ref() {
                        gfx.render();
                    }
                }
            }
            WindowEvent::KeyboardInput {
                event:
                    winit::event::KeyEvent {
                        logical_key, state, ..
                    },
                ..
            } => {
                use winit::event::ElementState;
                use winit::keyboard::{Key, NamedKey};

                let pressed = state == ElementState::Pressed;

                // Handle quit keys (native only)
                #[cfg(not(target_arch = "wasm32"))]
                if pressed {
                    match &logical_key {
                        Key::Character(c) if c == "q" || c == "Q" => {
                            event_loop.exit();
                            return;
                        }
                        Key::Named(NamedKey::Escape) => {
                            event_loop.exit();
                            return;
                        }
                        _ => {}
                    }
                }

                // Reset camera with 0
                if pressed {
                    if let Key::Character(c) = &logical_key {
                        if c == "0" {
                            #[cfg(not(target_arch = "wasm32"))]
                            if let Some(gfx) = &mut self.gfx {
                                gfx.camera.reset();
                                gfx.model_rotation = 0.0;
                                log::info!("Scene reset");
                            }
                            #[cfg(target_arch = "wasm32")]
                            if let Ok(mut gfx_opt) = self.gfx.try_borrow_mut() {
                                if let Some(gfx) = gfx_opt.as_mut() {
                                    gfx.camera.reset();
                                    gfx.model_rotation = 0.0;
                                    log::info!("Scene reset");
                                }
                            }
                        }
                    }
                }

                // Movement controls: WASD
                match &logical_key {
                    Key::Character(c) => match c.as_str() {
                        "w" | "W" => self.input.forward = pressed,
                        "s" | "S" => self.input.backward = pressed,
                        "a" | "A" => self.input.left = pressed,
                        "d" | "D" => self.input.right = pressed,
                        "e" | "E" => self.input.up = pressed,
                        "c" | "C" => self.input.down = pressed,
                        // Look controls: IJKL
                        "i" | "I" => self.input.look_up = pressed,
                        "k" | "K" => self.input.look_down = pressed,
                        "j" | "J" => self.input.look_left = pressed,
                        "l" | "L" => self.input.look_right = pressed,
                        _ => {}
                    },
                    Key::Named(NamedKey::Space) => self.input.up = pressed,
                    Key::Named(NamedKey::Shift) => self.input.down = pressed,
                    _ => {}
                }
            }
            _ => {}
        }
    }
}

// Public API

fn print_controls() {
    log::info!("Controls:");
    log::info!("  WASD        - Move camera (forward/left/back/right)");
    log::info!("  E/C         - Move camera up/down");
    log::info!("  Space/Shift - Move camera up/down (alt)");
    log::info!("  IJKL        - Look around (up/left/down/right)");
    log::info!("  0           - Reset scene");
    #[cfg(not(target_arch = "wasm32"))]
    {
        log::info!("  Q/Escape    - Quit");
        log::info!("  Ctrl-C      - Quit (terminal)");
    }
    log::info!("Gamepad:");
    log::info!("  Left stick  - Move camera");
    log::info!("  Right stick - Look around");
}

/// Render a GLB file and run the application
pub fn render_glb(path: &str) {
    #[cfg(not(target_arch = "wasm32"))]
    {
        env_logger::init();

        ctrlc::set_handler(|| {
            log::info!("Ctrl-C received, exiting...");
            std::process::exit(0);
        })
        .expect("Failed to set Ctrl-C handler");

        log::info!("fastn starting (native)...");
        log::info!("Loading: {}", path);
        print_controls();

        let event_loop = EventLoop::new().expect("Failed to create event loop");
        let mut app = App::new(Some(path.to_string()));

        event_loop.run_app(&mut app).expect("Event loop error");
    }

    #[cfg(target_arch = "wasm32")]
    {
        std::panic::set_hook(Box::new(console_error_panic_hook::hook));
        console_log::init_with_level(log::Level::Info).expect("Failed to initialize logger");

        log::info!("fastn starting (wasm)...");
        log::info!("Loading: {}", path);
        print_controls();

        let event_loop = EventLoop::new().expect("Failed to create event loop");
        let app = App::new(Some(path.to_string()));

        use winit::platform::web::EventLoopExtWebSys;
        event_loop.spawn_app(app);
    }
}

/// Run without loading any GLB (for testing)
pub fn main() {
    render_glb("");
}
