use crate::tracing::{init_tracing, TracingConfig, LogFormat};
use rustai_core::error::GatewayResult;

/// Initialize the full observability stack
pub fn init_observability(service_name: &str, log_level: Option<&str>) -> GatewayResult<()> {
    // Set RUST_LOG from config if not already set
    if let Some(level) = log_level {
        if std::env::var("RUST_LOG").is_err() {
            std::env::set_var("RUST_LOG", level);
        }
    }

    let config = TracingConfig {
        service_name: service_name.to_string(),
        log_format: LogFormat::Json,
    };

    init_tracing(config)
        .map_err(|e| rustai_core::error::GatewayError::Internal(format!("Tracing init error: {e}")))?;

    tracing::info!(
        service = %service_name,
        "Observability stack initialized"
    );

    Ok(())
}
