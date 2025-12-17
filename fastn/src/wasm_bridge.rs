//! WASM Bridge - FFI exports for shell-core communication
//!
//! This module provides the `app!` macro that generates the necessary
//! FFI exports for a fastn application.
//!
//! # Usage
//!
//! ```rust,ignore
//! use fastn::{app, Event, Command};
//!
//! struct MyApp;
//!
//! impl MyApp {
//!     fn new() -> Self { Self }
//!
//!     fn handle(&mut self, event: Event) -> Vec<Command> {
//!         vec![]
//!     }
//! }
//!
//! app!(MyApp);
//! ```

use std::cell::RefCell;

thread_local! {
    /// Result buffer for returning JSON to the shell.
    /// This is owned by WASM, NOT allocated via alloc().
    pub static RESULT_BUFFER: RefCell<Vec<u8>> = const { RefCell::new(Vec::new()) };
}

/// Store an empty result (empty JSON array)
#[doc(hidden)]
pub fn store_empty_result() {
    RESULT_BUFFER.with(|buf| {
        let mut buf = buf.borrow_mut();
        buf.clear();
        buf.extend_from_slice(b"[]");
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

/// Handle an event - parse JSON, call app.handle(), store result
#[doc(hidden)]
pub fn handle_event_impl<T>(app: &mut T, event_ptr: *const u8, event_len: usize) -> *const u8
where
    T: FnMut(crate::Event) -> Vec<crate::Command>,
{
    let event_bytes = unsafe { std::slice::from_raw_parts(event_ptr, event_len) };

    let event_json = match std::str::from_utf8(event_bytes) {
        Ok(s) => s,
        Err(_) => {
            store_empty_result();
            return get_result_ptr();
        }
    };

    let event: crate::Event = match serde_json::from_str(event_json) {
        Ok(e) => e,
        Err(_) => {
            store_empty_result();
            return get_result_ptr();
        }
    };

    let commands = app(event);

    let json = match serde_json::to_string(&commands) {
        Ok(j) => j,
        Err(_) => "[]".to_string(),
    };

    RESULT_BUFFER.with(|buf| {
        let mut buf = buf.borrow_mut();
        buf.clear();
        buf.extend_from_slice(json.as_bytes());
    });

    get_result_ptr()
}

/// Generates FFI exports for a fastn application.
///
/// The type must implement:
/// - `fn new() -> Self`
/// - `fn handle(&mut self, event: Event) -> Vec<Command>`
///
/// # Example
///
/// ```rust,ignore
/// use fastn::{app, Event, Command};
///
/// struct MyApp;
///
/// impl MyApp {
///     fn new() -> Self { Self }
///     fn handle(&mut self, event: Event) -> Vec<Command> { vec![] }
/// }
///
/// app!(MyApp);
/// ```
#[macro_export]
macro_rules! app {
    ($app_type:ty) => {
        use std::cell::RefCell;

        thread_local! {
            static APP: RefCell<Option<$app_type>> = const { RefCell::new(None) };
        }

        #[unsafe(no_mangle)]
        pub extern "C" fn init_core() {
            APP.with(|app| {
                *app.borrow_mut() = Some(<$app_type>::new());
            });
        }

        #[unsafe(no_mangle)]
        pub extern "C" fn handle_event(event_ptr: *const u8, event_len: usize) -> *const u8 {
            APP.with(|app| {
                let mut app_ref = app.borrow_mut();
                if let Some(ref mut app) = *app_ref {
                    // Create a closure that calls app.handle()
                    let mut handler = |event: $crate::Event| -> Vec<$crate::Command> {
                        app.handle(event)
                    };
                    $crate::wasm_bridge::handle_event_impl(&mut handler, event_ptr, event_len)
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
