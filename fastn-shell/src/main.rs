//! fastn-shell - Native Shell for Spatial Computing
//!
//! This is a native shell that:
//! 1. Loads a WASM module (compiled from app + fastn)
//! 2. Creates a window with wgpu rendering
//! 3. Sends input events to the WASM core
//! 4. Executes Commands returned by the WASM core
//! 5. Handles gamepad input via SDL2

mod gamepad;
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
    Command, Event, LogLevel,
    InputEvent, KeyboardEvent, KeyEventData, DeviceId,
    LifecycleEvent, FrameEvent,
};

use gamepad::GamepadManager;
use renderer::Renderer;
use wasm_runtime::WasmCore;

struct App {
    window: Option<Arc<Window>>,
    renderer: Option<Renderer>,
    wasm_core: Option<WasmCore>,
    last_frame_time: std::time::Instant,
    wasm_path: String,
    // Queue for commands that need to be executed
    pending_commands: Vec<Command>,
    // SDL2 context and gamepad manager
    sdl_context: sdl2::Sdl,
    gamepad: Option<GamepadManager>,
    // Track last gamepad log time to avoid spam
    last_gamepad_log: std::time::Instant,
    // Frame counter
    frame_count: u64,
}

impl App {
    fn new(wasm_path: String) -> Self {
        // Initialize SDL2 for gamepad support
        let sdl_context = sdl2::init().expect("Failed to initialize SDL2");

        // Initialize gamepad manager
        let gamepad = match GamepadManager::new(&sdl_context) {
            Ok(gp) => Some(gp),
            Err(e) => {
                log::warn!("Failed to initialize gamepad: {}", e);
                None
            }
        };

        Self {
            window: None,
            renderer: None,
            wasm_core: None,
            last_frame_time: std::time::Instant::now(),
            wasm_path,
            pending_commands: Vec::new(),
            sdl_context,
            gamepad,
            last_gamepad_log: std::time::Instant::now(),
            frame_count: 0,
        }
    }

    /// Send an event to the WASM core and execute any resulting commands
    fn send_event(&mut self, event: Event) {
        if let Some(ref mut wasm_core) = self.wasm_core {
            match wasm_core.send_event(&event) {
                Ok(commands) => {
                    self.execute_commands(commands);
                }
                Err(e) => {
                    log::error!("Failed to send event to core: {}", e);
                }
            }
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
                    EnvironmentCommand::SetCamera(camera_data) => {
                        if let Some(renderer) = &mut self.renderer {
                            renderer.set_camera(&camera_data);
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

    /// Convert winit KeyCode to key code string (matching web standard)
    fn keycode_to_string(key_code: KeyCode) -> String {
        match key_code {
            KeyCode::KeyA => "KeyA".to_string(),
            KeyCode::KeyB => "KeyB".to_string(),
            KeyCode::KeyC => "KeyC".to_string(),
            KeyCode::KeyD => "KeyD".to_string(),
            KeyCode::KeyE => "KeyE".to_string(),
            KeyCode::KeyF => "KeyF".to_string(),
            KeyCode::KeyG => "KeyG".to_string(),
            KeyCode::KeyH => "KeyH".to_string(),
            KeyCode::KeyI => "KeyI".to_string(),
            KeyCode::KeyJ => "KeyJ".to_string(),
            KeyCode::KeyK => "KeyK".to_string(),
            KeyCode::KeyL => "KeyL".to_string(),
            KeyCode::KeyM => "KeyM".to_string(),
            KeyCode::KeyN => "KeyN".to_string(),
            KeyCode::KeyO => "KeyO".to_string(),
            KeyCode::KeyP => "KeyP".to_string(),
            KeyCode::KeyQ => "KeyQ".to_string(),
            KeyCode::KeyR => "KeyR".to_string(),
            KeyCode::KeyS => "KeyS".to_string(),
            KeyCode::KeyT => "KeyT".to_string(),
            KeyCode::KeyU => "KeyU".to_string(),
            KeyCode::KeyV => "KeyV".to_string(),
            KeyCode::KeyW => "KeyW".to_string(),
            KeyCode::KeyX => "KeyX".to_string(),
            KeyCode::KeyY => "KeyY".to_string(),
            KeyCode::KeyZ => "KeyZ".to_string(),
            KeyCode::Digit0 => "Digit0".to_string(),
            KeyCode::Digit1 => "Digit1".to_string(),
            KeyCode::Digit2 => "Digit2".to_string(),
            KeyCode::Digit3 => "Digit3".to_string(),
            KeyCode::Digit4 => "Digit4".to_string(),
            KeyCode::Digit5 => "Digit5".to_string(),
            KeyCode::Digit6 => "Digit6".to_string(),
            KeyCode::Digit7 => "Digit7".to_string(),
            KeyCode::Digit8 => "Digit8".to_string(),
            KeyCode::Digit9 => "Digit9".to_string(),
            KeyCode::Space => "Space".to_string(),
            KeyCode::Enter => "Enter".to_string(),
            KeyCode::Escape => "Escape".to_string(),
            KeyCode::ArrowUp => "ArrowUp".to_string(),
            KeyCode::ArrowDown => "ArrowDown".to_string(),
            KeyCode::ArrowLeft => "ArrowLeft".to_string(),
            KeyCode::ArrowRight => "ArrowRight".to_string(),
            KeyCode::ShiftLeft | KeyCode::ShiftRight => "Shift".to_string(),
            KeyCode::ControlLeft | KeyCode::ControlRight => "Control".to_string(),
            KeyCode::AltLeft | KeyCode::AltRight => "Alt".to_string(),
            _ => format!("{:?}", key_code),
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
                    repeat,
                    ..
                },
                ..
            } => {
                // Handle escape to exit (shell-level, not sent to core)
                if key_code == KeyCode::Escape && state == ElementState::Pressed {
                    event_loop.exit();
                    return;
                }

                // Send keyboard event to core
                let code = Self::keycode_to_string(key_code);
                let key_event_data = KeyEventData {
                    device_id: DeviceId::from("keyboard-0"),
                    key: code.clone(),
                    code,
                    shift: false, // TODO: Track modifier state
                    ctrl: false,
                    alt: false,
                    meta: false,
                    repeat,
                };

                let kb_event = match state {
                    ElementState::Pressed => KeyboardEvent::KeyDown(key_event_data),
                    ElementState::Released => KeyboardEvent::KeyUp(key_event_data),
                };

                self.send_event(Event::Input(InputEvent::Keyboard(kb_event)));
            }
            WindowEvent::RedrawRequested => {
                let now = std::time::Instant::now();
                let dt = now.duration_since(self.last_frame_time).as_secs_f32();
                let time = now.elapsed().as_secs_f64();
                self.last_frame_time = now;
                self.frame_count += 1;

                // Pump SDL events (required for gamepad state updates)
                let mut event_pump = self.sdl_context.event_pump().unwrap();
                event_pump.pump_events();

                // Update gamepad state
                if let Some(ref mut gamepad) = self.gamepad {
                    gamepad.update();

                    // Log gamepad state periodically (every 500ms) if there's input
                    if gamepad.has_input()
                        && now.duration_since(self.last_gamepad_log).as_millis() > 500
                    {
                        let state = gamepad.state();
                        log::info!(
                            "Gamepad: L({:.2},{:.2}) R({:.2},{:.2}) LT:{:.2} RT:{:.2} A:{} B:{} X:{} Y:{}",
                            state.left_stick_x,
                            state.left_stick_y,
                            state.right_stick_x,
                            state.right_stick_y,
                            state.left_trigger,
                            state.right_trigger,
                            state.button_a,
                            state.button_b,
                            state.button_x,
                            state.button_y
                        );
                        self.last_gamepad_log = now;
                    }
                }

                // Send Frame event to core (this triggers camera updates based on held keys)
                self.send_event(Event::Lifecycle(LifecycleEvent::Frame(FrameEvent {
                    time,
                    dt,
                    frame: self.frame_count,
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
        eprintln!("Example: fastn-shell ./app.wasm");
        std::process::exit(1);
    });

    let event_loop = EventLoop::new().unwrap();
    event_loop.set_control_flow(ControlFlow::Poll);

    let mut app = App::new(wasm_path);
    event_loop.run_app(&mut app).unwrap();
}
