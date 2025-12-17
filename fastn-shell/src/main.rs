//! fastn-shell - Native Shell for Spatial Computing
//!
//! This is a native shell that:
//! 1. Loads a WASM module (compiled from app + fastn-core)
//! 2. Creates a window with wgpu rendering
//! 3. Sends Events to the WASM core
//! 4. Executes Commands returned by the core

mod renderer;
mod wasm_runtime;

use std::sync::Arc;
use winit::{
    application::ApplicationHandler,
    event::{ElementState, KeyEvent, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    keyboard::{KeyCode, PhysicalKey},
    window::{Window, WindowId},
};

use fastn::{
    Command, Event, LifecycleEvent, InitEvent, Platform, FrameEvent, ResizeEvent,
    InputEvent, KeyboardEvent, KeyEventData, AssetEvent, AssetLoadedData, MeshInfo,
    SceneEvent, LogLevel, AssetType,
};

use renderer::Renderer;
use wasm_runtime::WasmCore;

struct App {
    window: Option<Arc<Window>>,
    renderer: Option<Renderer>,
    wasm_core: Option<WasmCore>,
    last_frame_time: std::time::Instant,
    wasm_path: String,
    // Queue for events that need to be sent after current processing
    pending_events: Vec<Event>,
}

impl App {
    fn new(wasm_path: String) -> Self {
        Self {
            window: None,
            renderer: None,
            wasm_core: None,
            last_frame_time: std::time::Instant::now(),
            wasm_path,
            pending_events: Vec::new(),
        }
    }

    fn send_event(&mut self, event: Event) {
        if let Some(core) = &mut self.wasm_core {
            let commands = core.handle_event(&event);
            self.execute_commands(commands);
        }

        // Process any pending events that were queued during command execution
        while !self.pending_events.is_empty() {
            let events = std::mem::take(&mut self.pending_events);
            for event in events {
                if let Some(core) = &mut self.wasm_core {
                    let commands = core.handle_event(&event);
                    self.execute_commands(commands);
                }
            }
        }
    }

    fn execute_commands(&mut self, commands: Vec<Command>) {
        for cmd in commands {
            self.execute_command(cmd);
        }
    }

    fn execute_command(&mut self, cmd: Command) {
        match cmd {
            Command::Debug(debug_cmd) => {
                use fastn::DebugCommand;
                match debug_cmd {
                    DebugCommand::Log { level, message } => {
                        match level {
                            LogLevel::Debug => log::debug!("[Core] {}", message),
                            LogLevel::Info => log::info!("[Core] {}", message),
                            LogLevel::Warn => log::warn!("[Core] {}", message),
                            LogLevel::Error => log::error!("[Core] {}", message),
                        }
                    }
                }
            }
            Command::Asset(asset_cmd) => {
                use fastn::AssetCommand;
                match asset_cmd {
                    AssetCommand::Load { asset_id, path } => {
                        log::info!("Loading asset: {} from {}", asset_id, path);
                        // TODO: Actually load the GLB file
                        // For now, simulate successful load by queueing the event
                        self.pending_events.push(Event::Asset(AssetEvent::Loaded(AssetLoadedData {
                            asset_id: asset_id.clone(),
                            path: path.clone(),
                            asset_type: AssetType::Glb,
                            meshes: vec![MeshInfo {
                                index: 0,
                                name: Some("default".to_string()),
                                vertex_count: 36,
                                has_skeleton: false,
                            }],
                            animations: vec![],
                            skeletons: vec![],
                        })));
                    }
                    AssetCommand::Unload { asset_id } => {
                        log::info!("Unloading asset: {}", asset_id);
                    }
                    AssetCommand::Cancel { asset_id } => {
                        log::info!("Cancel loading asset: {}", asset_id);
                    }
                }
            }
            Command::Scene(scene_cmd) => {
                use fastn::SceneCommand;
                match scene_cmd {
                    SceneCommand::CreateVolume(data) => {
                        log::info!("Creating volume: {} at {:?}", data.volume_id, data.transform.position);
                        if let Some(renderer) = &mut self.renderer {
                            renderer.create_volume(&data);
                        }
                        // Queue volume ready event
                        self.pending_events.push(Event::Scene(SceneEvent::VolumeReady {
                            volume_id: data.volume_id,
                        }));
                    }
                    SceneCommand::SetTransform(data) => {
                        log::debug!("SetTransform: {} -> {:?}", data.volume_id, data.transform.position);
                    }
                    _ => {
                        log::debug!("Unhandled scene command: {:?}", scene_cmd);
                    }
                }
            }
            Command::Environment(env_cmd) => {
                use fastn::EnvironmentCommand;
                match env_cmd {
                    EnvironmentCommand::SetBackground(bg) => {
                        if let Some(renderer) = &mut self.renderer {
                            renderer.set_background(&bg);
                        }
                    }
                    _ => {}
                }
            }
            _ => {
                log::debug!("Unhandled command: {:?}", cmd);
            }
        }
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }

        let window_attrs = Window::default_attributes()
            .with_title("fastn-shell")
            .with_inner_size(winit::dpi::LogicalSize::new(1280, 720));

        let window = Arc::new(event_loop.create_window(window_attrs).unwrap());

        // Create renderer
        let renderer = pollster::block_on(Renderer::new(Arc::clone(&window)));

        // Load WASM core
        log::info!("Loading WASM module: {}", self.wasm_path);
        let wasm_core = WasmCore::new(&self.wasm_path).expect("Failed to load WASM module");

        let size = window.inner_size();

        self.window = Some(window);
        self.renderer = Some(renderer);
        self.wasm_core = Some(wasm_core);

        // Send init event
        let init_event = Event::Lifecycle(LifecycleEvent::Init(InitEvent {
            platform: Platform::Desktop,
            viewport_width: size.width,
            viewport_height: size.height,
            dpr: 1.0,
            xr_supported: false,
            xr_immersive_vr: false,
            xr_immersive_ar: false,
            webrtc_supported: false,
            websocket_supported: false,
            features: vec![],
        }));
        self.send_event(init_event);
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => {
                self.send_event(Event::Lifecycle(LifecycleEvent::Shutdown));
                event_loop.exit();
            }
            WindowEvent::Resized(size) => {
                if let Some(renderer) = &mut self.renderer {
                    renderer.resize(size.width, size.height);
                }
                self.send_event(Event::Lifecycle(LifecycleEvent::Resize(ResizeEvent {
                    width: size.width,
                    height: size.height,
                    dpr: 1.0,
                })));
            }
            WindowEvent::KeyboardInput {
                event: KeyEvent {
                    physical_key: PhysicalKey::Code(key_code),
                    state,
                    ..
                },
                ..
            } => {
                let key_name = format!("{:?}", key_code);
                let key_data = KeyEventData {
                    device_id: "keyboard-0".to_string(),
                    key: key_name.clone(),
                    code: key_name,
                    shift: false,
                    ctrl: false,
                    alt: false,
                    meta: false,
                    repeat: false,
                };

                let event = match state {
                    ElementState::Pressed => {
                        Event::Input(InputEvent::Keyboard(KeyboardEvent::KeyDown(key_data)))
                    }
                    ElementState::Released => {
                        Event::Input(InputEvent::Keyboard(KeyboardEvent::KeyUp(key_data)))
                    }
                };
                self.send_event(event);

                // Handle escape to exit
                if key_code == KeyCode::Escape && state == ElementState::Pressed {
                    event_loop.exit();
                }
            }
            WindowEvent::RedrawRequested => {
                let now = std::time::Instant::now();
                let dt = now.duration_since(self.last_frame_time).as_secs_f32();
                self.last_frame_time = now;

                // Send frame event
                self.send_event(Event::Lifecycle(LifecycleEvent::Frame(FrameEvent {
                    time: now.elapsed().as_secs_f64(),
                    dt,
                    frame: 0,
                })));

                // Render
                if let Some(renderer) = &mut self.renderer {
                    renderer.render();
                }

                // Request next frame
                if let Some(window) = &self.window {
                    window.request_redraw();
                }
            }
            _ => {}
        }
    }
}

fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let args: Vec<String> = std::env::args().collect();
    let wasm_path = args.get(1).cloned().unwrap_or_else(|| {
        eprintln!("Usage: fastn-shell <path-to-wasm>");
        eprintln!("Example: fastn-shell ./amitu.wasm");
        std::process::exit(1);
    });

    let event_loop = EventLoop::new().unwrap();
    event_loop.set_control_flow(ControlFlow::Poll);

    let mut app = App::new(wasm_path);
    event_loop.run_app(&mut app).unwrap();
}
