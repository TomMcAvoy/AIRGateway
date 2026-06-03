use tracing_subscriber::{
    filter::EnvFilter,
    prelude::*,
    registry::Registry,
    Layer,
};
use tracing::info;

/// Configuration for the tracing subsystem
pub struct TracingConfig {
    pub service_name: String,
    pub log_format: LogFormat,
}

pub enum LogFormat {
    Json,
    Text,
}

impl Default for TracingConfig {
    fn default() -> Self {
        Self {
            service_name: "rustai-gateway".to_string(),
            log_format: LogFormat::Json,
        }
    }
}

/// Initialize the tracing and logging subsystem
pub fn init_tracing(config: TracingConfig) -> anyhow::Result<()> {
    let env_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info"));

    let subscriber = Registry::default();

    let fmt_layer = match config.log_format {
        LogFormat::Json => {
            tracing_subscriber::fmt::layer()
                .json()
                .with_target(true)
                .with_thread_ids(true)
                .with_file(true)
                .with_line_number(true)
                .boxed()
        }
        LogFormat::Text => {
            tracing_subscriber::fmt::layer()
                .with_target(true)
                .with_thread_ids(true)
                .boxed()
        }
    };

    subscriber
        .with(fmt_layer.with_filter(env_filter))
        .init();

    info!(
        service = %config.service_name,
        "Tracing and logging initialized"
    );

    Ok(())
}
