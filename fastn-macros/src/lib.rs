//! Proc macros for fastn

use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, ItemFn};

/// Marks a function as the fastn app entry point.
///
/// This attribute generates the necessary FFI exports for WASM.
/// The function must have the signature `fn init() -> fastn::App`.
///
/// # Example
///
/// ```rust,ignore
/// #[fastn::app]
/// fn init() -> fastn::App {
///     let mut app = fastn::init();
///     app.add_volume_from_glb("cube.glb", 0);
///     app
/// }
/// ```
#[proc_macro_attribute]
pub fn app(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let input_fn = parse_macro_input!(item as ItemFn);
    let fn_name = &input_fn.sig.ident;

    // Generate the FFI exports alongside the original function
    let expanded = quote! {
        #input_fn

        #[unsafe(no_mangle)]
        pub extern "C" fn init_core() -> *const u8 {
            let app = #fn_name();
            fastn::wasm_bridge::store_commands(app.commands());
            fastn::wasm_bridge::get_result_ptr()
        }

        #[unsafe(no_mangle)]
        pub extern "C" fn alloc(size: usize) -> *mut u8 {
            fastn::wasm_bridge::alloc(size)
        }

        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn dealloc(ptr: *mut u8, size: usize) {
            unsafe { fastn::wasm_bridge::dealloc(ptr, size) }
        }

        #[unsafe(no_mangle)]
        pub extern "C" fn get_result_len() -> usize {
            fastn::wasm_bridge::get_result_len()
        }
    };

    TokenStream::from(expanded)
}
