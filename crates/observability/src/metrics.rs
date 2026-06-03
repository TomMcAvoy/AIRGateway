use async_trait::async_trait;
use prometheus::{
    Counter, CounterVec, Gauge, GaugeVec, HistogramVec, Opts, Registry,
};
use rustai_core::traits::MetricsCollector;
use std::sync::Arc;
use tracing::info;

/// Prometheus-based metrics collector for the RustAI Gateway
pub struct PrometheusMetrics {
    registry: Registry,

    // Request metrics
    requests_total: CounterVec,
    request_duration_seconds: HistogramVec,
    active_connections: Gauge,

    // Upstream metrics
    upstream_requests_total: CounterVec,
    upstream_errors_total: CounterVec,
    upstream_latency_seconds: HistogramVec,

    // Rate limiting metrics
    rate_limited_requests_total: CounterVec,

    // Plugin metrics
    plugin_execution_duration_seconds: HistogramVec,
    plugin_errors_total: CounterVec,

    // Bandwidth metrics
    bytes_received_total: Counter,
    bytes_sent_total: Counter,
}

impl PrometheusMetrics {
    /// Create a new PrometheusMetrics collector
    pub fn new() -> Self {
        let registry = Registry::new();

        let requests_total = CounterVec::new(
            Opts::new("rustai_requests_total", "Total number of HTTP requests handled")
                .namespace("rustai"),
            &["method", "path", "status"],
        )
        .expect("Failed to create requests_total metric");
        registry.register(Box::new(requests_total.clone())).ok();

        let request_duration_seconds = HistogramVec::new(
            prometheus::HistogramOpts::new(
                "rustai_request_duration_seconds",
                "Request duration in seconds",
            )
            .namespace("rustai")
            .buckets(vec![0.001, 0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0]),
            &["method", "path"],
        )
        .expect("Failed to create request_duration metric");
        registry.register(Box::new(request_duration_seconds.clone())).ok();

        let active_connections = Gauge::new(
            "rustai_active_connections",
            "Number of active connections",
        )
        .expect("Failed to create active_connections metric");
        registry.register(Box::new(active_connections.clone())).ok();

        let upstream_requests_total = CounterVec::new(
            Opts::new("rustai_upstream_requests_total", "Total number of upstream requests")
                .namespace("rustai"),
            &["upstream", "provider"],
        )
        .expect("Failed to create upstream_requests metric");
        registry.register(Box::new(upstream_requests_total.clone())).ok();

        let upstream_errors_total = CounterVec::new(
            Opts::new("rustai_upstream_errors_total", "Total number of upstream errors")
                .namespace("rustai"),
            &["upstream", "error_type"],
        )
        .expect("Failed to create upstream_errors metric");
        registry.register(Box::new(upstream_errors_total.clone())).ok();

        let upstream_latency_seconds = HistogramVec::new(
            prometheus::HistogramOpts::new(
                "rustai_upstream_latency_seconds",
                "Upstream request latency in seconds",
            )
            .namespace("rustai")
            .buckets(vec![0.001, 0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.0, 5.0]),
            &["upstream", "provider"],
        )
        .expect("Failed to create upstream_latency metric");
        registry.register(Box::new(upstream_latency_seconds.clone())).ok();

        let rate_limited_requests_total = CounterVec::new(
            Opts::new("rustai_rate_limited_requests_total", "Total number of rate limited requests")
                .namespace("rustai"),
            &["route_id"],
        )
        .expect("Failed to create rate_limited metric");
        registry.register(Box::new(rate_limited_requests_total.clone())).ok();

        let plugin_execution_duration_seconds = HistogramVec::new(
            prometheus::HistogramOpts::new(
                "rustai_plugin_execution_duration_seconds",
                "Plugin execution duration in seconds",
            )
            .namespace("rustai")
            .buckets(vec![0.0001, 0.0005, 0.001, 0.005, 0.01, 0.05, 0.1]),
            &["plugin_name"],
        )
        .expect("Failed to create plugin_execution metric");
        registry.register(Box::new(plugin_execution_duration_seconds.clone())).ok();

        let plugin_errors_total = CounterVec::new(
            Opts::new("rustai_plugin_errors_total", "Total number of plugin execution errors")
                .namespace("rustai"),
            &["plugin_name"],
        )
        .expect("Failed to create plugin_errors metric");
        registry.register(Box::new(plugin_errors_total.clone())).ok();

        let bytes_received_total = Counter::new(
            "rustai_bytes_received_total",
            "Total bytes received from clients",
        )
        .expect("Failed to create bytes_received metric");
        registry.register(Box::new(bytes_received_total.clone())).ok();

        let bytes_sent_total = Counter::new(
            "rustai_bytes_sent_total",
            "Total bytes sent to clients",
        )
        .expect("Failed to create bytes_sent metric");
        registry.register(Box::new(bytes_sent_total.clone())).ok();

        info!("Prometheus metrics initialized");

        Self {
            registry,
            requests_total,
            request_duration_seconds,
            active_connections,
            upstream_requests_total,
            upstream_errors_total,
            upstream_latency_seconds,
            rate_limited_requests_total,
            plugin_execution_duration_seconds,
            plugin_errors_total,
            bytes_received_total,
            bytes_sent_total,
        }
    }

    /// Get the prometheus registry for HTTP endpoint
    pub fn registry(&self) -> &Registry {
        &self.registry
    }

    /// Gather all metrics as text for the /metrics endpoint
    pub fn gather_text(&self) -> String {
        use prometheus::TextEncoder;
        let encoder = TextEncoder::new();
        let metric_families = self.registry.gather();
        encoder.encode_to_string(&metric_families)
            .unwrap_or_else(|_| "Metrics encoding error".to_string())
    }
}

#[async_trait]
impl MetricsCollector for PrometheusMetrics {
    fn record_request(&self, _provider: &str, status: &str, latency_secs: f64) {
        self.requests_total
            .with_label_values(&["POST", "/v1/chat/completions", status])
            .inc();
        self.request_duration_seconds
            .with_label_values(&["POST", "/v1/chat/completions"])
            .observe(latency_secs);
    }

    fn record_bytes(&self, direction: &str, bytes: u64) {
        match direction {
            "in" | "received" => {
                self.bytes_received_total.inc_by(bytes as f64);
            }
            "out" | "sent" => {
                self.bytes_sent_total.inc_by(bytes as f64);
            }
            _ => {}
        }
    }

    fn record_connection(&self, delta: i64) {
        if delta > 0 {
            self.active_connections.add(delta as f64);
        } else {
            self.active_connections.sub(delta.unsigned_abs() as f64);
        }
    }

    fn record_rate_limit(&self, route_id: &str) {
        self.rate_limited_requests_total
            .with_label_values(&[route_id])
            .inc();
    }

    fn record_plugin_execution(&self, plugin_name: &str, duration_secs: f64) {
        self.plugin_execution_duration_seconds
            .with_label_values(&[plugin_name])
            .observe(duration_secs);
    }
}

impl Default for PrometheusMetrics {
    fn default() -> Self {
        Self::new()
    }
}
