//! Basic wgpu renderer for fastn-shell

use std::sync::Arc;
use winit::window::Window;
use wgpu::util::DeviceExt;
use fastn::{CreateVolumeData, BackgroundData, CameraData};
use glam::{Mat4, Vec3};
use bytemuck::{Pod, Zeroable};
use crate::asset_loader::AssetManager;

#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
struct Vertex {
    position: [f32; 3],
    normal: [f32; 3],
}

#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
struct Uniforms {
    mvp: [[f32; 4]; 4],
    color: [f32; 4],
}

/// Mesh buffers for a volume (either shared or custom)
pub enum VolumeMesh {
    /// Use the shared primitive cube mesh
    Primitive { size: f32 },
    /// Use a custom loaded mesh
    Custom {
        vertex_buffer: wgpu::Buffer,
        index_buffer: wgpu::Buffer,
        num_indices: u32,
    },
}

pub struct Volume {
    pub id: String,
    pub position: [f32; 3],
    pub rotation: [f32; 4],
    pub scale: [f32; 3],
    pub color: [f32; 4],
    pub mesh: VolumeMesh,
}

// Default camera settings
const DEFAULT_CAMERA_POSITION: Vec3 = Vec3::new(0.0, 1.6, 3.0);
const DEFAULT_CAMERA_YAW: f32 = -std::f32::consts::FRAC_PI_2; // Facing -Z (towards origin)
const DEFAULT_CAMERA_PITCH: f32 = -0.5; // Looking slightly down at origin

pub struct Renderer {
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    render_pipeline: wgpu::RenderPipeline,
    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
    uniform_buffer: wgpu::Buffer,
    uniform_bind_group: wgpu::BindGroup,
    depth_texture: wgpu::TextureView,
    num_indices: u32,
    background_color: [f32; 4],
    volumes: Vec<Volume>,
    camera_position: Vec3,
    camera_yaw: f32,   // Rotation around Y axis (left/right)
    camera_pitch: f32, // Rotation around X axis (up/down)
}

impl Renderer {
    pub async fn new(window: Arc<Window>) -> Self {
        let size = window.inner_size();

        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            ..Default::default()
        });

        let surface = instance.create_surface(window).unwrap();

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::default(),
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .unwrap();

        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::default(),
                label: None,
                memory_hints: wgpu::MemoryHints::default(),
                trace: wgpu::Trace::Off,
                experimental_features: Default::default(),
            })
            .await
            .unwrap();

        let surface_caps = surface.get_capabilities(&adapter);
        let surface_format = surface_caps.formats[0];

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width: size.width,
            height: size.height,
            present_mode: wgpu::PresentMode::Fifo,
            alpha_mode: wgpu::CompositeAlphaMode::Auto,
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &config);

        // Create depth texture
        let depth_texture = create_depth_texture(&device, &config);

        // Create shader
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shader.wgsl").into()),
        });

        // Create uniform buffer and bind group
        let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Uniform Buffer"),
            size: std::mem::size_of::<Uniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let uniform_bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Uniform Bind Group Layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });

        let uniform_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Uniform Bind Group"),
            layout: &uniform_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buffer.as_entire_binding(),
            }],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Render Pipeline Layout"),
            bind_group_layouts: &[&uniform_bind_group_layout],
            push_constant_ranges: &[],
        });

        let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Render Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[wgpu::VertexBufferLayout {
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
                }],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: config.format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
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

        // Create cube vertices with normals
        let vertices = create_cube_vertices();
        let indices = create_cube_indices();

        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Vertex Buffer"),
            contents: bytemuck::cast_slice(&vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });

        let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Index Buffer"),
            contents: bytemuck::cast_slice(&indices),
            usage: wgpu::BufferUsages::INDEX,
        });

        Self {
            surface,
            device,
            queue,
            config,
            render_pipeline,
            vertex_buffer,
            index_buffer,
            uniform_buffer,
            uniform_bind_group,
            depth_texture,
            num_indices: indices.len() as u32,
            background_color: [0.1, 0.1, 0.2, 1.0],
            volumes: Vec::new(),
            camera_position: DEFAULT_CAMERA_POSITION,
            camera_yaw: DEFAULT_CAMERA_YAW,
            camera_pitch: DEFAULT_CAMERA_PITCH,
        }
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        if width > 0 && height > 0 {
            self.config.width = width;
            self.config.height = height;
            self.surface.configure(&self.device, &self.config);
            self.depth_texture = create_depth_texture(&self.device, &self.config);
        }
    }

    pub fn set_background(&mut self, bg: &BackgroundData) {
        match bg {
            BackgroundData::Color(color) => {
                self.background_color = *color;
            }
            _ => {}
        }
    }

    pub fn create_volume(&mut self, data: &CreateVolumeData, asset_manager: &AssetManager) {
        // Determine mesh type and create appropriate volume
        let (mesh, color) = match &data.source {
            fastn::VolumeSource::Primitive(p) => {
                let size = match p {
                    fastn::Primitive::Cube { size } => *size,
                    fastn::Primitive::Box { width, .. } => *width,
                    _ => 1.0,
                };
                let color = data.material
                    .as_ref()
                    .and_then(|m| m.color)
                    .unwrap_or([1.0, 1.0, 1.0, 1.0]);
                (VolumeMesh::Primitive { size }, color)
            }
            fastn::VolumeSource::Asset { asset_id, .. } => {
                if let Some(loaded_mesh) = asset_manager.get_mesh(asset_id) {
                    // Create GPU buffers from loaded mesh
                    let vertices: Vec<Vertex> = loaded_mesh.vertices.iter()
                        .zip(loaded_mesh.normals.iter())
                        .map(|(pos, norm)| Vertex {
                            position: *pos,
                            normal: *norm,
                        })
                        .collect();

                    let vertex_buffer = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                        label: Some(&format!("Vertex Buffer {}", data.volume_id)),
                        contents: bytemuck::cast_slice(&vertices),
                        usage: wgpu::BufferUsages::VERTEX,
                    });

                    let index_buffer = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                        label: Some(&format!("Index Buffer {}", data.volume_id)),
                        contents: bytemuck::cast_slice(&loaded_mesh.indices),
                        usage: wgpu::BufferUsages::INDEX,
                    });

                    // Use color from GLB material, or override from command
                    let color = data.material
                        .as_ref()
                        .and_then(|m| m.color)
                        .unwrap_or(loaded_mesh.color);

                    log::info!("Created custom mesh buffers for {} ({} vertices, {} indices)",
                        data.volume_id, vertices.len(), loaded_mesh.indices.len());

                    (VolumeMesh::Custom {
                        vertex_buffer,
                        index_buffer,
                        num_indices: loaded_mesh.indices.len() as u32,
                    }, color)
                } else {
                    log::warn!("Asset {} not found, using placeholder cube", asset_id);
                    let color = data.material
                        .as_ref()
                        .and_then(|m| m.color)
                        .unwrap_or([1.0, 0.5, 0.5, 1.0]); // Pink = missing asset
                    (VolumeMesh::Primitive { size: 1.0 }, color)
                }
            }
        };

        self.volumes.push(Volume {
            id: data.volume_id.clone(),
            position: data.transform.position,
            rotation: data.transform.rotation,
            scale: data.transform.scale,
            color,
            mesh,
        });
        log::info!("Volume created: {} with color {:?} (total: {})",
            data.volume_id, color, self.volumes.len());
    }

    /// Set camera from CameraData (position + target)
    /// Computes yaw and pitch from the direction vector
    pub fn set_camera(&mut self, camera: &CameraData) {
        self.camera_position = Vec3::from_array(camera.position);

        // Compute direction from position to target
        let target = Vec3::from_array(camera.target);
        let direction = (target - self.camera_position).normalize();

        // Extract yaw (rotation around Y) and pitch (rotation around X)
        self.camera_yaw = direction.z.atan2(direction.x);
        self.camera_pitch = direction.y.asin();
    }

    pub fn render(&mut self) {
        let output = match self.surface.get_current_texture() {
            Ok(t) => t,
            Err(_) => return,
        };

        let view = output.texture.create_view(&wgpu::TextureViewDescriptor::default());

        let aspect = self.config.width as f32 / self.config.height as f32;
        let proj = Mat4::perspective_rh(std::f32::consts::FRAC_PI_4, aspect, 0.1, 100.0);

        // Calculate camera direction from yaw and pitch
        let direction = Vec3::new(
            self.camera_yaw.cos() * self.camera_pitch.cos(),
            self.camera_pitch.sin(),
            self.camera_yaw.sin() * self.camera_pitch.cos(),
        );
        let target = self.camera_position + direction;

        let view_mat = Mat4::look_at_rh(
            self.camera_position,
            target,
            Vec3::Y,
        );

        let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
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
                            r: self.background_color[0] as f64,
                            g: self.background_color[1] as f64,
                            b: self.background_color[2] as f64,
                            a: self.background_color[3] as f64,
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
                occlusion_query_set: None,
                timestamp_writes: None,
            });

            render_pass.set_pipeline(&self.render_pipeline);
            render_pass.set_bind_group(0, &self.uniform_bind_group, &[]);

            // Render each volume
            for volume in &self.volumes {
                // Compute scale based on mesh type
                let scale = match &volume.mesh {
                    VolumeMesh::Primitive { size } => Vec3::from_array(volume.scale) * *size,
                    VolumeMesh::Custom { .. } => Vec3::from_array(volume.scale),
                };

                let model = Mat4::from_scale_rotation_translation(
                    scale,
                    glam::Quat::from_array(volume.rotation),
                    Vec3::from_array(volume.position),
                );
                let mvp = proj * view_mat * model;

                let uniforms = Uniforms {
                    mvp: mvp.to_cols_array_2d(),
                    color: volume.color,
                };

                self.queue.write_buffer(
                    &self.uniform_buffer,
                    0,
                    bytemuck::cast_slice(&[uniforms]),
                );

                // Set buffers and draw based on mesh type
                match &volume.mesh {
                    VolumeMesh::Primitive { .. } => {
                        render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
                        render_pass.set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint16);
                        render_pass.draw_indexed(0..self.num_indices, 0, 0..1);
                    }
                    VolumeMesh::Custom { vertex_buffer, index_buffer, num_indices } => {
                        render_pass.set_vertex_buffer(0, vertex_buffer.slice(..));
                        render_pass.set_index_buffer(index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                        render_pass.draw_indexed(0..*num_indices, 0, 0..1);
                    }
                }
            }
        }

        self.queue.submit(std::iter::once(encoder.finish()));
        output.present();
    }
}

fn create_depth_texture(device: &wgpu::Device, config: &wgpu::SurfaceConfiguration) -> wgpu::TextureView {
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("Depth Texture"),
        size: wgpu::Extent3d {
            width: config.width,
            height: config.height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Depth32Float,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    });
    texture.create_view(&wgpu::TextureViewDescriptor::default())
}

fn create_cube_vertices() -> Vec<Vertex> {
    vec![
        // Front face (+Z)
        Vertex { position: [-0.5, -0.5,  0.5], normal: [0.0, 0.0, 1.0] },
        Vertex { position: [ 0.5, -0.5,  0.5], normal: [0.0, 0.0, 1.0] },
        Vertex { position: [ 0.5,  0.5,  0.5], normal: [0.0, 0.0, 1.0] },
        Vertex { position: [-0.5,  0.5,  0.5], normal: [0.0, 0.0, 1.0] },
        // Back face (-Z)
        Vertex { position: [-0.5, -0.5, -0.5], normal: [0.0, 0.0, -1.0] },
        Vertex { position: [-0.5,  0.5, -0.5], normal: [0.0, 0.0, -1.0] },
        Vertex { position: [ 0.5,  0.5, -0.5], normal: [0.0, 0.0, -1.0] },
        Vertex { position: [ 0.5, -0.5, -0.5], normal: [0.0, 0.0, -1.0] },
        // Top face (+Y)
        Vertex { position: [-0.5,  0.5, -0.5], normal: [0.0, 1.0, 0.0] },
        Vertex { position: [-0.5,  0.5,  0.5], normal: [0.0, 1.0, 0.0] },
        Vertex { position: [ 0.5,  0.5,  0.5], normal: [0.0, 1.0, 0.0] },
        Vertex { position: [ 0.5,  0.5, -0.5], normal: [0.0, 1.0, 0.0] },
        // Bottom face (-Y)
        Vertex { position: [-0.5, -0.5, -0.5], normal: [0.0, -1.0, 0.0] },
        Vertex { position: [ 0.5, -0.5, -0.5], normal: [0.0, -1.0, 0.0] },
        Vertex { position: [ 0.5, -0.5,  0.5], normal: [0.0, -1.0, 0.0] },
        Vertex { position: [-0.5, -0.5,  0.5], normal: [0.0, -1.0, 0.0] },
        // Right face (+X)
        Vertex { position: [ 0.5, -0.5, -0.5], normal: [1.0, 0.0, 0.0] },
        Vertex { position: [ 0.5,  0.5, -0.5], normal: [1.0, 0.0, 0.0] },
        Vertex { position: [ 0.5,  0.5,  0.5], normal: [1.0, 0.0, 0.0] },
        Vertex { position: [ 0.5, -0.5,  0.5], normal: [1.0, 0.0, 0.0] },
        // Left face (-X)
        Vertex { position: [-0.5, -0.5, -0.5], normal: [-1.0, 0.0, 0.0] },
        Vertex { position: [-0.5, -0.5,  0.5], normal: [-1.0, 0.0, 0.0] },
        Vertex { position: [-0.5,  0.5,  0.5], normal: [-1.0, 0.0, 0.0] },
        Vertex { position: [-0.5,  0.5, -0.5], normal: [-1.0, 0.0, 0.0] },
    ]
}

fn create_cube_indices() -> Vec<u16> {
    vec![
        0, 1, 2, 2, 3, 0,       // front
        4, 5, 6, 6, 7, 4,       // back
        8, 9, 10, 10, 11, 8,    // top
        12, 13, 14, 14, 15, 12, // bottom
        16, 17, 18, 18, 19, 16, // right
        20, 21, 22, 22, 23, 20, // left
    ]
}
