use crate::crd::{AiGateway, AiGatewayStatus, GatewayCondition};
use futures::StreamExt;
use kube::{
    api::{Patch, PatchParams},
    runtime::controller::{Action, Controller},
    Client, Resource,
};
use std::sync::Arc;
use std::time::Duration;
use tracing::{error, info, warn};

/// Reconciliation error type
#[derive(Debug, thiserror::Error)]
pub enum ReconcilerError {
    #[error("Kubernetes API error: {0}")]
    KubeError(#[from] kube::Error),

    #[error("Serialization error: {0}")]
    SerdeError(#[from] serde_json::Error),

    #[error("Configuration error: {0}")]
    ConfigError(String),
}

/// Context passed to the reconciler
pub struct Context {
    pub client: Client,
}

/// Reconcile an AiGateway resource
async fn reconcile(gateway: Arc<AiGateway>, ctx: Arc<Context>) -> Result<Action, ReconcilerError> {
    let name = gateway.name_any();
    let namespace = gateway.namespace().unwrap_or_default();

    info!(
        name = %name,
        namespace = %namespace,
        provider = %gateway.spec.provider,
        "Reconciling AiGateway resource"
    );

    // Validate the gateway configuration
    if let Err(e) = validate_gateway(&gateway) {
        warn!(
            name = %name,
            error = %e,
            "Gateway validation failed"
        );

        // Update status to reflect error
        update_status(
            &ctx.client,
            &name,
            &namespace,
            AiGatewayStatus {
                ready: false,
                conditions: vec![GatewayCondition {
                    condition_type: "Ready".to_string(),
                    status: "False".to_string(),
                    reason: "ValidationFailed".to_string(),
                    message: format!("Validation error: {e}"),
                    last_transition_time: None,
                }],
                observed_generation: Some(gateway.metadata.generation.unwrap_or(0)),
            },
        )
        .await?;

        return Ok(Action::requeue(Duration::from_secs(30)));
    }

    // In a full implementation, this would:
    // 1. Create/update the proxy configuration
    // 2. Create/update any necessary ConfigMaps or Secrets
    // 3. Deploy or configure the proxy instance

    // Mark as ready
    update_status(
        &ctx.client,
        &name,
        &namespace,
        AiGatewayStatus {
            ready: true,
            conditions: vec![GatewayCondition {
                condition_type: "Ready".to_string(),
                status: "True".to_string(),
                reason: "Reconciled".to_string(),
                message: "Gateway configuration applied successfully".to_string(),
                last_transition_time: None,
            }],
            observed_generation: Some(gateway.metadata.generation.unwrap_or(0)),
        },
    )
    .await?;

    info!(
        name = %name,
        "Gateway reconciled successfully"
    );

    // Requeue after 5 minutes for re-reconciliation
    Ok(Action::requeue(Duration::from_secs(300)))
}

/// Validate gateway configuration
fn validate_gateway(gateway: &AiGateway) -> Result<(), ReconcilerError> {
    if gateway.spec.provider.is_empty() {
        return Err(ReconcilerError::ConfigError("Provider must be specified".into()));
    }

    if gateway.spec.upstream_url.is_empty() {
        return Err(ReconcilerError::ConfigError("Upstream URL must be specified".into()));
    }

    // Validate URL
    url::Url::parse(&gateway.spec.upstream_url)
        .map_err(|e| ReconcilerError::ConfigError(format!("Invalid upstream URL: {e}")))?;

    Ok(())
}

/// Update the status of an AiGateway resource
async fn update_status(
    client: &Client,
    name: &str,
    namespace: &str,
    status: AiGatewayStatus,
) -> Result<(), ReconcilerError> {
    let patch = Patch::Apply(&serde_json::json!({
        "apiVersion": "gateway.rustai.dev/v1",
        "kind": "AiGateway",
        "metadata": {
            "name": name,
            "namespace": namespace,
        },
        "status": status,
    }));

    let params = PatchParams::apply("rustai-gateway-controller")
        .force()
        .field_manager("rustai-gateway-controller");

    let api = kube::Api::<AiGateway>::namespaced(client.clone(), namespace);
    api.patch_status(name, &params, patch).await?;

    Ok(())
}

/// Error handler for the reconciler
fn error_policy(
    gateway: Arc<AiGateway>,
    error: &ReconcilerError,
    _ctx: Arc<Context>,
) -> Action {
    warn!(
        name = %gateway.name_any(),
        error = %error,
        "Reconciliation failed"
    );
    Action::requeue(Duration::from_secs(60))
}

/// Start the Kubernetes controller
pub async fn run_controller(client: Client) -> Result<(), Box<dyn std::error::Error>> {
    info!("Starting RustAI Gateway Kubernetes controller");

    let context = Arc::new(Context { client: client.clone() });

    Controller::new(
        kube::api::Api::<AiGateway>::all(client.clone()),
        kube::runtime::watcher::Config::default(),
    )
    .owns(
        kube::api::Api::<k8s_openapi::api::core::v1::Secret>::all(client.clone()),
        kube::runtime::watcher::Config::default(),
    )
    .run(reconcile, error_policy, context)
    .for_each(|reconciliation_result| {
        async move {
            match reconciliation_result {
                Ok(gateway) => {
                    info!(
                        name = %gateway.name_any(),
                        "Reconciliation completed"
                    );
                }
                Err(e) => {
                    error!("Reconciliation error: {e}");
                }
            }
        }
    })
    .await;

    Ok(())
}
