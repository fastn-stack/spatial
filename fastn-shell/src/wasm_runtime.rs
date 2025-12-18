//! WASM runtime using wasmtime
//!
//! The WASM module must export:
//! - `init_core() -> app_ptr` - Initialize and return app pointer
//! - `get_result_ptr(app_ptr) -> ptr` - Get pointer to result JSON
//! - `get_result_len(app_ptr) -> len` - Get length of result JSON
//! - `on_event(app_ptr, event_ptr, event_len) -> ptr` - Process event
//! - `alloc(size) -> ptr` - Allocate memory in WASM
//! - `dealloc(ptr, size)` - Deallocate memory in WASM

use fastn_protocol::{Command, Event};
use wasmtime::*;

pub struct WasmCore {
    store: Store<()>,
    memory: Memory,
    app_ptr: i32,
    alloc: TypedFunc<i32, i32>,
    on_event: TypedFunc<(i32, i32, i32), i32>,
    get_result_ptr: TypedFunc<i32, i32>,
    get_result_len: TypedFunc<i32, i32>,
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

        let alloc = instance
            .get_typed_func::<i32, i32>(&mut store, "alloc")?;

        let on_event = instance
            .get_typed_func::<(i32, i32, i32), i32>(&mut store, "on_event")?;

        let get_result_ptr = instance
            .get_typed_func::<i32, i32>(&mut store, "get_result_ptr")?;

        let get_result_len = instance
            .get_typed_func::<i32, i32>(&mut store, "get_result_len")?;

        // Initialize the core and get app pointer
        let app_ptr = init_core.call(&mut store, ())?;

        // Read initial commands from result buffer
        let result_ptr = get_result_ptr.call(&mut store, app_ptr)?;
        let result_len = get_result_len.call(&mut store, app_ptr)?;

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
            app_ptr,
            alloc,
            on_event,
            get_result_ptr,
            get_result_len,
        };

        Ok((core, commands))
    }

    /// Send an event to the WASM core and get back commands
    pub fn send_event(&mut self, event: &Event) -> Result<Vec<Command>, Box<dyn std::error::Error>> {
        // Serialize the event to JSON
        let event_json = serde_json::to_string(event)?;
        let event_bytes = event_json.as_bytes();
        let event_len = event_bytes.len() as i32;

        // Allocate memory in WASM for the event
        let event_ptr = self.alloc.call(&mut self.store, event_len)?;

        // Write the event JSON to WASM memory
        self.memory.data_mut(&mut self.store)[event_ptr as usize..(event_ptr as usize + event_len as usize)]
            .copy_from_slice(event_bytes);

        // Call on_event with app pointer
        let _result_ptr = self.on_event.call(&mut self.store, (self.app_ptr, event_ptr, event_len))?;
        let result_len = self.get_result_len.call(&mut self.store, self.app_ptr)?;

        // Read the commands from WASM memory
        let commands = if result_len > 0 {
            let result_ptr = self.get_result_ptr.call(&mut self.store, self.app_ptr)?;
            let mem_data = self.memory.data(&self.store);
            let result_bytes = &mem_data[result_ptr as usize..(result_ptr as usize + result_len as usize)];
            let result_json = std::str::from_utf8(result_bytes)?;
            serde_json::from_str::<Vec<Command>>(result_json)?
        } else {
            vec![]
        };

        Ok(commands)
    }
}
