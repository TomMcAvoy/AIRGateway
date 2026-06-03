use crate::types::{PluginAction, PluginResult};
use dashmap::DashMap;
use rustai_core::error::{GatewayError, GatewayResult};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;
use tracing::{debug, info, warn};

/// Token bucket rate limiter
pub struct TokenBucketRateLimiter {
    buckets: DashMap<String, TokenBucket>,
    default_rate: u64,       // tokens per second
    default_burst: u32,      // max burst size
}

struct TokenBucket {
    tokens: f64,
    last_refill: Instant,
    rate: f64,       // tokens per second
    burst: f64,      // max tokens
}

impl TokenBucketRateLimiter {
    pub fn new(rate: u64, burst: u32) -> Self {
        info!(
            default_rate = rate,
            default_burst = burst,
            "Initializing token bucket rate limiter"
        );

        Self {
            buckets: DashMap::new(),
            default_rate: rate,
            default_burst: burst,
        }
    }

    /// Check if a request should be rate limited
    pub fn check_rate_limit(&self, key: &str, weight: u64) -> Result<Option<Duration>, GatewayError> {
        let mut bucket = self.buckets.entry(key.to_string())
            .or_insert_with(|| TokenBucket {
                tokens: self.default_burst as f64,
                last_refill: Instant::now(),
                rate: self.default_rate as f64,
                burst: self.default_burst as f64,
            });

        // Refill tokens
        let now = Instant::now();
        let elapsed = now.duration_since(bucket.last_refill).as_secs_f64();
        bucket.tokens = (bucket.tokens + elapsed * bucket.rate).min(bucket.burst);
        bucket.last_refill = now;

        // Check if enough tokens
        if bucket.tokens >= weight as f64 {
            bucket.tokens -= weight as f64;
            Ok(None) // Not rate limited
        } else {
            let wait_time = Duration::from_secs_f64((weight as f64 - bucket.tokens) / bucket.rate);
            debug!(
                key = %key,
                wait_ms = wait_time.as_millis(),
                "Rate limit triggered"
            );
            Ok(Some(wait_time))
        }
    }

    /// Create a plugin result for rate limiting
    pub fn rate_limit_result(retry_after: Duration) -> PluginResult {
        PluginResult {
            plugin_name: "rate_limiter".to_string(),
            action: PluginAction::RateLimit {
                retry_after_secs: retry_after.as_secs(),
            },
            modified_payload: None,
            metadata: Some(serde_json::json!({
                "retry_after_secs": retry_after.as_secs(),
            })),
        }
    }
}

/// PII masking plugin
pub struct PiiMasker {
    patterns: Vec<(String, String)>, // (pattern, replacement)
}

impl PiiMasker {
    pub fn new() -> Self {
        info!("Initializing PII masker plugin");

        let patterns = vec![
            // Email addresses
            (r"[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\.[a-zA-Z]{2,}".to_string(), "[EMAIL]".to_string()),
            // Phone numbers (basic US format)
            (r"\b\d{3}[-.]?\d{3}[-.]?\d{4}\b".to_string(), "[PHONE]".to_string()),
            // SSNs
            (r"\b\d{3}-\d{2}-\d{4}\b".to_string(), "[SSN]".to_string()),
            // Credit card numbers (simplified)
            (r"\b(?:\d{4}[-\s]?){3}\d{4}\b".to_string(), "[CREDIT_CARD]".to_string()),
            // IP addresses
            (r"\b\d{1,3}\.\d{1,3}\.\d{1,3}\.\d{1,3}\b".to_string(), "[IP_ADDR]".to_string()),
            // API keys / tokens (bearer tokens)
            (r"(?i)(Bearer\s+)[a-zA-Z0-9._-]+".to_string(), "$1[REDACTED]".to_string()),
        ];

        Self { patterns }
    }

    /// Mask PII in a JSON string
    pub fn mask_pii(&self, payload: &str) -> String {
        let mut result = payload.to_string();

        for (pattern, replacement) in &self.patterns {
            if let Ok(re) = regex::Regex::new(pattern) {
                result = re.replace_all(&result, replacement.as_str()).to_string();
            }
        }

        result
    }

    /// Create a plugin result for PII masking
    pub fn masking_result(original: &str, masked: &str) -> PluginResult {
        PluginResult {
            plugin_name: "pii_masker".to_string(),
            action: PluginAction::Modify,
            modified_payload: Some(serde_json::from_str(masked).unwrap_or(serde_json::Value::Null)),
            metadata: Some(serde_json::json!({
                "original_size": original.len(),
                "masked_size": masked.len(),
            })),
        }
    }
}

/// Distributed tracing plugin with span management
pub struct TracingPlugin {
    trace_enabled: bool,
}

impl TracingPlugin {
    pub fn new() -> Self {
        info!("Initializing tracing plugin");
        Self {
            trace_enabled: true,
        }
    }

    /// Extract trace context from headers/payload
    pub fn extract_trace_context(&self, payload: &str) -> Option<TraceContext> {
        // Try to parse traceparent or similar tracing headers from payload
        if let Ok(json) = serde_json::from_str::<serde_json::Value>(payload) {
            let trace_id = json.get("trace_id")
                .or_else(|| json.get("traceparent"))
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());

            let span_id = json.get("span_id")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());

            if let (Some(trace_id), Some(span_id)) = (trace_id, span_id) {
                return Some(TraceContext { trace_id, span_id });
            }
        }

        // Generate new trace context if not present
        Some(TraceContext {
            trace_id: uuid::Uuid::new_v4().to_string(),
            span_id: uuid::Uuid::new_v4().to_string(),
        })
    }

    /// Inject trace context into payload
    pub fn inject_trace_context(&self, payload: &str, context: &TraceContext) -> String {
        if let Ok(mut json) = serde_json::from_str::<serde_json::Value>(payload) {
            if let Some(obj) = json.as_object_mut() {
                obj.insert("trace_id".to_string(), serde_json::json!(context.trace_id));
                obj.insert("span_id".to_string(), serde_json::json!(context.span_id));
            }
            serde_json::to_string(&json).unwrap_or_else(|_| payload.to_string())
        } else {
            payload.to_string()
        }
    }

    /// Create a tracing plugin result
    pub fn tracing_result(context: &TraceContext) -> PluginResult {
        PluginResult {
            plugin_name: "tracer".to_string(),
            action: PluginAction::Modify,
            modified_payload: None,
            metadata: Some(serde_json::json!({
                "trace_id": context.trace_id,
                "span_id": context.span_id,
            })),
        }
    }
}

/// Trace context
#[derive(Debug, Clone)]
pub struct TraceContext {
    pub trace_id: String,
    pub span_id: String,
}

impl std::fmt::Display for TraceContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "trace_id={}, span_id={}", self.trace_id, self.span_id)
    }
}
