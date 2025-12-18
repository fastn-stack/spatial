//! WASM Bridge - FFI helpers for shell-core communication
//!
//! This module provides helper functions used by the `#[fastn::app]` proc macro
//! to generate the necessary FFI exports.

use std::cell::RefCell;
use crate::camera::CameraController;
use crate::protocol::{Command, Event};

thread_local! {
    /// Result buffer for returning JSON to the shell.
    pub static RESULT_BUFFER: RefCell<Vec<u8>> = const { RefCell::new(Vec::new()) };

    /// Camera controller instance
    pub static CAMERA: RefCell<CameraController> = RefCell::new(CameraController::new());
}

/// Store RealityViewContent as JSON commands
#[doc(hidden)]
pub fn store_content(content: &crate::RealityViewContent) {
    let commands = content.to_commands();
    let json = serde_json::to_string(&commands).unwrap_or_else(|_| "[]".to_string());
    RESULT_BUFFER.with(|buf| {
        let mut buf = buf.borrow_mut();
        buf.clear();
        buf.extend_from_slice(json.as_bytes());
    });
}

/// Get pointer to the result buffer
#[doc(hidden)]
pub fn get_result_ptr() -> *const u8 {
    RESULT_BUFFER.with(|buf| buf.borrow().as_ptr())
}

/// Get length of the result buffer
#[doc(hidden)]
pub fn get_result_len() -> usize {
    RESULT_BUFFER.with(|buf| buf.borrow().len())
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

/// Process an event from the shell and return commands
/// The event is passed as a JSON string in WASM memory
#[doc(hidden)]
pub fn on_event(event_ptr: *const u8, event_len: usize) -> *const u8 {
    // Parse the event JSON from WASM memory
    let event_bytes = unsafe { std::slice::from_raw_parts(event_ptr, event_len) };
    let event_json = match std::str::from_utf8(event_bytes) {
        Ok(s) => s,
        Err(_) => {
            store_commands(&[]);
            return get_result_ptr();
        }
    };

    let event: Event = match serde_json::from_str(event_json) {
        Ok(e) => e,
        Err(_) => {
            store_commands(&[]);
            return get_result_ptr();
        }
    };

    // Process the event through the camera controller
    let commands = CAMERA.with(|cam| {
        cam.borrow_mut().handle_event(&event)
    });

    store_commands(&commands);
    get_result_ptr()
}

/// Store commands as JSON in the result buffer
#[doc(hidden)]
pub fn store_commands(commands: &[Command]) {
    let json = serde_json::to_string(commands).unwrap_or_else(|_| "[]".to_string());
    RESULT_BUFFER.with(|buf| {
        let mut buf = buf.borrow_mut();
        buf.clear();
        buf.extend_from_slice(json.as_bytes());
    });
}
