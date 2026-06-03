use thiserror::Error;

/// Centralized error types for the RustAI Gateway
#[derive(Error, Debug)]
pub enum GatewayError {
    #[error("Configuration error: {0}")]
    Config(String),

    #[error("Upstream connection failed: {0}")]
    UpstreamConnection(String),

    #[error("Upstream timeout after {0:?}")]
    UpstreamTimeout(tokio::time::Duration),

    #[error("Rate limit exceeded: retry after {0:?}")]
    RateLimited(tokio::time::Duration),

    #[error("Authentication failed: {0}")]
    Auth(String),

    #[error("Protocol translation error: {0}")]
    ProtocolTranslation(String),

    #[error("Wasm plugin error: {0}")]
    WasmPlugin(String),

    #[error("MCP error: {0}")]
    Mcp(String),

    #[error("Kubernetes API error: {0}")]
    KubeApi(String),

    #[error("Internal error: {0}")]
    Internal(String),

    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Bad request: {0}")]
    BadRequest(String),

    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error(transparent)]
    Serde(#[from] serde_json::Error),

    #[error(transparent)]
    Anyhow(#[from] anyhow::Error),
}

impl GatewayError {
    /// Get the HTTP status code for this error
    pub fn status_code(&self) -> axum::http::StatusCode {
        use axum::http::StatusCode;
        match self {
            GatewayError::Config(_) => StatusCode::INTERNAL_SERVER_ERROR,
            GatewayError::UpstreamConnection(_) => StatusCode::BAD_GATEWAY,
            GatewayError::UpstreamTimeout(_) => StatusCode::GATEWAY_TIMEOUT,
            GatewayError::RateLimited(_) => StatusCode::TOO_MANY_REQUESTS,
            GatewayError::Auth(_) => StatusCode::UNAUTHORIZED,
            GatewayError::ProtocolTranslation(_) => StatusCode::BAD_GATEWAY,
            GatewayError::WasmPlugin(_) => StatusCode::INTERNAL_SERVER_ERROR,
            GatewayError::Mcp(_) => StatusCode::INTERNAL_SERVER_ERROR,
            GatewayError::KubeApi(_) => StatusCode::BAD_GATEWAY,
            GatewayError::Internal(_) => StatusCode::INTERNAL_SERVER_ERROR,
            GatewayError::NotFound(_) => StatusCode::NOT_FOUND,
            GatewayError::BadRequest(_) => StatusCode::BAD_REQUEST,
            GatewayError::Io(_) => StatusCode::INTERNAL_SERVER_ERROR,
            GatewayError::Serde(_) => StatusCode::BAD_REQUEST,
            GatewayError::Anyhow(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

impl From<GatewayError> for axum::http::StatusCode {
    fn from(err: GatewayError) -> Self {
        err.status_code()
    }
}

/// Implement axum IntoResponse for GatewayError
impl axum::response::IntoResponse for GatewayError {
    fn into_response(self) -> axum::response::Response<axum::body::Body> {
        let status = self.status_code();
        let body = serde_json::json!({
            "error": self.to_string(),
            "code": status.as_u16(),
        });
        let json_bytes = serde_json::to_vec(&body).unwrap_or_default();
        (
            status,
            [(axum::http::header::CONTENT_TYPE, "application/json")],
            json_bytes,
        ).into_response()
    }
}

/// Result type alias for the gateway
pub type GatewayResult<T> = Result<T, GatewayError>;
