use crate::error::{GatewayError, GatewayResult};
use crate::types::GatewayConfig;
use std::path::Path;

/// Configuration loader for the gateway
pub struct ConfigLoader;

impl ConfigLoader {
    /// Load configuration from a TOML file
    pub fn from_toml_file<P: AsRef<Path>>(path: P) -> GatewayResult<GatewayConfig> {
        let content = std::fs::read_to_string(path.as_ref())
            .map_err(|e| GatewayError::Config(format!("Failed to read config file: {e}")))?;
        Self::from_toml_str(&content)
    }

    /// Parse configuration from a TOML string
    pub fn from_toml_str(content: &str) -> GatewayResult<GatewayConfig> {
        let config: GatewayConfig = toml::from_str(content)
            .map_err(|e| GatewayError::Config(format!("Failed to parse TOML config: {e}")))?;
        Self::validate(&config)?;
        Ok(config)
    }

    /// Load configuration from environment variables
    /// Environment variables override file-based config
    pub fn from_env() -> GatewayResult<GatewayConfig> {
        let config_path = std::env::var("RUSTAI_CONFIG")
            .unwrap_or_else(|_| "rustai.toml".to_string());

        let mut config = if Path::new(&config_path).exists() {
            Self::from_toml_file(&config_path)?
        } else {
            GatewayConfig::default()
        };

        // Override with environment variables
        if let Ok(addr) = std::env::var("RUSTAI_LISTEN_ADDR") {
            config.listen_addr = addr;
        }
        if let Ok(addr) = std::env::var("RUSTAI_METRICS_ADDR") {
            config.metrics_addr = Some(addr);
        }
        if let Ok(level) = std::env::var("RUSTAI_LOG_LEVEL") {
            config.log_level = Some(level);
        }

        Ok(config)
    }

    /// Validate the configuration
    fn validate(config: &GatewayConfig) -> GatewayResult<()> {
        if config.upstreams.is_empty() {
            return Err(GatewayError::Config(
                "At least one upstream backend must be configured".into(),
            ));
        }

        for upstream in &config.upstreams {
            if upstream.base_url.as_str().is_empty() {
                return Err(GatewayError::Config(format!(
                    "Upstream '{}' has an empty base URL",
                    upstream.id
                )));
            }
        }

        // Validate TLS config if provided
        if config.listen_tls_addr.is_some() {
            if config.cert_path.is_none() || config.key_path.is_none() {
                return Err(GatewayError::Config(
                    "TLS listen address requires both cert_path and key_path".into(),
                ));
            }
        }

        Ok(())
    }
}

impl Default for GatewayConfig {
    fn default() -> Self {
        Self {
            name: "rustai-gateway".to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            listen_addr: "0.0.0.0:8080".to_string(),
            listen_tls_addr: None,
            cert_path: None,
            key_path: None,
            upstreams: vec![],
            routes: vec![],
            rate_limit_default: None,
            log_level: Some("info".to_string()),
            metrics_addr: Some("0.0.0.0:9090".to_string()),
            wasm_plugins_dir: Some("/etc/rustai/plugins".to_string()),
        }
    }
}
