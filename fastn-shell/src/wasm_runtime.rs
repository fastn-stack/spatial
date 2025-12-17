//! WASM runtime using wasmtime
//!
//! The WASM module must export:
//! - `init_core() -> commands_json_ptr` - Initialize and return initial commands
//! - `alloc(size) -> ptr` - Allocate memory in WASM
//! - `dealloc(ptr, size)` - Deallocate memory in WASM
//! - `get_result_len() -> len` - Get length of result buffer

use fastn::Command;
use wasmtime::*;

pub struct WasmCore {
    store: Store<()>,
    memory: Memory,
    get_result_len: TypedFunc<(), i32>,
}

impl WasmCore {
    pub fn new(wasm_path: &str) -> Result<(Self, Vec<Command>), Box<dyn std::error::Error>> {
        let engine = Engine::default();
        let module = Module::from_file(&engine, wasm_path)?;
        let mut store = Store::new(&engine, ());

        let instance = Instance::new(&mut store, &module, &[])?;

        let memory = instance
            .get_memory(&mut store, "memory")
            .ok_or("WASM module must export 'memory'")?;

        let init_core = instance
            .get_typed_func::<(), i32>(&mut store, "init_core")?;

        let get_result_len = instance
            .get_typed_func::<(), i32>(&mut store, "get_result_len")?;

        // Initialize the core and get initial commands
        let result_ptr = init_core.call(&mut store, ())?;
        let result_len = get_result_len.call(&mut store, ())?;

        let commands = if result_len > 0 {
            let mem_data = memory.data(&store);
            let result_bytes = &mem_data[result_ptr as usize..(result_ptr as usize + result_len as usize)];
            let result_json = std::str::from_utf8(result_bytes)?;
            log::debug!("Init commands JSON: {}", result_json);
            serde_json::from_str::<Vec<Command>>(result_json)?
        } else {
            vec![]
        };

        log::info!("WASM core initialized with {} commands", commands.len());

        let core = Self {
            store,
            memory,
            get_result_len,
        };

        Ok((core, commands))
    }
}
