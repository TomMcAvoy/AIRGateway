use async_trait::async_trait;
use rustai_core::error::{GatewayError, GatewayResult};
use rustai_core::traits::ProviderAdapter;
use rustai_core::types::{LlmRequest, Provider, SseEvent};
use serde_json::Value;

/// OpenAI-compatible provider adapter
pub struct OpenAIAdapter;

#[async_trait]
impl ProviderAdapter for OpenAIAdapter {
    fn provider(&self) -> Provider {
        Provider::OpenAI
    }

    fn translate_request(&self, request: LlmRequest) -> GatewayResult<Value> {
        let mut body = serde_json::json!({
            "model": request.model,
            "messages": request.payload,
            "stream": request.stream,
        });

        if let Some(max_tokens) = request.max_tokens {
            body["max_tokens"] = serde_json::json!(max_tokens);
        }
        if let Some(temp) = request.temperature {
            body["temperature"] = serde_json::json!(temp);
        }

        Ok(body)
    }

    fn translate_response(&self, response: Value) -> GatewayResult<Value> {
        // OpenAI responses are already in the standard format
        Ok(response)
    }

    fn parse_sse_chunk(&self, chunk: &[u8]) -> GatewayResult<Option<SseEvent>> {
        let text = String::from_utf8_lossy(chunk);
        for line in text.lines() {
            if let Some(data) = line.strip_prefix("data: ") {
                if data == "[DONE]" {
                    return Ok(None);
                }
                return Ok(Some(SseEvent {
                    id: None,
                    event: "chat.completion.chunk".to_string(),
                    data: data.to_string(),
                    retry: None,
                }));
            }
        }
        Ok(None)
    }
}

/// Anthropic Claude provider adapter
pub struct AnthropicAdapter;

#[async_trait]
impl ProviderAdapter for AnthropicAdapter {
    fn provider(&self) -> Provider {
        Provider::Anthropic
    }

    fn translate_request(&self, request: LlmRequest) -> GatewayResult<Value> {
        let messages = match &request.payload {
            Value::Array(arr) => arr.clone(),
            other => vec![serde_json::json!({
                "role": "user",
                "content": other
            })],
        };

        let mut body = serde_json::json!({
            "model": request.model,
            "messages": messages,
            "stream": request.stream,
        });

        if let Some(max_tokens) = request.max_tokens {
            body["max_tokens"] = serde_json::json!(max_tokens);
        }

        Ok(body)
    }

    fn translate_response(&self, response: Value) -> GatewayResult<Value> {
        // Convert Anthropic response format to OpenAI-compatible format
        if let Some(content) = response.get("content") {
            Ok(serde_json::json!({
                "choices": [{
                    "message": {
                        "role": "assistant",
                        "content": content
                    }
                }]
            }))
        } else {
            Ok(response)
        }
    }

    fn parse_sse_chunk(&self, chunk: &[u8]) -> GatewayResult<Option<SseEvent>> {
        let text = String::from_utf8_lossy(chunk);
        for line in text.lines() {
            if let Some(data) = line.strip_prefix("data: ") {
                if data == "[DONE]" {
                    return Ok(None);
                }
                return Ok(Some(SseEvent {
                    id: None,
                    event: "content_block_delta".to_string(),
                    data: data.to_string(),
                    retry: None,
                }));
            }
        }
        Ok(None)
    }
}

/// Google Gemini provider adapter
pub struct GoogleAdapter;

#[async_trait]
impl ProviderAdapter for GoogleAdapter {
    fn provider(&self) -> Provider {
        Provider::Google
    }

    fn translate_request(&self, request: LlmRequest) -> GatewayResult<Value> {
        // Convert OpenAI format to Gemini format
        let contents = match &request.payload {
            Value::Array(messages) => {
                let mut gemini_contents = Vec::new();
                for msg in messages {
                    if let (Some(role), Some(content)) = (
                        msg.get("role").and_then(|r| r.as_str()),
                        msg.get("content"),
                    ) {
                        let gemini_role = match role {
                            "system" => "user",
                            "assistant" => "model",
                            _ => "user",
                        };
                        gemini_contents.push(serde_json::json!({
                            "role": gemini_role,
                            "parts": [{"text": content}]
                        }));
                    }
                }
                gemini_contents
            }
            _ => vec![],
        };

        let mut body = serde_json::json!({
            "contents": contents
        });

        if let Some(max_tokens) = request.max_tokens {
            body["generationConfig"] = serde_json::json!({
                "maxOutputTokens": max_tokens
            });
        }

        Ok(body)
    }

    fn translate_response(&self, response: Value) -> GatewayResult<Value> {
        // Convert Gemini response to OpenAI-compatible format
        if let Some(candidates) = response.get("candidates") {
            if let Some(first) = candidates.get(0) {
                if let Some(content) = first.get("content") {
                    let text = content
                        .get("parts")
                        .and_then(|p| p.get(0))
                        .and_then(|p| p.get("text"))
                        .cloned()
                        .unwrap_or(Value::Null);

                    return Ok(serde_json::json!({
                        "choices": [{
                            "message": {
                                "role": "assistant",
                                "content": text
                            }
                        }]
                    }));
                }
            }
        }
        Ok(response)
    }

    fn parse_sse_chunk(&self, chunk: &[u8]) -> GatewayResult<Option<SseEvent>> {
        let text = String::from_utf8_lossy(chunk);
        for line in text.lines() {
            if let Some(data) = line.strip_prefix("data: ") {
                return Ok(Some(SseEvent {
                    id: None,
                    event: "message".to_string(),
                    data: data.to_string(),
                    retry: None,
                }));
            }
        }
        Ok(None)
    }
}

/// Get the appropriate provider adapter for a given provider
pub fn get_provider_adapter(provider: &Provider) -> Box<dyn ProviderAdapter> {
    match provider {
        Provider::OpenAI => Box::new(OpenAIAdapter),
        Provider::Anthropic => Box::new(AnthropicAdapter),
        Provider::Google => Box::new(GoogleAdapter),
        Provider::Azure => Box::new(OpenAIAdapter), // Azure uses OpenAI-compatible API
        Provider::Ollama => Box::new(OpenAIAdapter), // Ollama uses OpenAI-compatible API
        Provider::Custom(_) => Box::new(OpenAIAdapter), // Default to OpenAI format
    }
}
