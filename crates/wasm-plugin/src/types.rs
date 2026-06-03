use serde::{Deserialize, Serialize};

/// Configuration for a Wasm plugin
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WasmPluginConfig {
    pub name: String,
    pub wasm_path: String,
    pub enabled: bool,
    pub config: Option<serde_json::Value>,
}

/// Plugin type categorization
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PluginType {
    /// Rate limiting plugin
    RateLimiter,
    /// PII masking plugin
    PiiMasker,
    /// Distributed tracing plugin
    Tracer,
    /// Custom/user-defined plugin
    Custom(String),
}

/// Result from a Wasm plugin execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginResult {
    pub plugin_name: String,
    pub action: PluginAction,
    pub modified_payload: Option<serde_json::Value>,
    pub metadata: Option<serde_json::Value>,
}

/// Action a plugin can take
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PluginAction {
    /// Allow the request to proceed
    Allow,
    /// Deny the request
    Deny { reason: String },
    /// Rate limit the request
    RateLimit { retry_after_secs: u64 },
    /// Modify the request payload
    Modify,
}

/// Plugin host functions available to Wasm modules
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HostFunctions {
    pub http_fetch: bool,
    pub crypto_random: bool,
    pub clock: bool,
    pub filesystem_read: bool,
}

impl Default for HostFunctions {
    fn default() -> Self {
        Self {
            http_fetch: false,
            crypto_random: true,
            clock: true,
            filesystem_read: false,
        }
    }
}

/// Memory limits for Wasm plugin sandbox
#[derive(Debug, Clone)]
pub struct MemoryLimits {
    pub max_memory_pages: u32,  // 64KB per page
    pub max_stack_size: u32,
    pub max_execution_secs: u64,
}

impl Default for MemoryLimits {
    fn default() -> Self {
        Self {
            max_memory_pages: 256,    // 16MB
            max_stack_size: 1048576,  // 1MB
            max_execution_secs: 5,    // 5 seconds
        }
    }
}
