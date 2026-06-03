use bytes::Bytes;
use futures::StreamExt;
use rustai_core::traits::LlmResponse;
use rustai_core::types::SseEvent;
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
use tracing::{debug, error};

/// SSE proxy that streams server-sent events from upstream to client
pub struct SseProxy;

impl SseProxy {
    /// Stream an LLM response as SSE events
    pub async fn stream_response(
        mut response: Box<dyn LlmResponse>,
    ) -> Vec<SseEvent> {
        let mut sse_stream = match response.stream_sse().await {
            Ok(s) => s,
            Err(_) => return vec![],
        };

        let mut events = Vec::new();
        while let Some(event_result) = sse_stream.next_event().await {
            match event_result {
                Ok(event) => events.push(event),
                Err(e) => {
                    debug!("SSE stream error: {e}");
                    break;
                }
            }
        }
        events
    }
}

/// Empty SSE stream used as fallback
pub struct EmptySseStream;

#[async_trait::async_trait]
impl rustai_core::traits::SseStream for EmptySseStream {
    async fn next_event(&mut self) -> Option<rustai_core::error::GatewayResult<SseEvent>> {
        None
    }
}
