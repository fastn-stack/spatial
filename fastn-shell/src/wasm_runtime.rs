//! WASM runtime using wasmtime
//!
//! The WASM module must export:
//! - `init_core()` - Initialize the core
//! - `handle_event(event_json_ptr, event_json_len) -> commands_json_ptr`
//! - `alloc(size) -> ptr` - Allocate memory in WASM
//! - `dealloc(ptr, size)` - Deallocate memory in WASM

use fastn::{Command, Event};
use wasmtime::*;

pub struct WasmCore {
    store: Store<()>,
    instance: Instance,
    memory: Memory,
    // Function handles
    init_core: TypedFunc<(), ()>,
    handle_event: TypedFunc<(i32, i32), i32>,
    alloc: TypedFunc<i32, i32>,
    dealloc: TypedFunc<(i32, i32), ()>,
    get_result_len: TypedFunc<(), i32>,
}

impl WasmCore {
    pub fn new(wasm_path: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let engine = Engine::default();
        let module = Module::from_file(&engine, wasm_path)?;
        let mut store = Store::new(&engine, ());

        let instance = Instance::new(&mut store, &module, &[])?;

        let memory = instance
            .get_memory(&mut store, "memory")
            .ok_or("WASM module must export 'memory'")?;

        let init_core = instance
            .get_typed_func::<(), ()>(&mut store, "init_core")?;

        let handle_event = instance
            .get_typed_func::<(i32, i32), i32>(&mut store, "handle_event")?;

        let alloc = instance
            .get_typed_func::<i32, i32>(&mut store, "alloc")?;

        let dealloc = instance
            .get_typed_func::<(i32, i32), ()>(&mut store, "dealloc")?;

        let get_result_len = instance
            .get_typed_func::<(), i32>(&mut store, "get_result_len")?;

        let mut core = Self {
            store,
            instance,
            memory,
            init_core,
            handle_event,
            alloc,
            dealloc,
            get_result_len,
        };

        // Initialize the core
        core.init_core.call(&mut core.store, ())?;
        log::info!("WASM core initialized");

        Ok(core)
    }

    pub fn handle_event(&mut self, event: &Event) -> Vec<Command> {
        // Serialize event to JSON
        let event_json = match serde_json::to_string(event) {
            Ok(json) => json,
            Err(e) => {
                log::error!("Failed to serialize event: {}", e);
                return vec![];
            }
        };
        log::debug!("Sending event JSON: {}", event_json);

        let event_bytes = event_json.as_bytes();
        let event_len = event_bytes.len() as i32;

        // Allocate memory in WASM for the event
        let event_ptr = match self.alloc.call(&mut self.store, event_len) {
            Ok(ptr) => ptr,
            Err(e) => {
                log::error!("Failed to allocate WASM memory: {}", e);
                return vec![];
            }
        };

        // Copy event JSON into WASM memory
        {
            let mem_data = self.memory.data_mut(&mut self.store);
            mem_data[event_ptr as usize..(event_ptr as usize + event_len as usize)]
                .copy_from_slice(event_bytes);
        }

        // Call handle_event
        let result_ptr = match self.handle_event.call(&mut self.store, (event_ptr, event_len)) {
            Ok(ptr) => ptr,
            Err(e) => {
                log::error!("Failed to call handle_event: {}", e);
                // Deallocate the event memory
                let _ = self.dealloc.call(&mut self.store, (event_ptr, event_len));
                return vec![];
            }
        };

        // Deallocate event memory
        let _ = self.dealloc.call(&mut self.store, (event_ptr, event_len));

        // Get result length
        let result_len = match self.get_result_len.call(&mut self.store, ()) {
            Ok(len) => len,
            Err(e) => {
                log::error!("Failed to get result length: {}", e);
                return vec![];
            }
        };

        if result_len <= 0 {
            return vec![];
        }

        // Read result JSON from WASM memory and parse it
        // NOTE: result buffer is owned by WASM's thread-local, NOT allocated via alloc()
        // So we should NOT deallocate it
        let result_json: String = {
            let mem_data = self.memory.data(&self.store);
            let result_bytes = &mem_data[result_ptr as usize..(result_ptr as usize + result_len as usize)];
            match std::str::from_utf8(result_bytes) {
                Ok(s) => s.to_string(),
                Err(e) => {
                    log::error!("Invalid UTF-8 in result: {}", e);
                    return vec![];
                }
            }
        };

        // Parse commands
        match serde_json::from_str::<Vec<Command>>(&result_json) {
            Ok(commands) => commands,
            Err(e) => {
                log::error!("Failed to parse commands: {} - JSON: {}", e, result_json);
                vec![]
            }
        }
    }
}
