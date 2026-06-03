use axum::{
    body::Body,
    extract::State,
    http::HeaderMap,
    response::Response,
    routing::{get, post},
    Json, Router as AxumRouter,
};
use rustai_core::error::GatewayError;
use rustai_core::types::GatewayConfig;
use std::sync::Arc;
use tower::ServiceBuilder;
use tower_http::{cors::CorsLayer, trace::TraceLayer};
use tracing::{info, warn};

use crate::proxy_handler::ProxyHandler;

/// Shared application state
#[derive(Clone)]
pub struct AppState {
    pub config: Arc<GatewayConfig>,
    pub proxy_handler: Arc<ProxyHandler>,
}

/// Build and configure the Axum-based router
pub fn build_router(config: GatewayConfig) -> Result<AxumRouter<AppState>, GatewayError> {
    let config = Arc::new(config);
    let proxy_handler = Arc::new(ProxyHandler::new(config.clone()));

    let state = AppState {
        config: config.clone(),
        proxy_handler,
    };

    let cors = CorsLayer::new()
        .allow_origin(tower_http::cors::Any)
        .allow_methods(tower_http::cors::Any)
        .allow_headers(tower_http::cors::Any);

    let service_stack = ServiceBuilder::new()
        .layer(TraceLayer::new_for_http())
        .layer(cors)
        .into_inner();

    let router: AxumRouter<AppState> = AxumRouter::new()
        .with_state(state)
        .layer(service_stack)
        .route("/health", get(health_handler))
        .route("/v1/chat/completions", post(chat_completions_handler));

    Ok(router)
}

async fn health_handler() -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "status": "ok",
        "service": "rustai-gateway",
        "version": env!("CARGO_PKG_VERSION"),
    }))
}

#[axum::debug_handler]
async fn chat_completions_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<serde_json::Value>,
) -> Result<Response<Body>, GatewayError> {
    let request_id = uuid::Uuid::new_v4().to_string();
    info!(request_id = %request_id, "Received chat completions request");

    let model = payload
        .get("model")
        .and_then(|m| m.as_str())
        .unwrap_or("gpt-4")
        .to_string();

    let stream = payload
        .get("stream")
        .and_then(|s| s.as_bool())
        .unwrap_or(false);

    let provider = if let Some(provider_header) = headers.get("x-provider") {
        if let Ok(val) = provider_header.to_str() {
            match val.to_lowercase().as_str() {
                "openai" => rustai_core::types::Provider::OpenAI,
                "anthropic" => rustai_core::types::Provider::Anthropic,
                "google" => rustai_core::types::Provider::Google,
                "azure" => rustai_core::types::Provider::Azure,
                "ollama" => rustai_core::types::Provider::Ollama,
                _ => rustai_core::types::Provider::Custom(val.to_string()),
            }
        } else {
            rustai_core::types::Provider::OpenAI
        }
    } else {
        let model_lower = model.to_lowercase();
        if model_lower.contains("gpt") || model_lower.contains("o1") || model_lower.contains("o3") {
            rustai_core::types::Provider::OpenAI
        } else if model_lower.contains("claude") {
            rustai_core::types::Provider::Anthropic
        } else if model_lower.contains("gemini") {
            rustai_core::types::Provider::Google
        } else {
            rustai_core::types::Provider::OpenAI
        }
    };

    let llm_request = rustai_core::types::LlmRequest {
        request_id: request_id.clone(),
        model: model.clone(),
        provider,
        stream,
        payload: payload
            .get("messages")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        max_tokens: payload.get("max_tokens").and_then(|v| v.as_u64().map(|n| n as u32)),
        temperature: payload.get("temperature").and_then(|v| v.as_f64()),
    };

    let response = state.proxy_handler.handle_request(llm_request).await?;
    Ok(response)
}
