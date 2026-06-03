use clap::Parser;
use rustai_core::config::ConfigLoader;
use rustai_core::error::{GatewayError, GatewayResult};
use rustai_observability::logging::init_observability;
use rustai_observability::metrics::PrometheusMetrics;
use rustai_router::router::{build_router, AppState};
use std::sync::Arc;
use tracing::info;

/// RustAI Gateway - Cloud-native AI Gateway and Reverse Proxy
#[derive(Parser, Debug)]
#[command(name = "rustai")]
#[command(version, about, long_about = None)]
struct Args {
    /// Path to configuration file
    #[arg(short, long, default_value = "rustai.toml")]
    config: String,

    /// Address to listen on
    #[arg(short, long)]
    listen: Option<String>,

    /// Metrics listen address
    #[arg(long)]
    metrics_addr: Option<String>,

    /// Log level (trace, debug, info, warn, error)
    #[arg(short, long)]
    log_level: Option<String>,
}

#[tokio::main]
async fn main() -> GatewayResult<()> {
    let args = Args::parse();

    // Load configuration
    let mut config = ConfigLoader::from_toml_file(&args.config)?;

    // Override with CLI args
    if let Some(listen) = args.listen {
        config.listen_addr = listen;
    }
    if let Some(metrics_addr) = args.metrics_addr {
        config.metrics_addr = Some(metrics_addr);
    }
    if let Some(log_level) = args.log_level {
        config.log_level = Some(log_level);
    }

    // Initialize observability
    init_observability(&config.name, config.log_level.as_deref())?;

    info!(
        name = %config.name,
        version = %config.version,
        listen = %config.listen_addr,
        "RustAI Gateway starting"
    );

    // Initialize metrics
    let metrics = Arc::new(PrometheusMetrics::new());

    // Start metrics HTTP server if configured
    if let Some(ref metrics_addr) = config.metrics_addr {
        let metrics_clone = metrics.clone();
        let addr = metrics_addr.clone();
        tokio::spawn(async move {
            if let Err(e) = start_metrics_server(&addr, metrics_clone).await {
                tracing::error!("Metrics server error: {e}");
            }
        });
        info!(addr = %metrics_addr, "Metrics endpoint enabled");
    }

    // Build the Axum router
    let app = build_router(config)?;

    // Start the HTTP server
    let listener = tokio::net::TcpListener::bind("0.0.0.0:8080")
        .await
        .map_err(|e| GatewayError::Internal(format!("Failed to bind: {e}")))?;

    info!("Listening on {}", listener.local_addr().unwrap());
    axum::serve(listener, app)
        .await
        .map_err(|e| GatewayError::Internal(format!("Server error: {e}")))?;

    Ok(())
}

/// Start a minimal metrics HTTP server for Prometheus scraping
async fn start_metrics_server(
    addr: &str,
    metrics: Arc<PrometheusMetrics>,
) -> Result<(), Box<dyn std::error::Error>> {
    let app = axum::Router::new()
        .route("/metrics", axum::routing::get(move || {
            let metrics = metrics.clone();
            async move { metrics.gather_text() }
        }));

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}
