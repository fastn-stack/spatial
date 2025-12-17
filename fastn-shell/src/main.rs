//! fastn-shell - Native Shell for Spatial Computing
//!
//! This is a native shell that:
//! 1. Loads a WASM module (compiled from app + fastn)
//! 2. Creates a window with wgpu rendering
//! 3. Executes Commands returned by the WASM core

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

use fastn::{Command, LogLevel};

use renderer::Renderer;
use wasm_runtime::WasmCore;

struct App {
    window: Option<Arc<Window>>,
    renderer: Option<Renderer>,
    #[allow(dead_code)]
    wasm_core: Option<WasmCore>,
    last_frame_time: std::time::Instant,
    wasm_path: String,
    // Queue for commands that need to be executed
    pending_commands: Vec<Command>,
}

impl App {
    fn new(wasm_path: String) -> Self {
        Self {
            window: None,
            renderer: None,
            wasm_core: None,
            last_frame_time: std::time::Instant::now(),
            wasm_path,
            pending_commands: Vec::new(),
        }
    }

    fn execute_commands(&mut self, commands: Vec<Command>) {
        for cmd in commands {
            self.execute_command(cmd);
        }

        // Process any pending commands that were queued
        while !self.pending_commands.is_empty() {
            let commands = std::mem::take(&mut self.pending_commands);
            for cmd in commands {
                self.execute_command(cmd);
            }
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
                log::debug!("Asset command (not implemented): {:?}", asset_cmd);
            }
            Command::Scene(scene_cmd) => {
                use fastn::SceneCommand;
                match scene_cmd {
                    SceneCommand::CreateVolume(data) => {
                        log::info!("Creating volume: {} at {:?}", data.volume_id, data.transform.position);
                        if let Some(renderer) = &mut self.renderer {
                            renderer.create_volume(&data);
                        }
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

        // Load WASM core and get initial commands
        log::info!("Loading WASM module: {}", self.wasm_path);
        let (wasm_core, init_commands) = WasmCore::new(&self.wasm_path)
            .expect("Failed to load WASM module");

        self.window = Some(window);
        self.renderer = Some(renderer);
        self.wasm_core = Some(wasm_core);

        // Execute initial commands
        self.execute_commands(init_commands);
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => {
                event_loop.exit();
            }
            WindowEvent::Resized(size) => {
                if let Some(renderer) = &mut self.renderer {
                    renderer.resize(size.width, size.height);
                }
            }
            WindowEvent::KeyboardInput {
                event: KeyEvent {
                    physical_key: PhysicalKey::Code(key_code),
                    state,
                    ..
                },
                ..
            } => {
                // Handle escape to exit
                if key_code == KeyCode::Escape && state == ElementState::Pressed {
                    event_loop.exit();
                }
            }
            WindowEvent::RedrawRequested => {
                let now = std::time::Instant::now();
                let _dt = now.duration_since(self.last_frame_time).as_secs_f32();
                self.last_frame_time = now;

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
        eprintln!("Example: fastn-shell ./app.wasm");
        std::process::exit(1);
    });

    let event_loop = EventLoop::new().unwrap();
    event_loop.set_control_flow(ControlFlow::Poll);

    let mut app = App::new(wasm_path);
    event_loop.run_app(&mut app).unwrap();
}
