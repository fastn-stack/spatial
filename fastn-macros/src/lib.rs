//! Proc macros for fastn

use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, ItemFn};

/// Marks a function as the fastn app entry point.
///
/// This attribute generates the necessary FFI exports for WASM.
/// The function receives a `&mut RealityViewContent` to populate with entities.
///
/// ## WASM API
///
/// The generated exports follow a handle-based pattern:
/// - `init_core() -> app_ptr` - Create app, returns pointer (as i32)
/// - `get_result_ptr(app_ptr) -> ptr` - Get pointer to result JSON
/// - `get_result_len(app_ptr) -> len` - Get length of result JSON
/// - `on_event(app_ptr, event_ptr, event_len) -> ptr` - Process event, returns result ptr
/// - `alloc(size) -> ptr` - Allocate memory for shell to write into
/// - `dealloc(ptr, size)` - Free allocated memory
///
/// # Example
///
/// ```rust,ignore
/// use fastn::{ModelEntity, MeshResource, SimpleMaterial, RealityViewContent};
///
/// #[fastn::app]
/// fn make_content(content: &mut RealityViewContent) {
///     let cube = ModelEntity::new(
///         MeshResource::generate_box(0.5),
///         SimpleMaterial::new().color(1.0, 0.0, 0.0)
///     );
///     content.add(cube);
/// }
/// ```
#[proc_macro_attribute]
pub fn app(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let input_fn = parse_macro_input!(item as ItemFn);
    let fn_name = &input_fn.sig.ident;

    let expanded = quote! {
        #input_fn

        /// Create the app and return its pointer.
        /// Call get_result_ptr/get_result_len to read initial commands.
        #[unsafe(no_mangle)]
        pub extern "C" fn init_core() -> i32 {
            let mut content = fastn::RealityViewContent::new();
            #fn_name(&mut content);
            fastn::wasm_bridge::create_app(&content) as i32
        }

        /// Get pointer to the result buffer (initial commands or last on_event result)
        #[unsafe(no_mangle)]
        pub extern "C" fn get_result_ptr(app_ptr: i32) -> i32 {
            unsafe { fastn::wasm_bridge::get_result_ptr(app_ptr as *const fastn::wasm_bridge::CoreApp) as i32 }
        }

        /// Get length of the result buffer
        #[unsafe(no_mangle)]
        pub extern "C" fn get_result_len(app_ptr: i32) -> i32 {
            unsafe { fastn::wasm_bridge::get_result_len(app_ptr as *const fastn::wasm_bridge::CoreApp) as i32 }
        }

        /// Process an event. Returns pointer to result JSON.
        #[unsafe(no_mangle)]
        pub extern "C" fn on_event(app_ptr: i32, event_ptr: i32, event_len: i32) -> i32 {
            unsafe {
                fastn::wasm_bridge::app_on_event(
                    app_ptr as *mut fastn::wasm_bridge::CoreApp,
                    event_ptr as *const u8,
                    event_len as usize
                ) as i32
            }
        }

        #[unsafe(no_mangle)]
        pub extern "C" fn alloc(size: i32) -> i32 {
            fastn::wasm_bridge::alloc(size as usize) as i32
        }

        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn dealloc(ptr: i32, size: i32) {
            unsafe { fastn::wasm_bridge::dealloc(ptr as *mut u8, size as usize) }
        }
    };

    TokenStream::from(expanded)
}
