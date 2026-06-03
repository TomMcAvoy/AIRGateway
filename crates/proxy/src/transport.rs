use async_trait::async_trait;
use bytes::Bytes;
use futures::StreamExt;
use rustai_core::error::{GatewayError, GatewayResult};
use rustai_core::traits::{BackendTransport, LlmResponse, SseStream};
use rustai_core::types::{LlmRequest, Provider, SseEvent, Upstream};
use std::sync::Arc;
use std::time::Duration;
use tracing::{debug, warn};

/// Reqwest-based HTTP backend transport for proxying LLM requests
pub struct HyperTransport {
    upstream: Arc<Upstream>,
    client: reqwest::Client,
}

impl HyperTransport {
    pub fn new(upstream: &Upstream) -> GatewayResult<Self> {
        let timeout = upstream.timeout.unwrap_or(Duration::from_secs(30));

        let client = reqwest::Client::builder()
            .timeout(timeout)
            .pool_max_idle_per_host(upstream.max_connections.unwrap_or(100) as usize)
            .build()
            .map_err(|e| GatewayError::Internal(format!("HTTP client build error: {e}")))?;

        Ok(Self {
            upstream: Arc::new(upstream.clone()),
            client,
        })
    }
}

#[async_trait]
impl BackendTransport for HyperTransport {
    async fn forward_request(&self, request: LlmRequest) -> GatewayResult<Box<dyn LlmResponse>> {
        let url = format!("{}v1/chat/completions", self.upstream.base_url);
        debug!(url = %url, request_id = %request.request_id, "Forwarding LLM request");

        let payload = serde_json::json!({
            "model": request.model,
            "stream": request.stream,
            "messages": request.payload,
            "max_tokens": request.max_tokens,
            "temperature": request.temperature,
        });

        let mut req_builder = self.client.post(&url).json(&payload);

        // Set auth headers based on provider
        if let Some(ref api_key) = self.upstream.api_key {
            req_builder = match self.upstream.provider {
                Provider::Anthropic => {
                    req_builder
                        .header("x-api-key", api_key)
                        .header("anthropic-version", "2023-06-01")
                }
                _ => {
                    req_builder.header("authorization", format!("Bearer {api_key}"))
                }
            };
        }

        // Add custom headers
        if let Some(ref headers) = self.upstream.headers {
            for (key, value) in headers {
                req_builder = req_builder.header(key.as_str(), value.as_str());
            }
        }

        let response = req_builder
            .send()
            .await
            .map_err(|e| GatewayError::UpstreamConnection(e.to_string()))?;

        let is_streaming = request.stream;
        Ok(Box::new(ReqwestLlmResponse {
            response: Some(response),
            is_streaming,
        }))
    }

    async fn health_check(&self) -> GatewayResult<bool> {
        let url = format!("{}health", self.upstream.base_url);
        match self.client.get(&url).send().await {
            Ok(resp) => Ok(resp.status().is_success()),
            Err(_) => Ok(false),
        }
    }

    fn protocol(&self) -> &'static str {
        "http"
    }
}

/// LLM response using reqwest
pub struct ReqwestLlmResponse {
    response: Option<reqwest::Response>,
    is_streaming: bool,
}

#[async_trait]
impl LlmResponse for ReqwestLlmResponse {
    async fn as_json(&mut self) -> GatewayResult<serde_json::Value> {
        let response = self.response.take()
            .ok_or_else(|| GatewayError::Internal("Response already consumed".into()))?;

        response.json::<serde_json::Value>().await
            .map_err(|e| GatewayError::UpstreamConnection(e.to_string()))
    }

    async fn stream_sse(&mut self) -> GatewayResult<Box<dyn SseStream>> {
        let response = self.response.take()
            .ok_or_else(|| GatewayError::Internal("Response already consumed".into()))?;

        Ok(Box::new(ReqwestSseStream {
            stream: Box::pin(response.bytes_stream()),
        }))
    }

    fn is_streaming(&self) -> bool {
        self.is_streaming
    }
}

/// SSE stream backed by a reqwest byte stream
pub struct ReqwestSseStream {
    stream: std::pin::Pin<
        Box<dyn futures::Stream<Item = Result<Bytes, reqwest::Error>> + Send>,
    >,
}

#[async_trait]
impl SseStream for ReqwestSseStream {
    async fn next_event(&mut self) -> Option<GatewayResult<SseEvent>> {
        while let Some(chunk_result) = self.stream.next().await {
            match chunk_result {
                Ok(data) => {
                    let text = String::from_utf8_lossy(&data);
                    for line in text.lines() {
                        if let Some(data_str) = line.strip_prefix("data: ") {
                            if data_str == "[DONE]" {
                                return None;
                            }
                            return Some(Ok(SseEvent {
                                id: None,
                                event: "message".to_string(),
                                data: data_str.to_string(),
                                retry: None,
                            }));
                        }
                    }
                    // Return raw data if no SSE format found
                    if !text.is_empty() {
                        return Some(Ok(SseEvent {
                            id: None,
                            event: "message".to_string(),
                            data: text.to_string(),
                            retry: None,
                        }));
                    }
                }
                Err(e) => {
                    return Some(Err(GatewayError::UpstreamConnection(e.to_string())));
                }
            }
        }
        None
    }
}
