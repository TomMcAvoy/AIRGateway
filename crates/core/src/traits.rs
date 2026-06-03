use crate::error::GatewayResult;
use crate::types::{GatewayConfig, LlmRequest, SseEvent};
use async_trait::async_trait;
use bytes::Bytes;
use std::sync::Arc;

/// Backend proxy transport trait
/// Abstracts over different transport protocols (HTTP, gRPC, WebSocket)
#[async_trait]
pub trait BackendTransport: Send + Sync {
    /// Forward an LLM request to the upstream backend
    async fn forward_request(&self, request: LlmRequest) -> GatewayResult<Box<dyn LlmResponse>>;

    /// Check upstream backend health
    async fn health_check(&self) -> GatewayResult<bool>;

    /// Get the transport protocol identifier
    fn protocol(&self) -> &'static str;
}

/// LLM response trait (supports both streaming and non-streaming)
#[async_trait]
pub trait LlmResponse: Send {
    /// Get the response as a complete JSON value (non-streaming)
    async fn as_json(&mut self) -> GatewayResult<serde_json::Value>;

    /// Stream SSE events for streaming responses
    async fn stream_sse(&mut self) -> GatewayResult<Box<dyn SseStream>>;

    /// Whether this response supports streaming
    fn is_streaming(&self) -> bool;
}

/// SSE event stream trait
#[async_trait]
pub trait SseStream: Send {
    /// Get the next SSE event from the stream
    async fn next_event(&mut self) -> Option<GatewayResult<SseEvent>>;
}

/// Middleware trait for the Tower-based middleware pipeline
#[async_trait]
pub trait GatewayMiddleware: Send + Sync {
    /// Name identifier for the middleware
    fn name(&self) -> &'static str;

    /// Process an incoming LLM request before forwarding
    async fn pre_process(
        &self,
        request: LlmRequest,
    ) -> GatewayResult<LlmRequest>;

    /// Process the upstream response before returning to client
    async fn post_process(
        &self,
        response: Box<dyn LlmResponse>,
    ) -> GatewayResult<Box<dyn LlmResponse>>;
}

/// Plugin trait for Wasm-based extensions
#[async_trait]
pub trait WasmPlugin: Send + Sync {
    /// Initialize the plugin with configuration
    async fn init(&self, config: &str) -> GatewayResult<()>;

    /// Execute plugin logic on a request
    async fn on_request(&self, request: LlmRequest) -> GatewayResult<LlmRequest>;

    /// Execute plugin logic on a response
    async fn on_response(
        &self,
        response: Box<dyn LlmResponse>,
    ) -> GatewayResult<Box<dyn LlmResponse>>;
}

/// Provider adapter trait for different LLM API formats
#[async_trait]
pub trait ProviderAdapter: Send + Sync {
    /// The provider this adapter handles
    fn provider(&self) -> crate::types::Provider;

    /// Translate an internal request to the provider's API format
    fn translate_request(&self, request: LlmRequest) -> GatewayResult<serde_json::Value>;

    /// Translate a provider response back to internal format
    fn translate_response(&self, response: serde_json::Value) -> GatewayResult<serde_json::Value>;

    /// Extract streaming SSE events from provider-specific format
    fn parse_sse_chunk(&self, chunk: &[u8]) -> GatewayResult<Option<SseEvent>>;
}

/// Configuration source trait (file, env, k8s, etc.)
#[async_trait]
pub trait ConfigSource: Send + Sync {
    /// Load gateway configuration from this source
    async fn load_config(&self) -> GatewayResult<GatewayConfig>;

    /// Watch for configuration changes
    async fn watch(&self) -> GatewayResult<tokio::sync::watch::Receiver<Arc<GatewayConfig>>>;
}

/// Metrics collector trait
#[async_trait]
pub trait MetricsCollector: Send + Sync {
    /// Record a request
    fn record_request(&self, provider: &str, status: &str, latency_secs: f64);

    /// Record bytes transferred
    fn record_bytes(&self, direction: &str, bytes: u64);

    /// Record active connection count
    fn record_connection(&self, delta: i64);

    /// Record rate limit hits
    fn record_rate_limit(&self, route_id: &str);

    /// Record plugin execution
    fn record_plugin_execution(&self, plugin_name: &str, duration_secs: f64);
}
