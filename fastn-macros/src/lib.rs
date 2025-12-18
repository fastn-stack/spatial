//! Proc macros for fastn

use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, ItemFn};

/// Marks a function as the fastn app entry point.
///
/// This attribute generates the necessary FFI exports for WASM.
/// The function receives a `&mut RealityViewContent` to populate with entities.
///
/// # Example (matching visionOS RealityView pattern)
///
/// ```rust,ignore
/// use fastn::{ModelEntity, MeshResource, SimpleMaterial, RealityViewContent};
///
/// #[fastn::app]
/// fn make_content(content: &mut RealityViewContent) {
///     // Equivalent to Swift:
///     // let box = ModelEntity(mesh: .generateBox(size: 0.5),
///     //                       materials: [SimpleMaterial(color: .red, isMetallic: false)])
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

    // Generate the FFI exports alongside the original function
    let expanded = quote! {
        #input_fn

        #[unsafe(no_mangle)]
        pub extern "C" fn init_core() -> *const u8 {
            let mut content = fastn::RealityViewContent::new();
            #fn_name(&mut content);
            fastn::wasm_bridge::store_content(&content);
            fastn::wasm_bridge::get_result_ptr()
        }

        #[unsafe(no_mangle)]
        pub extern "C" fn on_event(event_ptr: *const u8, event_len: usize) -> *const u8 {
            fastn::wasm_bridge::on_event(event_ptr, event_len)
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
