use std::sync::Arc;
use winit::{
    application::ApplicationHandler,
    event::WindowEvent,
    event_loop::{ActiveEventLoop, EventLoop},
    window::{Window, WindowId},
};

// Light orange/yellow clear color (peach)
const CLEAR_COLOR: wgpu::Color = wgpu::Color {
    r: 1.0,
    g: 0.9,
    b: 0.7,
    a: 1.0,
};

struct GfxState {
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
}

impl GfxState {
    async fn new(window: Arc<Window>) -> Result<Self, String> {
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

        Ok(Self {
            surface,
            device,
            queue,
            config,
        })
    }

    fn resize(&mut self, width: u32, height: u32) {
        if width > 0 && height > 0 {
            self.config.width = width;
            self.config.height = height;
            self.surface.configure(&self.device, &self.config);
        }
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
            let _render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Clear Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(CLEAR_COLOR),
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
        }

        self.queue.submit(std::iter::once(encoder.finish()));
        output.present();
    }
}

struct App {
    window: Option<Arc<Window>>,
    gfx: Option<GfxState>,
    #[cfg(not(target_arch = "wasm32"))]
    sdl_context: Option<sdl2::Sdl>,
    #[cfg(not(target_arch = "wasm32"))]
    event_pump: Option<sdl2::EventPump>,
    #[cfg(not(target_arch = "wasm32"))]
    game_controller_subsystem: Option<sdl2::GameControllerSubsystem>,
    #[cfg(not(target_arch = "wasm32"))]
    controllers: std::collections::HashMap<u32, sdl2::controller::GameController>,
}

impl App {
    #[cfg(target_arch = "wasm32")]
    fn new() -> Self {
        Self {
            window: None,
            gfx: None,
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn new() -> Self {
        let mut app = Self {
            window: None,
            gfx: None,
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
                        // Open any already-connected controllers
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
                                    } else {
                                        log::info!("Joystick {} is not a game controller", id);
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
        let Some(event_pump) = &mut self.event_pump else {
            return;
        };
        let Some(gc_subsystem) = &self.game_controller_subsystem else {
            return;
        };

        for event in event_pump.poll_iter() {
            use sdl2::event::Event;
            match event {
                Event::ControllerDeviceAdded { which, .. } => {
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
                Event::ControllerDeviceRemoved { which, .. } => {
                    if let Some(controller) = self.controllers.remove(&which) {
                        log::info!(
                            "Gamepad disconnected: {} (id: {})",
                            controller.name(),
                            which
                        );
                    }
                }
                Event::ControllerButtonDown { which, button, .. } => {
                    log::info!(
                        "Gamepad button pressed: {:?} (controller: {})",
                        button,
                        which
                    );
                }
                Event::ControllerButtonUp { which, button, .. } => {
                    log::info!(
                        "Gamepad button released: {:?} (controller: {})",
                        button,
                        which
                    );
                }
                Event::ControllerAxisMotion {
                    which, axis, value, ..
                } => {
                    // Only log significant axis movements (deadzone filtering)
                    // Use i32 to avoid overflow with i16::MIN.abs()
                    if (value as i32).abs() > 8000 {
                        log::info!(
                            "Gamepad axis: {:?} = {} (controller: {})",
                            axis,
                            value,
                            which
                        );
                    }
                }
                _ => {}
            }
        }
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

            // Style body for fullscreen canvas
            body.style().set_property("margin", "0").unwrap();
            body.style().set_property("padding", "0").unwrap();
            body.style().set_property("overflow", "hidden").unwrap();

            // Style canvas
            let canvas = window.canvas().unwrap();
            canvas.style().set_property("width", "100vw").unwrap();
            canvas.style().set_property("height", "100vh").unwrap();

            // Request initial size
            let width = web_window.inner_width().unwrap().as_f64().unwrap() as u32;
            let height = web_window.inner_height().unwrap().as_f64().unwrap() as u32;
            let _ = window.request_inner_size(winit::dpi::PhysicalSize::new(width, height));
        }

        self.window = Some(window.clone());

        #[cfg(target_arch = "wasm32")]
        {
            wasm_bindgen_futures::spawn_local(async move {
                let gfx = GfxState::new(window.clone()).await;
                match gfx {
                    Ok(gfx) => {
                        // Store gfx state - we need to use a different approach for WASM
                        // For now, just render once
                        gfx.render();
                        log::info!("fastn initialized");
                        // In WASM, the event loop handles rendering via request_redraw
                    }
                    Err(e) => log::error!("Failed to initialize graphics: {}", e),
                }
            });
        }

        #[cfg(not(target_arch = "wasm32"))]
        {
            let gfx = pollster::block_on(GfxState::new(window.clone()));
            match gfx {
                Ok(gfx) => {
                    self.gfx = Some(gfx);
                    log::info!("fastn initialized");
                }
                Err(e) => log::error!("Failed to initialize graphics: {}", e),
            }
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        self.poll_gamepad_events();
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => {
                event_loop.exit();
            }
            WindowEvent::Resized(size) => {
                if let Some(gfx) = &mut self.gfx {
                    gfx.resize(size.width, size.height);
                }
                if let Some(window) = &self.window {
                    window.request_redraw();
                }
            }
            WindowEvent::RedrawRequested => {
                if let Some(gfx) = &self.gfx {
                    gfx.render();
                }
            }
            // Keyboard events
            WindowEvent::KeyboardInput {
                event:
                    winit::event::KeyEvent {
                        physical_key,
                        logical_key,
                        state,
                        repeat,
                        ..
                    },
                ..
            } => {
                log::info!(
                    "Keyboard: {:?} {:?} (physical: {:?}, repeat: {})",
                    state,
                    logical_key,
                    physical_key,
                    repeat
                );
            }
            WindowEvent::ModifiersChanged(modifiers) => {
                log::info!("Modifiers: {:?}", modifiers.state());
            }
            // Mouse button events
            WindowEvent::MouseInput { state, button, .. } => {
                log::info!("Mouse button: {:?} {:?}", state, button);
            }
            // Mouse movement
            WindowEvent::CursorMoved { position, .. } => {
                log::info!("Mouse moved: ({:.1}, {:.1})", position.x, position.y);
            }
            // Mouse scroll/wheel
            WindowEvent::MouseWheel { delta, phase, .. } => {
                log::info!("Mouse wheel: {:?} (phase: {:?})", delta, phase);
            }
            // Cursor enter/leave window
            WindowEvent::CursorEntered { .. } => {
                log::info!("Cursor entered window");
            }
            WindowEvent::CursorLeft { .. } => {
                log::info!("Cursor left window");
            }
            _ => {}
        }
    }
}

#[cfg(target_arch = "wasm32")]
pub fn main() {
    std::panic::set_hook(Box::new(console_error_panic_hook::hook));
    console_log::init_with_level(log::Level::Info).expect("Failed to initialize logger");

    log::info!("fastn starting (wasm)...");

    let event_loop = EventLoop::new().expect("Failed to create event loop");
    let app = App::new();

    use winit::platform::web::EventLoopExtWebSys;
    event_loop.spawn_app(app);
}

#[cfg(not(target_arch = "wasm32"))]
pub fn main() {
    env_logger::init();

    log::info!("fastn starting (native)...");

    let event_loop = EventLoop::new().expect("Failed to create event loop");
    let mut app = App::new();

    event_loop.run_app(&mut app).expect("Event loop error");
}
