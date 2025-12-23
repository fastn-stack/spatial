//! Quest OpenXR Test - Standalone test for Quest AR passthrough
//!
//! Build for Quest:
//!   cargo apk build --lib
//!
//! Install:
//!   adb install -r target/debug/apk/quest-test.apk

use ash::vk::Handle;
use openxr as xr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

#[cfg(target_os = "android")]
use android_activity::{AndroidApp, MainEvent, PollEvent};

/// Run the XR application
#[cfg(target_os = "android")]
pub fn run_xr_app(app: &AndroidApp) -> Result<(), Box<dyn std::error::Error>> {
    log::info!("=== Starting XR Session ===");

    // Get Android context
    let native_activity = app.activity_as_ptr();
    let vm = ndk_context::android_context().vm();

    // Initialize Meta OpenXR loader
    unsafe { initialize_meta_loader(vm, native_activity)?; }

    // Load OpenXR
    let entry = unsafe {
        xr::Entry::load().map_err(|e| format!("Failed to load OpenXR: {:?}", e))?
    };
    log::info!("OpenXR entry loaded!");

    // Check extensions
    let available_extensions = entry.enumerate_extensions()?;
    log::info!("KHR_vulkan_enable2: {}", available_extensions.khr_vulkan_enable2);
    log::info!("FB_passthrough: {}", available_extensions.fb_passthrough);

    if !available_extensions.khr_vulkan_enable2 {
        return Err("Vulkan not supported".into());
    }

    // Create OpenXR instance
    let mut extensions = xr::ExtensionSet::default();
    extensions.khr_vulkan_enable2 = true;
    if available_extensions.fb_passthrough {
        extensions.fb_passthrough = true;
    }

    let xr_instance = entry.create_instance(
        &xr::ApplicationInfo {
            application_name: "quest-test",
            application_version: 1,
            engine_name: "fastn",
            engine_version: 1,
            api_version: xr::Version::new(1, 0, 0),
        },
        &extensions,
        &[],
    )?;

    let instance_props = xr_instance.properties()?;
    log::info!("Runtime: {} v{}", instance_props.runtime_name, instance_props.runtime_version);

    // Get system
    let system = xr_instance.system(xr::FormFactor::HEAD_MOUNTED_DISPLAY)?;
    let system_props = xr_instance.system_properties(system)?;
    log::info!("System: {}", system_props.system_name);

    // Get view configuration
    let views = xr_instance.enumerate_view_configuration_views(system, xr::ViewConfigurationType::PRIMARY_STEREO)?;
    let view_width = views[0].recommended_image_rect_width;
    let view_height = views[0].recommended_image_rect_height;
    log::info!("Views: {} ({}x{} each)", views.len(), view_width, view_height);

    // Get Vulkan requirements
    let _vk_requirements = xr_instance.graphics_requirements::<xr::Vulkan>(system)?;

    // Create Vulkan instance
    log::info!("Creating Vulkan instance...");
    let vk_entry = unsafe { ash::Entry::load()? };

    let vk_app_info = ash::vk::ApplicationInfo::default()
        .application_name(c"quest-test")
        .application_version(1)
        .engine_name(c"fastn")
        .engine_version(1)
        .api_version(ash::vk::make_api_version(0, 1, 1, 0));

    let vk_instance_create_info = ash::vk::InstanceCreateInfo::default()
        .application_info(&vk_app_info);

    let vk_instance_raw = unsafe {
        xr_instance.create_vulkan_instance(
            system,
            std::mem::transmute(vk_entry.static_fn().get_instance_proc_addr),
            &vk_instance_create_info as *const _ as *const _,
        )?.map_err(|e| format!("Vulkan instance creation failed: {:?}", e))?
    };
    let vk_instance = unsafe { ash::Instance::load(vk_entry.static_fn(), ash::vk::Instance::from_raw(vk_instance_raw as _)) };
    log::info!("Vulkan instance created!");

    // Get Vulkan physical device
    let vk_physical_device = unsafe {
        let pd = xr_instance.vulkan_graphics_device(system, vk_instance.handle().as_raw() as _)?;
        ash::vk::PhysicalDevice::from_raw(pd as _)
    };

    // Find graphics queue family
    let queue_family_props = unsafe { vk_instance.get_physical_device_queue_family_properties(vk_physical_device) };
    let queue_family_index = queue_family_props.iter().position(|props| {
        props.queue_flags.contains(ash::vk::QueueFlags::GRAPHICS)
    }).ok_or("No graphics queue family")? as u32;

    // Create Vulkan device
    let queue_priorities = [1.0f32];
    let queue_create_info = ash::vk::DeviceQueueCreateInfo::default()
        .queue_family_index(queue_family_index)
        .queue_priorities(&queue_priorities);

    let device_create_info = ash::vk::DeviceCreateInfo::default()
        .queue_create_infos(std::slice::from_ref(&queue_create_info));

    let vk_device_raw = unsafe {
        xr_instance.create_vulkan_device(
            system,
            std::mem::transmute(vk_entry.static_fn().get_instance_proc_addr),
            vk_physical_device.as_raw() as _,
            &device_create_info as *const _ as *const _,
        )?.map_err(|e| format!("Vulkan device creation failed: {:?}", e))?
    };
    let vk_device = unsafe { ash::Device::load(vk_instance.fp_v1_0(), ash::vk::Device::from_raw(vk_device_raw as _)) };
    log::info!("Vulkan device created!");

    // Get queue
    let vk_queue = unsafe { vk_device.get_device_queue(queue_family_index, 0) };

    // Create command pool
    let command_pool_info = ash::vk::CommandPoolCreateInfo::default()
        .queue_family_index(queue_family_index)
        .flags(ash::vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER);
    let command_pool = unsafe { vk_device.create_command_pool(&command_pool_info, None)? };

    // Allocate command buffer
    let cmd_alloc_info = ash::vk::CommandBufferAllocateInfo::default()
        .command_pool(command_pool)
        .level(ash::vk::CommandBufferLevel::PRIMARY)
        .command_buffer_count(1);
    let command_buffers = unsafe { vk_device.allocate_command_buffers(&cmd_alloc_info)? };
    let cmd = command_buffers[0];

    // Create fence for synchronization
    let fence_info = ash::vk::FenceCreateInfo::default()
        .flags(ash::vk::FenceCreateFlags::SIGNALED);
    let fence = unsafe { vk_device.create_fence(&fence_info, None)? };

    // Create OpenXR session
    log::info!("Creating OpenXR session...");
    let (session, mut frame_waiter, mut frame_stream) = unsafe {
        xr_instance.create_session::<xr::Vulkan>(
            system,
            &xr::vulkan::SessionCreateInfo {
                instance: vk_instance.handle().as_raw() as _,
                physical_device: vk_physical_device.as_raw() as _,
                device: vk_device.handle().as_raw() as _,
                queue_family_index,
                queue_index: 0,
            },
        )?
    };
    log::info!("OpenXR session created!");

    // Create reference space
    let stage = session.create_reference_space(xr::ReferenceSpaceType::STAGE, xr::Posef::IDENTITY)?;

    // Create swapchains with image handles
    let swapchain_format = ash::vk::Format::R8G8B8A8_SRGB;
    log::info!("Creating swapchains...");

    let mut swapchain_data: Vec<_> = views.iter().map(|view| {
        let swapchain = session.create_swapchain(&xr::SwapchainCreateInfo {
            create_flags: xr::SwapchainCreateFlags::EMPTY,
            usage_flags: xr::SwapchainUsageFlags::COLOR_ATTACHMENT | xr::SwapchainUsageFlags::TRANSFER_DST,
            format: swapchain_format.as_raw() as _,
            sample_count: 1,
            width: view.recommended_image_rect_width,
            height: view.recommended_image_rect_height,
            face_count: 1,
            array_size: 1,
            mip_count: 1,
        }).expect("Failed to create swapchain");

        let images: Vec<ash::vk::Image> = swapchain.enumerate_images()
            .expect("Failed to enumerate swapchain images")
            .into_iter()
            .map(|img| ash::vk::Image::from_raw(img as _))
            .collect();

        log::info!("Swapchain created with {} images", images.len());

        (swapchain, images, view.recommended_image_rect_width, view.recommended_image_rect_height)
    }).collect();

    log::info!("=== Entering XR render loop ===");

    // Track session state
    let mut session_running = false;
    let should_quit = Arc::new(AtomicBool::new(false));
    let mut frame_count = 0u64;

    loop {
        // Check for Android events
        app.poll_events(Some(std::time::Duration::from_millis(0)), |event| {
            if let PollEvent::Main(MainEvent::Destroy) = event {
                should_quit.store(true, Ordering::Relaxed);
            }
        });

        if should_quit.load(Ordering::Relaxed) {
            break;
        }

        // Poll OpenXR events
        let mut event_buffer = xr::EventDataBuffer::new();
        while let Some(event) = xr_instance.poll_event(&mut event_buffer)? {
            match event {
                xr::Event::SessionStateChanged(e) => {
                    log::info!("Session state: {:?}", e.state());
                    match e.state() {
                        xr::SessionState::READY => {
                            session.begin(xr::ViewConfigurationType::PRIMARY_STEREO)?;
                            session_running = true;
                        }
                        xr::SessionState::STOPPING => {
                            session.end()?;
                            session_running = false;
                        }
                        xr::SessionState::EXITING | xr::SessionState::LOSS_PENDING => {
                            should_quit.store(true, Ordering::Relaxed);
                        }
                        _ => {}
                    }
                }
                xr::Event::InstanceLossPending(_) => {
                    should_quit.store(true, Ordering::Relaxed);
                }
                _ => {}
            }
        }

        if !session_running {
            std::thread::sleep(std::time::Duration::from_millis(100));
            continue;
        }

        // Wait for frame
        let frame_state = frame_waiter.wait()?;
        frame_stream.begin()?;

        if !frame_state.should_render {
            frame_stream.end(frame_state.predicted_display_time, xr::EnvironmentBlendMode::OPAQUE, &[])?;
            continue;
        }

        // Get actual view poses and FOVs from OpenXR
        let (_, xr_views) = session.locate_views(
            xr::ViewConfigurationType::PRIMARY_STEREO,
            frame_state.predicted_display_time,
            &stage,
        )?;

        // Render to each eye
        let mut projection_views = Vec::new();

        for (eye_index, ((swapchain, images, width, height), xr_view)) in
            swapchain_data.iter_mut().zip(xr_views.iter()).enumerate()
        {
            // Acquire swapchain image
            let image_index = swapchain.acquire_image()?;
            swapchain.wait_image(xr::Duration::INFINITE)?;

            let image = images[image_index as usize];

            // Wait for previous frame's fence
            unsafe {
                vk_device.wait_for_fences(&[fence], true, u64::MAX)?;
                vk_device.reset_fences(&[fence])?;
            }

            // Use different colors for each eye to verify rendering
            let clear_color = if eye_index == 0 {
                ash::vk::ClearColorValue { float32: [1.0, 0.0, 0.0, 1.0] } // Left eye: Red
            } else {
                ash::vk::ClearColorValue { float32: [0.0, 0.0, 1.0, 1.0] } // Right eye: Blue
            };

            // Record command buffer to clear image
            unsafe {
                vk_device.reset_command_buffer(cmd, ash::vk::CommandBufferResetFlags::empty())?;

                let begin_info = ash::vk::CommandBufferBeginInfo::default()
                    .flags(ash::vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT);
                vk_device.begin_command_buffer(cmd, &begin_info)?;

                // Transition image to TRANSFER_DST
                let barrier = ash::vk::ImageMemoryBarrier::default()
                    .old_layout(ash::vk::ImageLayout::UNDEFINED)
                    .new_layout(ash::vk::ImageLayout::TRANSFER_DST_OPTIMAL)
                    .src_access_mask(ash::vk::AccessFlags::empty())
                    .dst_access_mask(ash::vk::AccessFlags::TRANSFER_WRITE)
                    .image(image)
                    .subresource_range(ash::vk::ImageSubresourceRange {
                        aspect_mask: ash::vk::ImageAspectFlags::COLOR,
                        base_mip_level: 0,
                        level_count: 1,
                        base_array_layer: 0,
                        layer_count: 1,
                    });

                vk_device.cmd_pipeline_barrier(
                    cmd,
                    ash::vk::PipelineStageFlags::TOP_OF_PIPE,
                    ash::vk::PipelineStageFlags::TRANSFER,
                    ash::vk::DependencyFlags::empty(),
                    &[],
                    &[],
                    &[barrier],
                );

                // Clear to color
                let range = ash::vk::ImageSubresourceRange {
                    aspect_mask: ash::vk::ImageAspectFlags::COLOR,
                    base_mip_level: 0,
                    level_count: 1,
                    base_array_layer: 0,
                    layer_count: 1,
                };
                vk_device.cmd_clear_color_image(
                    cmd,
                    image,
                    ash::vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                    &clear_color,
                    &[range],
                );

                // Transition image to COLOR_ATTACHMENT_OPTIMAL for OpenXR
                let barrier2 = ash::vk::ImageMemoryBarrier::default()
                    .old_layout(ash::vk::ImageLayout::TRANSFER_DST_OPTIMAL)
                    .new_layout(ash::vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
                    .src_access_mask(ash::vk::AccessFlags::TRANSFER_WRITE)
                    .dst_access_mask(ash::vk::AccessFlags::COLOR_ATTACHMENT_READ)
                    .image(image)
                    .subresource_range(ash::vk::ImageSubresourceRange {
                        aspect_mask: ash::vk::ImageAspectFlags::COLOR,
                        base_mip_level: 0,
                        level_count: 1,
                        base_array_layer: 0,
                        layer_count: 1,
                    });

                vk_device.cmd_pipeline_barrier(
                    cmd,
                    ash::vk::PipelineStageFlags::TRANSFER,
                    ash::vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT,
                    ash::vk::DependencyFlags::empty(),
                    &[],
                    &[],
                    &[barrier2],
                );

                vk_device.end_command_buffer(cmd)?;
            }

            // Submit command buffer and wait for completion before releasing
            let cmd_buffers = [cmd];
            let submit_info = ash::vk::SubmitInfo::default()
                .command_buffers(&cmd_buffers);
            unsafe {
                vk_device.queue_submit(vk_queue, &[submit_info], fence)?;
                // Wait for GPU to finish before releasing swapchain image
                vk_device.wait_for_fences(&[fence], true, u64::MAX)?;
            }

            swapchain.release_image()?;

            // Build projection view with actual pose and FOV from OpenXR
            projection_views.push(xr::CompositionLayerProjectionView::new()
                .pose(xr_view.pose)
                .fov(xr_view.fov)
                .sub_image(xr::SwapchainSubImage::new()
                    .swapchain(swapchain)
                    .image_rect(xr::Rect2Di {
                        offset: xr::Offset2Di { x: 0, y: 0 },
                        extent: xr::Extent2Di { width: *width as i32, height: *height as i32 },
                    })
                    .image_array_index(0)));
        }

        // Submit frame
        let projection_layer = xr::CompositionLayerProjection::new()
            .space(&stage)
            .views(&projection_views);

        frame_stream.end(frame_state.predicted_display_time, xr::EnvironmentBlendMode::OPAQUE, &[&projection_layer])?;

        frame_count += 1;
        if frame_count % 100 == 0 {
            log::info!("Frame {}", frame_count);
        }
    }

    // Cleanup
    unsafe {
        vk_device.device_wait_idle()?;
        vk_device.destroy_fence(fence, None);
        vk_device.destroy_command_pool(command_pool, None);
    }

    log::info!("=== XR loop ended ===");
    Ok(())
}

#[cfg(target_os = "android")]
unsafe fn initialize_meta_loader(
    vm: *mut std::ffi::c_void,
    activity: *mut std::ffi::c_void,
) -> Result<(), Box<dyn std::error::Error>> {
    use std::ffi::c_void;

    let lib = libloading::Library::new("libopenxr_loader.so")
        .map_err(|e| format!("Failed to load loader: {:?}", e))?;
    log::info!("Loaded libopenxr_loader.so");

    #[repr(C)]
    struct XrLoaderInitInfoAndroidKHR {
        ty: xr::sys::StructureType,
        next: *const c_void,
        application_vm: *mut c_void,
        application_context: *mut c_void,
    }

    type XrInitializeLoaderKHR = unsafe extern "C" fn(*const c_void) -> xr::sys::Result;

    let init_loader: Option<libloading::Symbol<XrInitializeLoaderKHR>> =
        lib.get(b"xrInitializeLoaderKHR").ok().or_else(|| {
            lib.get(b"_Z21xrInitializeLoaderKHRPK29XrLoaderInitInfoBaseHeaderKHR").ok()
        });

    if let Some(init_fn) = init_loader {
        let init_info = XrLoaderInitInfoAndroidKHR {
            ty: xr::sys::StructureType::LOADER_INIT_INFO_ANDROID_KHR,
            next: std::ptr::null(),
            application_vm: vm,
            application_context: activity,
        };
        let result = init_fn(&init_info as *const _ as *const c_void);
        if result == xr::sys::Result::SUCCESS {
            log::info!("xrInitializeLoaderKHR succeeded!");
        }
    }

    std::mem::forget(lib);
    Ok(())
}

#[cfg(target_os = "android")]
#[no_mangle]
fn android_main(app: AndroidApp) {
    android_logger::init_once(
        android_logger::Config::default()
            .with_max_level(log::LevelFilter::Info)
            .with_tag("quest-test"),
    );

    log::info!("=== Quest OpenXR Test Started ===");

    match run_xr_app(&app) {
        Ok(()) => log::info!("App exited normally"),
        Err(e) => log::error!("App error: {}", e),
    }

    log::info!("=== Quest OpenXR Test Ended ===");
}

#[cfg(not(target_os = "android"))]
pub fn run() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    log::info!("This app is designed for Quest. Build with: cargo apk build --lib");
}
