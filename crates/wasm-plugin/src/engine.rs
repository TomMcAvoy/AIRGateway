use crate::runtime::WasmPluginInstance;
use crate::types::{MemoryLimits, PluginResult, WasmPluginConfig};
use dashmap::DashMap;
use rustai_core::error::{GatewayError, GatewayResult};
use std::sync::Arc;
use tracing::{debug, info, warn};

/// Manager for Wasm plugin lifecycle and execution
pub struct WasmPluginEngine {
    plugins: DashMap<String, Arc<WasmPluginInstance>>,
    configs: DashMap<String, WasmPluginConfig>,
    memory_limits: MemoryLimits,
}

impl WasmPluginEngine {
    /// Create a new Wasm plugin engine
    pub fn new(memory_limits: MemoryLimits) -> Self {
        info!("Initializing Wasm plugin engine");
        Self {
            plugins: DashMap::new(),
            configs: DashMap::new(),
            memory_limits,
        }
    }

    /// Load a plugin from a Wasm file
    pub fn load_plugin(&self, config: WasmPluginConfig) -> GatewayResult<()> {
        if self.plugins.contains_key(&config.name) {
            warn!(
                plugin_name = %config.name,
                "Plugin already loaded, reloading"
            );
            self.unload_plugin(&config.name);
        }

        let plugin = WasmPluginInstance::from_file(
            &config.name,
            &config.wasm_path,
            self.memory_limits.clone(),
        )?;

        self.plugins.insert(config.name.clone(), Arc::new(plugin));
        self.configs.insert(config.name.clone(), config);

        info!(
            plugin_name = %config.name,
            "Wasm plugin loaded successfully"
        );

        Ok(())
    }

    /// Load a plugin from Wasm bytes
    pub fn load_plugin_bytes(
        &self,
        name: &str,
        wasm_bytes: &[u8],
        config: Option<serde_json::Value>,
    ) -> GatewayResult<()> {
        if self.plugins.contains_key(name) {
            self.unload_plugin(name);
        }

        let plugin = WasmPluginInstance::from_bytes(
            name,
            wasm_bytes,
            self.memory_limits.clone(),
        )?;

        let plugin_config = WasmPluginConfig {
            name: name.to_string(),
            wasm_path: "bytes".to_string(),
            enabled: true,
            config,
        };

        self.plugins.insert(name.to_string(), Arc::new(plugin));
        self.configs.insert(name.to_string(), plugin_config);

        info!(plugin_name = %name, "Wasm plugin loaded from bytes");
        Ok(())
    }

    /// Unload a plugin
    pub fn unload_plugin(&self, name: &str) {
        self.plugins.remove(name);
        self.configs.remove(name);
        info!(plugin_name = %name, "Wasm plugin unloaded");
    }

    /// Execute a plugin on a request payload
    pub async fn execute_plugin(
        &self,
        name: &str,
        request_payload: &str,
    ) -> GatewayResult<PluginResult> {
        let plugin = self
            .plugins
            .get(name)
            .ok_or_else(|| GatewayError::NotFound(format!("Plugin '{name}' not found")))?;

        let config_json = self
            .configs
            .get(name)
            .and_then(|c| c.config.as_ref())
            .map(|c| serde_json::to_string(c).unwrap_or_default())
            .unwrap_or_default();

        plugin.execute(request_payload, &config_json).await
    }

    /// Execute all loaded plugins on a request payload
    pub async fn execute_all(
        &self,
        request_payload: &str,
    ) -> Vec<(String, GatewayResult<PluginResult>)> {
        let mut results = Vec::new();

        for entry in self.plugins.iter() {
            let name = entry.key().clone();
            let plugin = entry.value().clone();

            let config_json = self
                .configs
                .get(&name)
                .and_then(|c| c.config.as_ref())
                .map(|c| serde_json::to_string(c).unwrap_or_default())
                .unwrap_or_default();

            let result = plugin.execute(request_payload, &config_json).await;
            results.push((name, result));
        }

        results
    }

    /// Check if a plugin is loaded
    pub fn is_loaded(&self, name: &str) -> bool {
        self.plugins.contains_key(name)
    }

    /// Get list of loaded plugin names
    pub fn loaded_plugins(&self) -> Vec<String> {
        self.plugins.iter().map(|e| e.key().clone()).collect()
    }

    /// Get the number of loaded plugins
    pub fn plugin_count(&self) -> usize {
        self.plugins.len()
    }
}

/// Load plugins from a directory
pub fn load_plugins_from_dir(
    engine: &WasmPluginEngine,
    dir: &str,
) -> GatewayResult<usize> {
    let path = std::path::Path::new(dir);
    if !path.exists() {
        warn!(dir = %dir, "Plugin directory does not exist");
        return Ok(0);
    }

    let mut count = 0;
    if let Ok(entries) = std::fs::read_dir(path) {
        for entry in entries.flatten() {
            let file_path = entry.path();
            if file_path.extension().and_then(|e| e.to_str()) == Some("wasm") {
                let name = file_path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("unknown");

                let config = WasmPluginConfig {
                    name: name.to_string(),
                    wasm_path: file_path.to_string_lossy().to_string(),
                    enabled: true,
                    config: None,
                };

                match engine.load_plugin(config) {
                    Ok(()) => {
                        count += 1;
                        info!(
                            plugin_name = %name,
                            path = %file_path.display(),
                            "Loaded plugin from directory"
                        );
                    }
                    Err(e) => {
                        warn!(
                            plugin_name = %name,
                            error = %e,
                            "Failed to load plugin"
                        );
                    }
                }
            }
        }
    }

    Ok(count)
}
