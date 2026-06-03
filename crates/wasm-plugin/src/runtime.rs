use crate::types::{MemoryLimits, PluginResult};
use rustai_core::error::{GatewayError, GatewayResult};
use std::sync::Arc;
use std::time::Instant;
use tracing::{debug, error, info, warn};
use wasmtime::{
    Engine, Func, Linker, Memory, Module, Store, TypedFunc, ValType,
};
use wasmtime_wasi::WasiCtxBuilder;

/// A single Wasm plugin instance
pub struct WasmPluginInstance {
    name: String,
    engine: Engine,
    module: Module,
    memory_limits: MemoryLimits,
}

impl WasmPluginInstance {
    /// Create a new Wasm plugin instance from a .wasm file
    pub fn from_file(
        name: &str,
        wasm_path: &str,
        memory_limits: MemoryLimits,
    ) -> GatewayResult<Self> {
        let engine_config = wasmtime::Config::new();
        let engine = Engine::new(&engine_config)
            .map_err(|e| GatewayError::WasmPlugin(format!("Engine creation error: {e}")))?;

        let module = Module::from_file(&engine, wasm_path)
            .map_err(|e| GatewayError::WasmPlugin(format!("Module load error: {e}")))?;

        info!(
            plugin_name = %name,
            wasm_path = %wasm_path,
            "Loaded Wasm plugin module"
        );

        Ok(Self {
            name: name.to_string(),
            engine,
            module,
            memory_limits,
        })
    }

    /// Create a new Wasm plugin instance from bytes
    pub fn from_bytes(
        name: &str,
        wasm_bytes: &[u8],
        memory_limits: MemoryLimits,
    ) -> GatewayResult<Self> {
        let engine_config = wasmtime::Config::new();
        let engine = Engine::new(&engine_config)
            .map_err(|e| GatewayError::WasmPlugin(format!("Engine creation error: {e}")))?;

        let module = Module::new(&engine, wasm_bytes)
            .map_err(|e| GatewayError::WasmPlugin(format!("Module compile error: {e}")))?;

        info!(plugin_name = %name, "Loaded Wasm plugin module from bytes");

        Ok(Self {
            name: name.to_string(),
            engine,
            module,
            memory_limits,
        })
    }

    /// Execute the plugin's `process_request` function
    pub async fn execute(
        &self,
        request_payload: &str,
        config: &str,
    ) -> GatewayResult<PluginResult> {
        let start = Instant::now();

        // Build WASI context
        let wasi_ctx = WasiCtxBuilder::new()
            .inherit_stderr()
            .build();

        let mut store = Store::new(&self.engine, wasi_ctx);

        // Create linker
        let mut linker = Linker::new(&self.engine);
        wasmtime_wasi::add_to_linker_sync(&mut linker, |s| s)
            .map_err(|e| GatewayError::WasmPlugin(format!("WASI linker error: {e}")))?;

        // Link the module
        let instance = linker
            .instantiate(&mut store, &self.module)
            .map_err(|e| GatewayError::WasmPlugin(format!("Instance creation error: {e}")))?;

        // Get the process_request export
        let process = instance
            .get_typed_func::<(i32, i32, i32, i32), i32>(&mut store, "process_request")
            .map_err(|e| {
                warn!(
                    plugin_name = %self.name,
                    "process_request export not found: {e}"
                );
                // Fallback: if no process_request, return default allow
                return GatewayError::WasmPlugin(format!("process_request not exported: {e}"));
            })?;

        // Get memory for passing data
        let memory = instance
            .get_memory(&mut store, "memory")
            .ok_or_else(|| GatewayError::WasmPlugin("Memory export not found".into()))?;

        // Write request payload and config into Wasm memory
        let request_offset = self.write_string(&mut store, &memory, request_payload)?;
        let config_offset = self.write_string(&mut store, &memory, config)?;

        // Call the plugin function with timeout
        let result = tokio::time::timeout(
            std::time::Duration::from_secs(self.memory_limits.max_execution_secs),
            async {
                process.call(
                    &mut store,
                    (request_offset, request_payload.len() as i32, config_offset, config.len() as i32),
                )
            },
        )
        .await
        .map_err(|_| GatewayError::WasmPlugin("Plugin execution timed out".into()))?
        .map_err(|e| GatewayError::WasmPlugin(format!("Plugin execution error: {e}")))?;

        // Read result from Wasm memory
        let result_str = self.read_string(&mut store, &memory, result)?;

        let duration = start.elapsed();
        debug!(
            plugin_name = %self.name,
            duration_us = duration.as_micros(),
            "Wasm plugin executed"
        );

        serde_json::from_str(&result_str)
            .map_err(|e| GatewayError::WasmPlugin(format!("Result parse error: {e}")))
    }

    /// Write a string into Wasm linear memory, returns the offset
    fn write_string(
        &self,
        store: &mut Store<wasmtime_wasi::WasiCtx>,
        memory: &Memory,
        s: &str,
    ) -> GatewayResult<i32> {
        let offset = memory
            .data_size(&store)
            .checked_add(8) // alignment
            .ok_or_else(|| GatewayError::WasmPlugin("Memory overflow".into()))? as i32;

        // Grow memory if needed
        let needed_pages = ((offset as u64 + s.len() as u64) / 65536) + 1;
        let current_pages = memory.size(&store);
        if needed_pages > current_pages as u64 {
            memory
                .grow(&mut *store, (needed_pages - current_pages as u64) as u64)
                .map_err(|e| GatewayError::WasmPlugin(format!("Memory grow error: {e}")))?;
        }

        // Write string bytes
        memory
            .write(&store, offset as u64, s.as_bytes())
            .map_err(|e| GatewayError::WasmPlugin(format!("Memory write error: {e}")))?;

        Ok(offset)
    }

    /// Read a string from Wasm linear memory starting at offset, length from return value
    fn read_string(
        &self,
        store: &mut Store<wasmtime_wasi::WasiCtx>,
        memory: &Memory,
        result: i32,
    ) -> GatewayResult<String> {
        // The result is the offset; read until null terminator
        let data = memory
            .data(&store);

        let mut end = result as usize;
        while end < data.len() && data[end] != 0 {
            end += 1;
        }

        let bytes = &data[result as usize..end];
        Ok(String::from_utf8_lossy(bytes).to_string())
    }
}
