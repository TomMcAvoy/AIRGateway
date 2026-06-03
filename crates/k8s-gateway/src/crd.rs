use k8s_openapi::apimachinery::pkg::apis::meta::v1::ObjectMeta;
use kube::CustomResource;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// RustAI Gateway custom resource definition
/// Represents an AI Gateway configuration in Kubernetes
#[derive(CustomResource, Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[kube(
    group = "gateway.rustai.dev",
    version = "v1",
    kind = "AiGateway",
    plural = "aigateways",
    namespaced,
    status = "AiGatewayStatus",
    printcolumn = r#"{"name":"Provider", "jsonPath":".spec.provider", "type":"string"}"#,
    printcolumn = r#"{"name":"Ready", "jsonPath":".status.ready", "type":"boolean"}"#,
)]
#[serde(rename_all = "camelCase")]
pub struct AiGatewaySpec {
    /// The AI provider to use (openai, anthropic, google, azure, ollama)
    pub provider: String,

    /// The upstream endpoint URL
    pub upstream_url: String,

    /// The API key reference (from a Kubernetes secret)
    pub api_key_secret: Option<SecretReference>,

    /// Model routing configuration
    pub models: Option<Vec<ModelRoute>>,

    /// Rate limiting configuration
    pub rate_limit: Option<RateLimitSpec>,

    /// Wasm plugins to attach
    pub plugins: Option<Vec<PluginRef>>,

    /// TLS configuration
    pub tls: Option<TlsSpec>,

    /// Whether to enable streaming
    pub streaming: Option<bool>,

    /// Connection pool configuration
    pub connection_pool: Option<ConnectionPoolSpec>,
}

/// Reference to a Kubernetes secret
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SecretReference {
    pub name: String,
    pub key: String,
}

/// Model route definition
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ModelRoute {
    /// Model name pattern (e.g., "gpt-4*", "claude-*")
    pub model_pattern: String,

    /// Override upstream URL for this model
    pub upstream_url: Option<String>,

    /// Override rate limit for this model
    pub rate_limit: Option<RateLimitSpec>,
}

/// Rate limit specification
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct RateLimitSpec {
    pub requests_per_second: u64,
    pub burst_size: u32,
}

/// Reference to a Wasm plugin
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct PluginRef {
    pub name: String,
    pub wasm_config_map: String,
    pub config: Option<serde_json::Value>,
}

/// TLS configuration
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct TlsSpec {
    pub secret_name: String,
    pub cert_key: String,
    pub key_key: String,
}

/// Connection pool configuration
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ConnectionPoolSpec {
    pub max_connections: u32,
    pub idle_timeout_secs: u64,
}

/// Status of the AiGateway resource
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct AiGatewayStatus {
    pub ready: bool,
    pub conditions: Vec<GatewayCondition>,
    pub observed_generation: Option<i64>,
}

/// Gateway condition
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct GatewayCondition {
    #[serde(rename = "type")]
    pub condition_type: String,
    pub status: String,
    pub reason: String,
    pub message: String,
    pub last_transition_time: Option<String>,
}
