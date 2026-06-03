use rustai_core::error::{GatewayError, GatewayResult};
use rustls::ServerConfig;
use std::fs;
use std::sync::Arc;
use tracing::info;

/// TLS configuration for the proxy server
pub struct TlsConfig;

impl TlsConfig {
    /// Build a rustls ServerConfig from PEM certificate and key files
    pub fn build_server_config(
        cert_path: &str,
        key_path: &str,
    ) -> GatewayResult<Arc<ServerConfig>> {
        info!("Loading TLS certificate from {cert_path}");
        let cert_pem = fs::read_to_string(cert_path)
            .map_err(|e| GatewayError::Config(format!("Failed to read cert file: {e}")))?;

        info!("Loading TLS key from {key_path}");
        let key_pem = fs::read_to_string(key_path)
            .map_err(|e| GatewayError::Config(format!("Failed to read key file: {e}")))?;

        Self::build_from_pem(&cert_pem, &key_pem)
    }

    /// Build a rustls ServerConfig from PEM-encoded strings
    pub fn build_from_pem(
        cert_pem: &str,
        key_pem: &str,
    ) -> GatewayResult<Arc<ServerConfig>> {
        let certs = rustls_pemfile::certs(&mut cert_pem.as_bytes())
            .filter_map(|c| c.ok())
            .map(|c| rustls::pki_types::CertificateDer::from(c))
            .collect::<Vec<_>>();

        if certs.is_empty() {
            return Err(GatewayError::Config("No certificates found in PEM".into()));
        }

        let key = rustls_pemfile::private_key(&mut key_pem.as_bytes())
            .map_err(|e| GatewayError::Config(format!("Failed to parse private key: {e}")))?
            .ok_or_else(|| GatewayError::Config("No private key found in PEM".into()))?;

        let config = ServerConfig::builder()
            .with_no_client_auth()
            .with_single_cert(certs, key)
            .map_err(|e| GatewayError::Config(format!("TLS config error: {e}")))?;

        Ok(Arc::new(config))
    }
}
