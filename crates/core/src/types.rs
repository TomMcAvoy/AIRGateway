use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Duration;
use url::Url;

/// Supported AI provider backends
#[derive(Debug, Clone, Serialize, Deserialize, Hash, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Provider {
    OpenAI,
    Anthropic,
    Google,
    Azure,
    Ollama,
    Custom(String),
}

impl std::fmt::Display for Provider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Provider::OpenAI => write!(f, "openai"),
            Provider::Anthropic => write!(f, "anthropic"),
            Provider::Google => write!(f, "google"),
            Provider::Azure => write!(f, "azure"),
            Provider::Ollama => write!(f, "ollama"),
            Provider::Custom(name) => write!(f, "custom_{}", name),
        }
    }
}

/// Protocol version for upstream APIs
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ApiProtocol {
    Rest,
    Grpc,
    Sse,
    WebSocket,
}

/// Upstream backend endpoint configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Upstream {
    pub id: String,
    pub provider: Provider,
    pub protocol: ApiProtocol,
    pub base_url: Url,
    pub api_key: Option<String>,
    pub timeout: Option<Duration>,
    pub max_connections: Option<u32>,
    pub health_check_path: Option<String>,
    pub headers: Option<HashMap<String, String>>,
    pub weight: Option<u32>,
}

/// Rate limiting configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimitConfig {
    pub requests_per_second: u64,
    pub burst_size: u32,
    pub window_secs: u64,
}

/// LLM request metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmRequest {
    pub request_id: String,
    pub model: String,
    pub provider: Provider,
    pub stream: bool,
    pub payload: serde_json::Value,
    pub max_tokens: Option<u32>,
    pub temperature: Option<f64>,
}

/// SSE event for streaming LLM responses
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SseEvent {
    pub id: Option<String>,
    pub event: String,
    pub data: String,
    pub retry: Option<u64>,
}

impl SseEvent {
    pub fn as_bytes(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        if let Some(ref id) = self.id {
            buf.extend_from_slice(b"id: ");
            buf.extend_from_slice(id.as_bytes());
            buf.push(b'\n');
        }
        if let Some(retry) = self.retry {
            buf.extend_from_slice(b"retry: ");
            buf.extend_from_slice(retry.to_string().as_bytes());
            buf.push(b'\n');
        }
        buf.extend_from_slice(b"event: ");
        buf.extend_from_slice(self.event.as_bytes());
        buf.push(b'\n');
        buf.extend_from_slice(b"data: ");
        buf.extend_from_slice(self.data.as_bytes());
        buf.extend_from_slice(b"\n\n");
        buf
    }
}

/// Proxy route definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Route {
    pub id: String,
    pub path_pattern: String,
    pub methods: Vec<String>,
    pub upstreams: Vec<String>,
    pub rate_limit: Option<RateLimitConfig>,
    pub plugins: Vec<String>,
    pub strip_prefix: bool,
    pub timeout: Option<Duration>,
}

/// Connection pool metrics
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ConnectionMetrics {
    pub active_connections: u64,
    pub total_connections: u64,
    pub bytes_in: u64,
    pub bytes_out: u64,
    pub latency_p50: Duration,
    pub latency_p99: Duration,
}

/// Health status of an upstream backend
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum HealthStatus {
    Healthy,
    Degraded(String),
    Unhealthy(String),
}

/// Gateway configuration root
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GatewayConfig {
    pub name: String,
    pub version: String,
    pub listen_addr: String,
    pub listen_tls_addr: Option<String>,
    pub cert_path: Option<String>,
    pub key_path: Option<String>,
    pub upstreams: Vec<Upstream>,
    pub routes: Vec<Route>,
    pub rate_limit_default: Option<RateLimitConfig>,
    pub log_level: Option<String>,
    pub metrics_addr: Option<String>,
    pub wasm_plugins_dir: Option<String>,
}
