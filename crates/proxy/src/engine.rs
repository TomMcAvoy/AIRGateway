use crate::connection::ConnectionPool;
use dashmap::DashMap;
use rustai_core::config::ConfigLoader;
use rustai_core::error::{GatewayError, GatewayResult};
use rustai_core::traits::{BackendTransport, MetricsCollector};
use rustai_core::types::{GatewayConfig, LlmRequest, Provider, Route};
use std::sync::Arc;
use std::time::Duration;
use tokio::net::TcpListener;
use tokio::signal;
use tracing::{error, info, warn};

/// The main proxy engine that manages connections and routes traffic
pub struct ProxyEngine {
    config: Arc<GatewayConfig>,
    transports: DashMap<String, Arc<dyn BackendTransport>>,
    connection_pool: Arc<ConnectionPool>,
    metrics: Option<Arc<dyn MetricsCollector>>,
    routes: Vec<Route>,
}

impl ProxyEngine {
    /// Create a new proxy engine from configuration
    pub async fn from_config(config: GatewayConfig) -> GatewayResult<Self> {
        let config = Arc::new(config);
        let connection_pool = Arc::new(ConnectionPool::new(&config)?);
        let transports: DashMap<String, Arc<dyn BackendTransport>> = DashMap::new();
        let routes = config.routes.clone();

        // Initialize transports for each upstream
        // Note: Transport initialization uses reqwest-based clients
        // managed by the router crate's proxy_handler
        for upstream in &config.upstreams {
            info!(
                upstream_id = %upstream.id,
                provider = %upstream.provider,
                "Registered upstream backend"
            );
        }

        Ok(Self {
            config,
            transports,
            connection_pool,
            metrics: None,
            routes,
        })
    }

    /// Set the metrics collector
    pub fn with_metrics(mut self, metrics: Arc<dyn MetricsCollector>) -> Self {
        self.metrics = Some(metrics);
        self
    }

    /// Get the proxy configuration
    pub fn config(&self) -> Arc<GatewayConfig> {
        self.config.clone()
    }

    /// Get the connection pool
    pub fn connection_pool(&self) -> Arc<ConnectionPool> {
        self.connection_pool.clone()
    }

    /// Get the metrics collector
    pub fn metrics(&self) -> Option<Arc<dyn MetricsCollector>> {
        self.metrics.clone()
    }

    /// Get the configured routes
    pub fn routes(&self) -> &[Route] {
        &self.routes
    }

    /// Start the proxy engine
    pub async fn start(self) -> GatewayResult<()> {
        info!(
            listen = %self.config.listen_addr,
            routes = self.routes.len(),
            upstreams = self.config.upstreams.len(),
            "Proxy engine ready"
        );

        // Spawn metrics server if configured
        if let Some(ref metrics_addr) = self.config.metrics_addr {
            let addr = metrics_addr.clone();
            tokio::spawn(async move {
                info!("Metrics endpoint listening on {}", addr);
                // Metrics server is handled by the main binary via axum
                tokio::time::sleep(Duration::from_secs(u64::MAX)).await;
            });
        }

        // Wait for shutdown signal
        tokio::signal::ctrl_c().await
            .map_err(|e| GatewayError::Internal(format!("Signal error: {e}")))?;

        info!("Shutdown signal received, proxy engine stopping");
        Ok(())
    }

    /// Find the first matching route for a given path and method
    pub fn find_matching_route(&self, path: &str, method: &str) -> Option<Route> {
        self.routes.iter().find(|route| {
            let method_match = route.methods.iter().any(|m| m.eq_ignore_ascii_case(method));
            let path_match = path_matches_pattern(path, &route.path_pattern);
            method_match && path_match
        }).cloned()
    }
}

/// Simple glob-style path matching
fn path_matches_pattern(path: &str, pattern: &str) -> bool {
    if pattern == "/*" || pattern == "/**" {
        return true;
    }
    if pattern.ends_with("/*") {
        let prefix = &pattern[..pattern.len() - 2];
        return path.starts_with(prefix);
    }
    if pattern.ends_with("/**") {
        let prefix = &pattern[..pattern.len() - 3];
        return path.starts_with(prefix);
    }
    path == pattern
}
