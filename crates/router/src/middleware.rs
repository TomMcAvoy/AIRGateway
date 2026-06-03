use axum::{
    body::Body,
    http::Request,
    middleware::Next,
    response::Response,
};
use rustai_core::error::{GatewayError, GatewayResult};
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::Instant;
use tower::{Layer, Service};
use tracing::{info, warn};

/// Trait for Tower-based middleware layers
#[async_trait::async_trait]
pub trait TowerMiddlewareLayer: Send + Sync {
    fn name(&self) -> &'static str;

    /// Intercept and potentially modify the request
    async fn intercept_request(
        &self,
        req: Request<Body>,
    ) -> GatewayResult<Request<Body>>;

    /// Intercept and potentially modify the response
    async fn intercept_response(
        &self,
        req: &Request<Body>,
        resp: Response<Body>,
    ) -> GatewayResult<Response<Body>>;
}

/// Axum middleware for API key authentication
pub async fn auth_middleware(
    req: Request<Body>,
    next: Next,
) -> Result<Response<Body>, GatewayError> {
    let api_key = req
        .headers()
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .map(|s| s.to_string());

    match api_key {
        Some(_key) => {
            // Key validation would go here against configured keys
            Ok(next.run(req).await)
        }
        None => {
            warn!("Request missing authorization header");
            Err(GatewayError::Auth("Missing API key".into()))
        }
    }
}

/// Axum middleware for request logging
pub async fn logging_middleware(
    req: Request<Body>,
    next: Next,
) -> Result<Response<Body>, GatewayError> {
    let start = Instant::now();
    let method = req.method().clone();
    let uri = req.uri().clone();

    info!(
        method = %method,
        path = %uri.path(),
        "Incoming request"
    );

    let response = next.run(req).await;

    let duration = start.elapsed();
    info!(
        method = %method,
        path = %uri.path(),
        status = %response.status(),
        duration_ms = duration.as_millis(),
        "Request completed"
    );

    Ok(response)
}

/// Axum middleware for rate limiting
pub async fn rate_limit_middleware(
    req: Request<Body>,
    next: Next,
) -> Result<Response<Body>, GatewayError> {
    // Rate limiting logic would go here
    // For now, pass through
    Ok(next.run(req).await)
}

/// Layer that records latency metrics for all requests
#[derive(Clone)]
pub struct LatencyMetricsLayer;

impl<S> Layer<S> for LatencyMetricsLayer {
    type Service = LatencyMetricsService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        LatencyMetricsService { inner }
    }
}

#[derive(Clone)]
pub struct LatencyMetricsService<S> {
    inner: S,
}

impl<S, ReqBody, ResBody> Service<Request<ReqBody>> for LatencyMetricsService<S>
where
    S: Service<Request<ReqBody>, Response = Response<ResBody>>,
    S::Future: Send + 'static,
    S::Error: Send + 'static,
    ReqBody: Send + 'static,
    ResBody: Send + 'static,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, req: Request<ReqBody>) -> Self::Future {
        let start = Instant::now();
        let future = self.inner.call(req);
        Box::pin(async move {
            let result = future.await;
            let duration = start.elapsed();
            if let Ok(ref response) = result {
                tracing::debug!(
                    status = %response.status(),
                    latency_ms = duration.as_micros() as f64 / 1000.0,
                    "Request latency"
                );
            }
            result
        })
    }
}
