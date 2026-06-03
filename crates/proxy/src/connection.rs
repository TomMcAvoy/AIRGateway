use dashmap::DashMap;
use parking_lot::RwLock;
use rustai_core::error::{GatewayError, GatewayResult};
use rustai_core::types::{GatewayConfig, Upstream};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Semaphore;
use tracing::{debug, warn};

/// Manages connection pools to upstream backends
pub struct ConnectionPool {
    pools: DashMap<String, UpstreamPool>,
    max_connections: usize,
}

struct UpstreamPool {
    upstream: Upstream,
    semaphore: Arc<Semaphore>,
    active_connections: Arc<RwLock<u64>>,
    created_at: Instant,
}

impl ConnectionPool {
    pub fn new(config: &GatewayConfig) -> GatewayResult<Self> {
        let pools = DashMap::new();
        let max_connections = 10_000;

        for upstream in &config.upstreams {
            let max = upstream.max_connections.unwrap_or(100) as usize;
            pools.insert(upstream.id.clone(), UpstreamPool {
                upstream: upstream.clone(),
                semaphore: Arc::new(Semaphore::new(max)),
                active_connections: Arc::new(RwLock::new(0)),
                created_at: Instant::now(),
            });
        }

        Ok(Self {
            pools,
            max_connections,
        })
    }

    /// Acquire a connection permit for the given upstream
    pub async fn acquire(&self, upstream_id: &str) -> GatewayResult<ConnectionHandle> {
        let pool = self.pools.get(upstream_id)
            .ok_or_else(|| GatewayError::NotFound(format!("Upstream '{upstream_id}' not found")))?;

        let permit = pool.semaphore.clone().acquire_owned().await
            .map_err(|_| GatewayError::Internal("Failed to acquire connection permit".into()))?;

        {
            let mut count = pool.active_connections.write();
            *count += 1;
        }

        debug!(
            upstream_id = %upstream_id,
            active = *pool.active_connections.read(),
            "Connection acquired"
        );

        let pool_ref = Arc::new(pool.value());
        Ok(ConnectionHandle {
            upstream_id: upstream_id.to_string(),
            _permit: permit,
            active_count: pool.active_connections.clone(),
        })
    }

    /// Get active connection count for debugging
    pub fn active_count(&self, upstream_id: &str) -> u64 {
        self.pools
            .get(upstream_id)
            .map(|p| *p.active_connections.read())
            .unwrap_or(0)
    }

    /// Get total active connections across all pools
    pub fn total_active(&self) -> u64 {
        self.pools
            .iter()
            .map(|p| *p.active_connections.read())
            .sum()
    }
}

/// RAII guard that releases connection permits on drop
pub struct ConnectionHandle {
    upstream_id: String,
    _permit: tokio::sync::OwnedSemaphorePermit,
    active_count: Arc<RwLock<u64>>,
}

impl Drop for ConnectionHandle {
    fn drop(&mut self) {
        let mut count = self.active_count.write();
        if *count > 0 {
            *count -= 1;
        }
        debug!(
            upstream_id = %self.upstream_id,
            active = *count,
            "Connection released"
        );
    }
}
