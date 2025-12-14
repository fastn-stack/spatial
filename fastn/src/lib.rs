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

fn random_soothing_color() -> wgpu::Color {
    // Generate soothing pastel colors by using high base values with small random variations
    let base = 0.6;
    let range = 0.3;

    // Simple pseudo-random using time
    #[cfg(not(target_arch = "wasm32"))]
    let seed = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0);

    #[cfg(target_arch = "wasm32")]
    let seed = {
        let perf = web_sys::window()
            .and_then(|w| w.performance())
            .map(|p| p.now() as u64)
            .unwrap_or(0);
        perf.wrapping_mul(1103515245).wrapping_add(12345)
    };

    // Simple LCG for pseudo-random
    let r_seed = seed.wrapping_mul(1103515245).wrapping_add(12345);
    let g_seed = r_seed.wrapping_mul(1103515245).wrapping_add(12345);
    let b_seed = g_seed.wrapping_mul(1103515245).wrapping_add(12345);

    let r = base + (r_seed % 1000) as f64 / 1000.0 * range;
    let g = base + (g_seed % 1000) as f64 / 1000.0 * range;
    let b = base + (b_seed % 1000) as f64 / 1000.0 * range;

    wgpu::Color { r, g, b, a: 1.0 }
}

struct GfxState {
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    clear_color: wgpu::Color,
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
            clear_color: random_soothing_color(),
        })
    }

    fn resize(&mut self, width: u32, height: u32) {
        if width > 0 && height > 0 {
            self.config.width = width;
            self.config.height = height;
            self.surface.configure(&self.device, &self.config);
        }
    }

    fn change_color(&mut self) {
        self.clear_color = random_soothing_color();
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
                        load: wgpu::LoadOp::Clear(self.clear_color),
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
    #[cfg(not(target_arch = "wasm32"))]
    gfx: Option<GfxState>,
    #[cfg(target_arch = "wasm32")]
    gfx: Rc<RefCell<Option<GfxState>>>,
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
    fn new() -> Self {
        Self {
            window: None,
            gfx: Rc::new(RefCell::new(None)),
            gamepad_state: WebGamepadState::default(),
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
        // Collect events first to avoid borrow conflicts
        let events: Vec<_> = {
            let Some(event_pump) = &mut self.event_pump else {
                return;
            };
            event_pump.poll_iter().collect()
        };

        let mut had_input = false;

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
                Event::ControllerButtonDown { which, button, .. } => {
                    log::info!(
                        "Gamepad button pressed: {:?} (controller: {})",
                        button,
                        which
                    );
                    had_input = true;
                }
                Event::ControllerButtonUp { which, button, .. } => {
                    log::info!(
                        "Gamepad button released: {:?} (controller: {})",
                        button,
                        which
                    );
                    had_input = true;
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
                        had_input = true;
                    }
                }
                _ => {}
            }
        }

        if had_input {
            self.on_input_event();
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

        let mut current_connected: std::collections::HashSet<u32> = std::collections::HashSet::new();

        for i in 0..gamepads_js.length() {
            let gamepad_js: wasm_bindgen::JsValue = gamepads_js.get(i);
            // getGamepads returns nullable entries
            if gamepad_js.is_null() || gamepad_js.is_undefined() {
                continue;
            }
            let Ok(gamepad) = gamepad_js.dyn_into::<web_sys::Gamepad>() else {
                continue;
            };

            let index = gamepad.index();
            current_connected.insert(index);

            // Check for new connection
            if !self.gamepad_state.connected.contains(&index) {
                log::info!("Gamepad connected: {} (id: {})", gamepad.id(), index);
                // Log initial button/axis count
                log::info!("  Buttons: {}, Axes: {}", gamepad.buttons().length(), gamepad.axes().length());
            }

            // Poll buttons
            let buttons = gamepad.buttons();
            for btn_idx in 0..buttons.length() {
                let button_js: wasm_bindgen::JsValue = buttons.get(btn_idx);
                if button_js.is_null() || button_js.is_undefined() {
                    continue;
                }
                let Ok(button) = button_js.dyn_into::<web_sys::GamepadButton>() else {
                    continue;
                };
                let pressed = button.pressed();
                let key = (index, btn_idx);
                let was_pressed = self.gamepad_state.button_states.get(&key).copied().unwrap_or(false);

                if pressed && !was_pressed {
                    log::info!("Gamepad button pressed: {} (controller: {})", btn_idx, index);
                    self.on_input_event();
                } else if !pressed && was_pressed {
                    log::info!("Gamepad button released: {} (controller: {})", btn_idx, index);
                    self.on_input_event();
                }
                self.gamepad_state.button_states.insert(key, pressed);
            }

            // Poll axes
            let axes = gamepad.axes();
            for axis_idx in 0..axes.length() {
                let value_js: wasm_bindgen::JsValue = axes.get(axis_idx);
                if value_js.is_null() || value_js.is_undefined() {
                    continue;
                }
                let value = (value_js.as_f64().unwrap_or(0.0) * 32767.0) as i32;
                let key = (index, axis_idx);
                let prev_value = self.gamepad_state.axis_states.get(&key).copied().unwrap_or(0);

                // Only log significant changes (deadzone + change threshold)
                if value.abs() > 8000 && (value - prev_value).abs() > 2000 {
                    log::info!("Gamepad axis: {} = {} (controller: {})", axis_idx, value, index);
                    self.on_input_event();
                }
                self.gamepad_state.axis_states.insert(key, value);
            }
        }

        // Check for disconnections
        for &index in &self.gamepad_state.connected {
            if !current_connected.contains(&index) {
                log::info!("Gamepad disconnected (id: {})", index);
            }
        }
        self.gamepad_state.connected = current_connected;
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn on_input_event(&mut self) {
        if let Some(gfx) = &mut self.gfx {
            gfx.change_color();
        }
        if let Some(window) = &self.window {
            window.request_redraw();
        }
    }

    #[cfg(target_arch = "wasm32")]
    fn on_input_event(&mut self) {
        if let Ok(mut gfx_opt) = self.gfx.try_borrow_mut() {
            if let Some(gfx) = gfx_opt.as_mut() {
                gfx.change_color();
            }
        }
        if let Some(window) = &self.window {
            window.request_redraw();
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
            body.style().set_property("width", "100%").unwrap();
            body.style().set_property("height", "100%").unwrap();

            // Also style html element
            if let Some(html) = document.document_element() {
                let _ = html.set_attribute("style", "margin: 0; padding: 0; width: 100%; height: 100%;");
            }

            // Get device pixel ratio for proper scaling
            let dpr = web_window.device_pixel_ratio();

            // Get window dimensions
            let width = web_window.inner_width().unwrap().as_f64().unwrap() as u32;
            let height = web_window.inner_height().unwrap().as_f64().unwrap() as u32;

            // Style canvas - CSS size
            let canvas = window.canvas().unwrap();
            canvas.style().set_property("width", &format!("{}px", width)).unwrap();
            canvas.style().set_property("height", &format!("{}px", height)).unwrap();
            canvas.style().set_property("display", "block").unwrap();

            // Set actual canvas resolution (accounting for DPR)
            let physical_width = (width as f64 * dpr) as u32;
            let physical_height = (height as f64 * dpr) as u32;
            canvas.set_width(physical_width);
            canvas.set_height(physical_height);

            // Request winit to use this size
            let _ = window.request_inner_size(winit::dpi::PhysicalSize::new(physical_width, physical_height));
        }

        self.window = Some(window.clone());

        #[cfg(target_arch = "wasm32")]
        {
            let gfx_ref = self.gfx.clone();
            wasm_bindgen_futures::spawn_local(async move {
                let gfx = GfxState::new(window.clone()).await;
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

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        self.poll_gamepad_events();

        // In WASM, we need to keep polling for gamepad events continuously
        // Request a redraw to keep the event loop active
        #[cfg(target_arch = "wasm32")]
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
                if let Some(window) = &self.window {
                    window.request_redraw();
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
                #[cfg(not(target_arch = "wasm32"))]
                use winit::keyboard::{Key, NamedKey};

                log::info!(
                    "Keyboard: {:?} {:?} (physical: {:?}, repeat: {})",
                    state,
                    logical_key,
                    physical_key,
                    repeat
                );

                // Handle quit keys (native only)
                #[cfg(not(target_arch = "wasm32"))]
                if state == winit::event::ElementState::Pressed {
                    match &logical_key {
                        // 'q' or 'Q' to quit
                        Key::Character(c) if c == "q" || c == "Q" => {
                            event_loop.exit();
                            return;
                        }
                        // Escape to quit
                        Key::Named(NamedKey::Escape) => {
                            event_loop.exit();
                            return;
                        }
                        _ => {}
                    }
                }

                self.on_input_event();
            }
            WindowEvent::ModifiersChanged(modifiers) => {
                log::info!("Modifiers: {:?}", modifiers.state());
            }
            // Mouse button events
            WindowEvent::MouseInput { state, button, .. } => {
                log::info!("Mouse button: {:?} {:?}", state, button);
                self.on_input_event();
            }
            // Mouse movement - no color change for mouse move
            WindowEvent::CursorMoved { position, .. } => {
                log::trace!("Mouse moved: ({:.1}, {:.1})", position.x, position.y);
            }
            // Mouse scroll/wheel
            WindowEvent::MouseWheel { delta, phase, .. } => {
                log::info!("Mouse wheel: {:?} (phase: {:?})", delta, phase);
                self.on_input_event();
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

    // Set up Ctrl-C handler to exit gracefully
    ctrlc::set_handler(|| {
        log::info!("Ctrl-C received, exiting...");
        std::process::exit(0);
    })
    .expect("Failed to set Ctrl-C handler");

    log::info!("fastn starting (native)...");

    let event_loop = EventLoop::new().expect("Failed to create event loop");
    let mut app = App::new();

    event_loop.run_app(&mut app).expect("Event loop error");
}
