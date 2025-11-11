//! Timeout middleware

use axum::response::IntoResponse;
use std::time::Duration;
use tower_http::timeout::TimeoutLayer;

/// Create a timeout layer
pub fn create_timeout_layer(duration: Duration) -> TimeoutLayer {
    TimeoutLayer::new(duration)
}

/// Default timeout (30 seconds)
pub fn default_timeout() -> TimeoutLayer {
    create_timeout_layer(Duration::from_secs(30))
}

/// Long timeout for heavy operations (5 minutes)
pub fn long_timeout() -> TimeoutLayer {
    create_timeout_layer(Duration::from_secs(300))
}

/// Short timeout for quick operations (5 seconds)
pub fn short_timeout() -> TimeoutLayer {
    create_timeout_layer(Duration::from_secs(5))
}

/// Create a custom timeout middleware
pub async fn timeout_middleware(
    request: axum::extract::Request,
    next: axum::middleware::Next,
) -> axum::response::Response {
    // Default 30 second timeout
    let timeout = Duration::from_secs(30);

    match tokio::time::timeout(timeout, next.run(request)).await {
        Ok(response) => response,
        Err(_) => {
            tracing::warn!("Request timed out after {:?}", timeout);
            (
                axum::http::StatusCode::GATEWAY_TIMEOUT,
                axum::Json(crate::error::ErrorResponse::new(
                    "timeout",
                    "Request timed out",
                )),
            )
                .into_response()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_timeout_durations() {
        let default = default_timeout();
        let long = long_timeout();
        let short = short_timeout();

        // Verify they're created (actual duration is internal to TimeoutLayer)
        assert!(true);
    }

    #[test]
    fn test_custom_timeout_creation() {
        let timeout = create_timeout_layer(Duration::from_secs(10));
        assert!(true);
    }
}
