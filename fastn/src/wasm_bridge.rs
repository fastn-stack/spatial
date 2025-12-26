//! WASM Bridge - FFI helpers for shell-core communication
//!
//! This module provides helper functions used by the `#[fastn::app]` proc macro
//! to generate the necessary FFI exports.
//!
//! Design: No global state. The shell owns a pointer to CoreApp which holds all state.

use crate::camera::CameraController;
use fastn_protocol::{Command, Event};

/// The core application state that the shell owns.
/// This struct holds all state - no thread-locals or globals.
pub struct CoreApp {
    /// Camera controller for default input handling
    camera: CameraController,
    /// Result buffer for returning JSON to the shell
    result_buffer: Vec<u8>,
}

impl CoreApp {
    /// Create a new CoreApp and populate initial commands
    pub fn new(content: &crate::RealityViewContent) -> Box<Self> {
        let commands = content.to_commands();
        let mut app = Box::new(Self {
            camera: CameraController::new(),
            result_buffer: Vec::new(),
        });
        // Store initial commands in result buffer
        app.store_commands_internal(&commands);
        app
    }

    /// Process an event and return commands
    pub fn on_event(&mut self, event: &Event) -> Vec<Command> {
        self.camera.handle_event(event)
    }

    /// Store commands as JSON in the result buffer
    fn store_commands_internal(&mut self, commands: &[Command]) {
        let json = serde_json::to_string(commands).unwrap_or_else(|_| "[]".to_string());
        self.result_buffer.clear();
        self.result_buffer.extend_from_slice(json.as_bytes());
    }

    /// Get pointer to result buffer
    pub fn result_ptr(&self) -> *const u8 {
        self.result_buffer.as_ptr()
    }

    /// Get length of result buffer
    pub fn result_len(&self) -> usize {
        self.result_buffer.len()
    }
}

// FFI functions that work with CoreApp pointer

/// Create a CoreApp from RealityViewContent
/// Returns app pointer. Call get_result_ptr/get_result_len to get initial commands.
#[doc(hidden)]
pub fn create_app(content: &crate::RealityViewContent) -> *mut CoreApp {
    Box::into_raw(CoreApp::new(content))
}

/// Get pointer to the result buffer (initial commands or last on_event result)
///
/// # Safety
/// `app_ptr` must be a valid pointer returned by `create_app` and not yet destroyed.
#[doc(hidden)]
pub unsafe fn get_result_ptr(app_ptr: *const CoreApp) -> *const u8 {
    let app = unsafe { &*app_ptr };
    app.result_ptr()
}

/// Get length of the result buffer
///
/// # Safety
/// `app_ptr` must be a valid pointer returned by `create_app` and not yet destroyed.
#[doc(hidden)]
pub unsafe fn get_result_len(app_ptr: *const CoreApp) -> usize {
    let app = unsafe { &*app_ptr };
    app.result_len()
}

/// Process an event on the CoreApp
/// Returns pointer to commands JSON. Call get_result_len for length.
///
/// # Safety
/// - `app_ptr` must be a valid pointer returned by `create_app` and not yet destroyed.
/// - `event_ptr` must be a valid pointer to `event_len` bytes of valid memory.
#[doc(hidden)]
pub unsafe fn app_on_event(app_ptr: *mut CoreApp, event_ptr: *const u8, event_len: usize) -> *const u8 {
    let app = unsafe { &mut *app_ptr };

    // Parse the event JSON
    let event_bytes = unsafe { std::slice::from_raw_parts(event_ptr, event_len) };
    let event_json = match std::str::from_utf8(event_bytes) {
        Ok(s) => s,
        Err(_) => {
            app.store_commands_internal(&[]);
            return app.result_ptr();
        }
    };

    let event: Event = match serde_json::from_str(event_json) {
        Ok(e) => e,
        Err(_) => {
            app.store_commands_internal(&[]);
            return app.result_ptr();
        }
    };

    let commands = app.on_event(&event);
    app.store_commands_internal(&commands);
    app.result_ptr()
}

/// Destroy a CoreApp (call when done)
///
/// # Safety
/// `app_ptr` must be a valid pointer returned by `create_app` and not yet destroyed,
/// or null (which is a no-op).
#[doc(hidden)]
pub unsafe fn destroy_app(app_ptr: *mut CoreApp) {
    if !app_ptr.is_null() {
        unsafe { drop(Box::from_raw(app_ptr)); }
    }
}

/// Allocate memory in WASM for the shell to write into
#[doc(hidden)]
pub fn alloc(size: usize) -> *mut u8 {
    let mut buf = Vec::with_capacity(size);
    let ptr = buf.as_mut_ptr();
    std::mem::forget(buf);
    ptr
}

/// Deallocate memory that was allocated via `alloc`
#[doc(hidden)]
pub unsafe fn dealloc(ptr: *mut u8, size: usize) {
    unsafe { let _ = Vec::from_raw_parts(ptr, 0, size); }
}
