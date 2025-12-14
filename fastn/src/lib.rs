use wasm_bindgen::prelude::*;
use wgpu::SurfaceTarget;

pub fn main() {
    std::panic::set_hook(Box::new(console_error_panic_hook::hook));
    console_log::init_with_level(log::Level::Info).expect("Failed to initialize logger");

    log::info!("fastn starting...");
    wasm_bindgen_futures::spawn_local(async {
        if let Err(e) = run().await {
            log::error!("Error: {}", e);
        }
    });
}

async fn run() -> Result<(), String> {
    log::info!("run() starting...");

    let window = web_sys::window().ok_or("No window found")?;
    let document = window.document().ok_or("No document found")?;
    let body = document.body().ok_or("No body found")?;

    log::info!("Got window, document, body");

    // Clear body and set styles for full-screen canvas
    body.set_inner_html("");
    body.style().set_property("margin", "0").map_err(|e| format!("{:?}", e))?;
    body.style().set_property("padding", "0").map_err(|e| format!("{:?}", e))?;
    body.style().set_property("overflow", "hidden").map_err(|e| format!("{:?}", e))?;

    // Create canvas element
    let canvas = document
        .create_element("canvas")
        .map_err(|e| format!("Failed to create canvas: {:?}", e))?
        .dyn_into::<web_sys::HtmlCanvasElement>()
        .map_err(|e| format!("Failed to cast to HtmlCanvasElement: {:?}", e))?;

    canvas.set_id("fastn-canvas");
    canvas.style().set_property("display", "block").map_err(|e| format!("{:?}", e))?;

    body.append_child(&canvas).map_err(|e| format!("Failed to append canvas: {:?}", e))?;

    // Set initial size
    let width = window.inner_width().map_err(|e| format!("{:?}", e))?.as_f64().ok_or("width not f64")? as u32;
    let height = window.inner_height().map_err(|e| format!("{:?}", e))?.as_f64().ok_or("height not f64")? as u32;
    canvas.set_width(width);
    canvas.set_height(height);

    log::info!("Canvas created: {}x{}", width, height);

    // Initialize wgpu
    let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
        backends: wgpu::Backends::BROWSER_WEBGPU | wgpu::Backends::GL,
        ..Default::default()
    });

    log::info!("wgpu instance created");

    let surface = instance
        .create_surface(SurfaceTarget::Canvas(canvas.clone()))
        .map_err(|e| format!("Failed to create surface: {:?}", e))?;

    log::info!("Surface created");

    let adapter = instance
        .request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::default(),
            compatible_surface: Some(&surface),
            force_fallback_adapter: false,
        })
        .await
        .map_err(|e| format!("Failed to find adapter: {:?}", e))?;

    log::info!("Adapter acquired: {:?}", adapter.get_info());

    let (device, queue): (wgpu::Device, wgpu::Queue) = adapter
        .request_device(&wgpu::DeviceDescriptor::default())
        .await
        .map_err(|e| format!("Failed to create device: {:?}", e))?;

    log::info!("Device and queue created");

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

    // Light orange/yellow clear color (peach)
    let clear_color = wgpu::Color {
        r: 1.0,
        g: 0.9,
        b: 0.7,
        a: 1.0,
    };

    // Store state in closures
    let state = std::rc::Rc::new(std::cell::RefCell::new(State {
        device,
        queue,
        surface,
        config,
        clear_color,
    }));

    // Set up resize handler
    {
        let state = state.clone();
        let canvas = canvas.clone();
        let closure = Closure::<dyn FnMut()>::new(move || {
            let window = web_sys::window().unwrap();
            let width = window.inner_width().unwrap().as_f64().unwrap() as u32;
            let height = window.inner_height().unwrap().as_f64().unwrap() as u32;

            canvas.set_width(width);
            canvas.set_height(height);

            let mut state = state.borrow_mut();
            state.config.width = width;
            state.config.height = height;
            state.surface.configure(&state.device, &state.config);

            render(&state);
        });

        window
            .add_event_listener_with_callback("resize", closure.as_ref().unchecked_ref())
            .unwrap();
        closure.forget();
    }

    // Initial render
    render(&state.borrow());

    log::info!("fastn initialized with {}x{} canvas", width, height);

    Ok(())
}

struct State {
    device: wgpu::Device,
    queue: wgpu::Queue,
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
    clear_color: wgpu::Color,
}

fn render(state: &State) {
    let output = match state.surface.get_current_texture() {
        Ok(output) => output,
        Err(e) => {
            log::error!("Failed to get surface texture: {:?}", e);
            return;
        }
    };

    let view = output
        .texture
        .create_view(&wgpu::TextureViewDescriptor::default());

    let mut encoder = state
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
                    load: wgpu::LoadOp::Clear(state.clear_color),
                    store: wgpu::StoreOp::Store,
                },
                depth_slice: None,
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });
    }

    state.queue.submit(std::iter::once(encoder.finish()));
    output.present();
}
