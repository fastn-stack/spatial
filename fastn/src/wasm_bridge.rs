//! WASM Bridge - Provides exported functions for shell-core communication
//!
//! This module provides macros and helpers for creating WASM applications
//! that communicate with fastn-shell. Users typically use the `fastn_app!` macro
//! to set up all the necessary exports.
//!
//! # Example
//!
//! ```rust,ignore
//! use fastn::{fastn_app, Command, Event};
//!
//! struct MyApp {
//!     // your app state
//! }
//!
//! impl fastn::Core for MyApp {
//!     fn handle(&mut self, event: Event) -> Vec<Command> {
//!         // handle events and return commands
//!         vec![]
//!     }
//! }
//!
//! fastn_app!(MyApp, MyApp::new());
//! ```

use std::cell::RefCell;
use crate::{Command, Core, Event};

thread_local! {
    /// Result buffer for returning JSON to the shell
    /// This is NOT allocated via alloc(), so the shell should NOT deallocate it
    pub static RESULT_BUFFER: RefCell<Vec<u8>> = const { RefCell::new(Vec::new()) };
}

/// Store an empty result (empty JSON array)
pub fn store_empty_result() {
    RESULT_BUFFER.with(|buf| {
        let mut buf = buf.borrow_mut();
        buf.clear();
        buf.extend_from_slice(b"[]");
    });
}

/// Get pointer to the result buffer
pub fn get_result_ptr() -> *const u8 {
    RESULT_BUFFER.with(|buf| buf.borrow().as_ptr())
}

/// Get length of the result buffer
pub fn get_result_len() -> usize {
    RESULT_BUFFER.with(|buf| buf.borrow().len())
}

/// Store commands as JSON in the result buffer
pub fn store_commands(commands: &[Command]) {
    let json = match serde_json::to_string(commands) {
        Ok(json) => json,
        Err(_) => "[]".to_string(),
    };
    RESULT_BUFFER.with(|buf| {
        let mut buf = buf.borrow_mut();
        buf.clear();
        buf.extend_from_slice(json.as_bytes());
    });
}

/// Handle an event by parsing JSON, calling the core, and storing the result
pub fn handle_event_impl<C: Core>(core: &mut C, event_ptr: *const u8, event_len: usize) -> *const u8 {
    // Read event JSON from memory
    let event_bytes = unsafe {
        std::slice::from_raw_parts(event_ptr, event_len)
    };

    let event_json = match std::str::from_utf8(event_bytes) {
        Ok(s) => s,
        Err(_) => {
            store_empty_result();
            return get_result_ptr();
        }
    };

    // Parse event
    let event: Event = match serde_json::from_str(event_json) {
        Ok(e) => e,
        Err(_) => {
            store_empty_result();
            return get_result_ptr();
        }
    };

    // Handle event
    let commands = core.handle(event);

    // Store result
    store_commands(&commands);
    get_result_ptr()
}

/// Allocate memory in WASM for the shell to write into
///
/// # Safety
/// The caller must ensure that:
/// - The returned pointer is only used for writing `size` bytes
/// - The memory is deallocated using `dealloc` when no longer needed
pub fn alloc(size: usize) -> *mut u8 {
    let mut buf = Vec::with_capacity(size);
    let ptr = buf.as_mut_ptr();
    std::mem::forget(buf);
    ptr
}

/// Deallocate memory that was allocated via `alloc`
///
/// # Safety
/// The caller must ensure that:
/// - `ptr` was returned from a previous call to `alloc`
/// - `size` matches the size passed to `alloc`
/// - The memory is not used after this call
pub unsafe fn dealloc(ptr: *mut u8, size: usize) {
    unsafe { let _ = Vec::from_raw_parts(ptr, 0, size); }
}

/// Macro to generate WASM exports for a fastn application
///
/// This macro creates the necessary exported functions that fastn-shell
/// expects: `init_core`, `handle_event`, `alloc`, `dealloc`, `get_result_len`
///
/// # Example
///
/// ```rust,ignore
/// use fastn::{fastn_app, Core, Command, Event};
///
/// struct MyApp;
///
/// impl Core for MyApp {
///     fn handle(&mut self, event: Event) -> Vec<Command> {
///         vec![]
///     }
/// }
///
/// fastn_app!(MyApp, MyApp::default());
/// ```
#[macro_export]
macro_rules! fastn_app {
    ($core_type:ty, $init_expr:expr) => {
        use std::cell::RefCell;

        thread_local! {
            static CORE: RefCell<Option<$core_type>> = const { RefCell::new(None) };
        }

        #[unsafe(no_mangle)]
        pub extern "C" fn init_core() {
            CORE.with(|core| {
                *core.borrow_mut() = Some($init_expr);
            });
        }

        #[unsafe(no_mangle)]
        pub extern "C" fn handle_event(event_ptr: *const u8, event_len: usize) -> *const u8 {
            CORE.with(|core| {
                let mut core_ref = core.borrow_mut();
                if let Some(ref mut core) = *core_ref {
                    $crate::wasm_bridge::handle_event_impl(core, event_ptr, event_len)
                } else {
                    $crate::wasm_bridge::store_empty_result();
                    $crate::wasm_bridge::get_result_ptr()
                }
            })
        }

        #[unsafe(no_mangle)]
        pub extern "C" fn alloc(size: usize) -> *mut u8 {
            $crate::wasm_bridge::alloc(size)
        }

        #[unsafe(no_mangle)]
        pub extern "C" fn dealloc(ptr: *mut u8, size: usize) {
            unsafe { $crate::wasm_bridge::dealloc(ptr, size) }
        }

        #[unsafe(no_mangle)]
        pub extern "C" fn get_result_len() -> usize {
            $crate::wasm_bridge::get_result_len()
        }
    };
}
