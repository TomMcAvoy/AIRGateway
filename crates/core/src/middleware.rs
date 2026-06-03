use crate::error::GatewayResult;
use crate::traits::GatewayMiddleware;
use crate::types::LlmRequest;

/// Composite middleware that chains multiple middlewares together
pub struct MiddlewareChain {
    middlewares: Vec<Box<dyn GatewayMiddleware>>,
}

impl MiddlewareChain {
    pub fn new() -> Self {
        Self {
            middlewares: Vec::new(),
        }
    }

    pub fn with(mut self, middleware: impl GatewayMiddleware + 'static) -> Self {
        self.middlewares.push(Box::new(middleware));
        self
    }

    /// Apply all middlewares in pre-processing order
    pub async fn pre_process(&self, mut request: LlmRequest) -> GatewayResult<LlmRequest> {
        for middleware in &self.middlewares {
            request = middleware.pre_process(request).await?;
        }
        Ok(request)
    }

    /// Apply all middlewares in post-processing order (reversed)
    pub async fn post_process(
        &self,
        response: Box<dyn crate::traits::LlmResponse>,
    ) -> GatewayResult<Box<dyn crate::traits::LlmResponse>> {
        let mut response = response;
        for middleware in self.middlewares.iter().rev() {
            response = middleware.post_process(response).await?;
        }
        Ok(response)
    }
}

impl Default for MiddlewareChain {
    fn default() -> Self {
        Self::new()
    }
}

/// Authentication middleware for API key validation
pub struct AuthMiddleware {
    valid_keys: Vec<String>,
}

impl AuthMiddleware {
    pub fn new(keys: Vec<String>) -> Self {
        Self { valid_keys: keys }
    }
}

#[async_trait::async_trait]
impl GatewayMiddleware for AuthMiddleware {
    fn name(&self) -> &'static str {
        "auth"
    }

    async fn pre_process(
        &self,
        request: LlmRequest,
    ) -> GatewayResult<LlmRequest> {
        // Auth check happens in the Tower layer; this is a no-op
        Ok(request)
    }

    async fn post_process(
        &self,
        response: Box<dyn crate::traits::LlmResponse>,
    ) -> GatewayResult<Box<dyn crate::traits::LlmResponse>> {
        Ok(response)
    }
}

/// Request logging middleware
pub struct LoggingMiddleware;

#[async_trait::async_trait]
impl GatewayMiddleware for LoggingMiddleware {
    fn name(&self) -> &'static str {
        "logging"
    }

    async fn pre_process(
        &self,
        request: LlmRequest,
    ) -> GatewayResult<LlmRequest> {
        tracing::info!(
            request_id = %request.request_id,
            model = %request.model,
            provider = %request.provider,
            stream = request.stream,
            "Processing LLM request"
        );
        Ok(request)
    }

    async fn post_process(
        &self,
        response: Box<dyn crate::traits::LlmResponse>,
    ) -> GatewayResult<Box<dyn crate::traits::LlmResponse>> {
        tracing::info!("Completed LLM response processing");
        Ok(response)
    }
}
