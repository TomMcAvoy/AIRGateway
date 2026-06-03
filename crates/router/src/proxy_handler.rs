use axum::{
    body::Body,
    http::StatusCode,
    response::Response,
};
use bytes::Bytes;
use futures::StreamExt;
use rustai_core::error::{GatewayError, GatewayResult};
use rustai_core::types::{GatewayConfig, LlmRequest, Provider, SseEvent, Upstream};
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
use tracing::{debug, error, info, warn};

use crate::provider::{self, get_provider_adapter};

/// Handles proxying LLM requests to upstream backends
pub struct ProxyHandler {
    config: Arc<GatewayConfig>,
}

impl ProxyHandler {
    pub fn new(config: Arc<GatewayConfig>) -> Self {
        Self { config }
    }

    /// Handle an LLM request by finding the correct upstream and forwarding
    pub async fn handle_request(
        &self,
        request: LlmRequest,
    ) -> GatewayResult<Response<Body>> {
        let upstream = self.find_upstream(&request.provider)?;
        let adapter = get_provider_adapter(&request.provider);
        let translated_payload = adapter.translate_request(request.clone())?;

        let url = match upstream.provider {
            Provider::OpenAI | Provider::Azure => {
                format!("{}v1/chat/completions", upstream.base_url)
            }
            Provider::Anthropic => {
                format!("{}v1/messages", upstream.base_url)
            }
            Provider::Google => {
                let model = &request.model;
                format!("{}v1beta/models/{model}:streamGenerateContent", upstream.base_url)
            }
            Provider::Ollama => {
                format!("{}api/chat", upstream.base_url)
            }
            Provider::Custom(_) => {
                format!("{}v1/chat/completions", upstream.base_url)
            }
        };

        debug!(
            request_id = %request.request_id,
            upstream = %upstream.id,
            url = %url,
            provider = %upstream.provider,
            "Forwarding request to upstream"
        );

        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(upstream.timeout.map_or(30, |d| d.as_secs())))
            .build()
            .map_err(|e| GatewayError::Internal(format!("HTTP client build error: {e}")))?;

        let mut req_builder = client.post(&url).json(&translated_payload);

        if let Some(ref api_key) = upstream.api_key {
            req_builder = match upstream.provider {
                Provider::Anthropic => {
                    req_builder
                        .header("x-api-key", api_key)
                        .header("anthropic-version", "2023-06-01")
                }
                _ => req_builder.header("authorization", format!("Bearer {api_key}")),
            };
        }

        if let Some(ref headers) = upstream.headers {
            for (key, value) in headers {
                req_builder = req_builder.header(key.as_str(), value.as_str());
            }
        }

        let upstream_response = req_builder
            .send()
            .await
            .map_err(|e| GatewayError::UpstreamConnection(e.to_string()))?;

        if request.stream {
            self.stream_response(upstream_response, request).await
        } else {
            self.json_response(upstream_response, request).await
        }
    }

    async fn stream_response(
        &self,
        upstream_resp: reqwest::Response,
        request: LlmRequest,
    ) -> GatewayResult<Response<Body>> {
        info!(request_id = %request.request_id, "Streaming response from upstream");

        let adapter = get_provider_adapter(&request.provider);
        let (tx, rx) = mpsc::channel::<Bytes>(1024);

        tokio::spawn(async move {
            let mut stream = upstream_resp.bytes_stream();
            while let Some(chunk_result) = stream.next().await {
                match chunk_result {
                    Ok(chunk) => {
                        match adapter.parse_sse_chunk(&chunk) {
                            Ok(Some(event)) => {
                                let _ = tx.send(Bytes::from(event.as_bytes())).await;
                            }
                            Ok(None) => {
                                let _ = tx.send(chunk).await;
                            }
                            Err(e) => {
                                let err_event = SseEvent {
                                    id: None,
                                    event: "error".to_string(),
                                    data: format!("{{\"error\":\"{}\"}}", e),
                                    retry: None,
                                };
                                let _ = tx.send(Bytes::from(err_event.as_bytes())).await;
                                break;
                            }
                        }
                    }
                    Err(e) => {
                        error!("Stream error: {e}");
                        break;
                    }
                }
            }
            let _ = tx.send(Bytes::from("data: [DONE]\n\n")).await;
            info!(request_id = %request.request_id, "Stream completed");
        });

        use futures::StreamExt;
        let body = Body::from_stream(
            ReceiverStream::new(rx)
                .map(Ok::<_, std::convert::Infallible>)
        );

        Ok(Response::builder()
            .header("content-type", "text/event-stream")
            .header("cache-control", "no-cache")
            .header("connection", "keep-alive")
            .header("x-accel-buffering", "no")
            .body(body)
            .unwrap())
    }

    async fn json_response(
        &self,
        upstream_resp: reqwest::Response,
        request: LlmRequest,
    ) -> GatewayResult<Response<Body>> {
        let status = upstream_resp.status();
        let headers = upstream_resp.headers().clone();
        let bytes = upstream_resp.bytes().await
            .map_err(|e| GatewayError::UpstreamConnection(e.to_string()))?;

        let adapter = get_provider_adapter(&request.provider);
        if status.is_success() {
            if let Ok(json) = serde_json::from_slice::<serde_json::Value>(&bytes) {
                let translated = adapter.translate_response(json)
                    .unwrap_or(serde_json::json!({"error": "translation failed"}));
                let body = serde_json::to_vec(&translated).unwrap_or_default();
                return Ok(Response::builder()
                    .status(status)
                    .header("content-type", "application/json")
                    .body(Body::from(body))
                    .unwrap());
            }
        }

        let mut builder = Response::builder().status(status);
        for (key, value) in headers.iter() {
            if let Ok(val) = value.to_str() {
                builder = builder.header(key.as_str(), val);
            }
        }

        Ok(builder
            .body(Body::from(bytes))
            .unwrap_or_else(|_| {
                Response::builder()
                    .status(StatusCode::BAD_GATEWAY)
                    .body(Body::from("upstream error"))
                    .unwrap()
            }))
    }

    fn find_upstream(&self, provider: &Provider) -> GatewayResult<Upstream> {
        for upstream in &self.config.upstreams {
            if &upstream.provider == provider {
                return Ok(upstream.clone());
            }
        }
        let provider_str = provider.to_string();
        for upstream in &self.config.upstreams {
            if upstream.provider.to_string() == provider_str {
                return Ok(upstream.clone());
            }
        }
        self.config.upstreams.first().cloned()
            .ok_or_else(|| GatewayError::NotFound(
                format!("No upstream found for provider '{provider}'")
            ))
    }
}
