use crate::crd::AiGateway;
use futures::StreamExt;
use kube::runtime::watcher;
use kube::Client;
use rustai_core::traits::ConfigSource;
use rustai_core::types::GatewayConfig;
use rustai_core::error::{GatewayError, GatewayResult};
use std::sync::Arc;
use tokio::sync::watch;
use tracing::{error, info, warn};

/// Watches Kubernetes for AiGateway resource changes and updates the proxy configuration
pub struct GatewayWatcher {
    client: Client,
    config_tx: watch::Sender<Arc<GatewayConfig>>,
    config_rx: watch::Receiver<Arc<GatewayConfig>>,
}

impl GatewayWatcher {
    /// Create a new GatewayWatcher with an initial configuration
    pub fn new(client: Client, initial_config: GatewayConfig) -> Self {
        let (tx, rx) = watch::channel(Arc::new(initial_config));
        Self {
            client,
            config_tx: tx,
            config_rx: rx,
        }
    }

    /// Get a receiver for configuration updates
    pub fn subscribe(&self) -> watch::Receiver<Arc<GatewayConfig>> {
        self.config_rx.clone()
    }

    /// Start watching for AiGateway resource changes
    pub async fn start_watching(self) -> GatewayResult<()> {
        info!("Starting Kubernetes Gateway watcher");

        let api = kube::Api::<AiGateway>::all(self.client.clone());

        let watcher_config = watcher::Config::default();

        let mut stream = watcher(api, watcher_config)
            .await
            .map_err(|e| GatewayError::KubeApi(format!("Failed to create watcher: {e}")))?;

        while let Some(event) = stream.next().await {
            match event {
                Ok(event) => {
                    match event {
                        watcher::Event::Applied(gateway) => {
                            info!(
                                name = %gateway.name_any(),
                                "AiGateway resource applied"
                            );
                            self.handle_gateway_change(gateway).await;
                        }
                        watcher::Event::Deleted(gateway) => {
                            info!(
                                name = %gateway.name_any(),
                                "AiGateway resource deleted"
                            );
                            self.handle_gateway_deleted(gateway).await;
                        }
                        _ => {}
                    }
                }
                Err(e) => {
                    error!("Watcher error: {e}");
                }
            }
        }

        Ok(())
    }

    /// Handle a gateway resource being created or updated
    async fn handle_gateway_change(&self, gateway: AiGateway) {
        // TODO: Convert AiGateway to GatewayConfig and send update
        // This would involve:
        // 1. Reading the API key from the referenced secret
        // 2. Building an Upstream configuration
        // 3. Building Routes from the model configurations
        // 4. Updating the shared config

        info!(
            name = %gateway.name_any(),
            provider = %gateway.spec.provider,
            "Gateway configuration change detected"
        );
    }

    /// Handle a gateway resource being deleted
    async fn handle_gateway_deleted(&self, _gateway: AiGateway) {
        info!("Gateway resource deleted");
    }
}
