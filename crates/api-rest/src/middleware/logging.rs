//! Logging and tracing middleware

use axum::{
    body::Body,
    extract::Request,
    http::StatusCode,
    middleware::Next,
    response::{IntoResponse, Response},
};
use std::time::Instant;
use uuid::Uuid;

use crate::middleware::auth::AuthMethod;

/// Request ID header name
pub const REQUEST_ID_HEADER: &str = "x-request-id";

/// Add request ID to requests
pub async fn request_id_middleware(mut request: Request, next: Next) -> Response {
    // Get or generate request ID
    let request_id = request
        .headers()
        .get(REQUEST_ID_HEADER)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string())
        .unwrap_or_else(|| Uuid::new_v4().to_string());

    // Store in extensions for use by handlers
    request.extensions_mut().insert(RequestId(request_id.clone()));

    // Run the request
    let mut response = next.run(request).await;

    // Add request ID to response headers
    response.headers_mut().insert(
        REQUEST_ID_HEADER,
        request_id.parse().unwrap(),
    );

    response
}

/// Request ID wrapper
#[derive(Clone, Debug)]
pub struct RequestId(pub String);

impl std::fmt::Display for RequestId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Logging middleware
pub async fn logging_middleware(request: Request, next: Next) -> Response {
    let start = Instant::now();
    let method = request.method().clone();
    let uri = request.uri().clone();
    let version = request.version();

    // Get request ID
    let request_id = request
        .extensions()
        .get::<RequestId>()
        .map(|id| id.0.clone())
        .unwrap_or_else(|| "unknown".to_string());

    // Get user ID if authenticated
    let user_id = request
        .extensions()
        .get::<AuthMethod>()
        .map(|auth| auth.user_id())
        .unwrap_or_else(|| "anonymous".to_string());

    tracing::info!(
        request_id = %request_id,
        method = %method,
        uri = %uri,
        version = ?version,
        user_id = %user_id,
        "Request started"
    );

    let response = next.run(request).await;

    let duration = start.elapsed();
    let status = response.status();

    match status.as_u16() {
        200..=299 => tracing::info!(
            request_id = %request_id,
            method = %method,
            uri = %uri,
            status = %status.as_u16(),
            duration_ms = %duration.as_millis(),
            user_id = %user_id,
            "Request completed"
        ),
        300..=399 => tracing::info!(
            request_id = %request_id,
            method = %method,
            uri = %uri,
            status = %status.as_u16(),
            duration_ms = %duration.as_millis(),
            user_id = %user_id,
            "Request completed"
        ),
        400..=499 => tracing::warn!(
            request_id = %request_id,
            method = %method,
            uri = %uri,
            status = %status.as_u16(),
            duration_ms = %duration.as_millis(),
            user_id = %user_id,
            "Request completed"
        ),
        500..=599 => tracing::error!(
            request_id = %request_id,
            method = %method,
            uri = %uri,
            status = %status.as_u16(),
            duration_ms = %duration.as_millis(),
            user_id = %user_id,
            "Request completed"
        ),
        _ => tracing::info!(
            request_id = %request_id,
            method = %method,
            uri = %uri,
            status = %status.as_u16(),
            duration_ms = %duration.as_millis(),
            user_id = %user_id,
            "Request completed"
        ),
    };

    response
}

/// Metrics middleware
pub async fn metrics_middleware(request: Request, next: Next) -> Response {
    let start = Instant::now();
    let method = request.method().clone();
    let uri = request.uri().path().to_string();

    let response = next.run(request).await;

    let duration = start.elapsed();
    let status = response.status();

    // Record metrics (in a real implementation, you'd use a metrics backend)
    tracing::debug!(
        method = %method,
        path = %uri,
        status = %status.as_u16(),
        duration_ms = %duration.as_millis(),
        "Request metrics"
    );

    response
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_request_id_creation() {
        let id = RequestId(Uuid::new_v4().to_string());
        assert!(!id.0.is_empty());
    }

    #[test]
    fn test_request_id_display() {
        let id = RequestId("test-id-123".to_string());
        assert_eq!(format!("{}", id), "test-id-123");
    }
}
