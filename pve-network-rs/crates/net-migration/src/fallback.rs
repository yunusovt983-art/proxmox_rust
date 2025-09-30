//! Fallback handling for migration system
//!
//! Provides mechanisms to fallback to Perl implementation when Rust fails

use crate::perl_client::{ApiRequest, ApiResponse, PerlApiClient, PerlApiError};
use async_trait::async_trait;
use std::sync::Arc;
use std::time::{Duration, Instant};
use thiserror::Error;

/// Fallback handler errors
#[derive(Debug, Error)]
pub enum FallbackError {
    #[error("Perl API error: {0}")]
    PerlApi(#[from] PerlApiError),

    #[error("Fallback disabled for this operation")]
    Disabled,

    #[error("Fallback timeout exceeded")]
    Timeout,

    #[error("Both Rust and Perl implementations failed")]
    BothFailed {
        rust_error: String,
        perl_error: String,
    },
}

/// Result of a fallback operation
#[derive(Debug, Clone)]
pub struct FallbackResult {
    pub response: ApiResponse,
    pub used_fallback: bool,
    pub rust_error: Option<String>,
    pub execution_time: Duration,
}

/// Trait for handling fallback operations
#[async_trait]
pub trait FallbackHandler: Send + Sync {
    /// Execute operation with fallback capability
    async fn execute_with_fallback(
        &self,
        request: &ApiRequest,
        rust_operation: Box<
            dyn FnOnce() -> BoxFuture<
                    'static,
                    std::result::Result<ApiResponse, Box<dyn std::error::Error + Send + Sync>>,
                > + Send,
        >,
        fallback_enabled: bool,
        timeout: Duration,
    ) -> Result<FallbackResult, FallbackError>;

    /// Check if Perl API is healthy
    async fn check_perl_health(&self) -> bool;
}

// Type alias for boxed future
pub type BoxFuture<'a, T> = std::pin::Pin<Box<dyn std::future::Future<Output = T> + Send + 'a>>;

/// Default fallback handler implementation
pub struct DefaultFallbackHandler {
    perl_client: Arc<dyn PerlApiClient>,
}

impl DefaultFallbackHandler {
    pub fn new(perl_client: Arc<dyn PerlApiClient>) -> Self {
        Self { perl_client }
    }
}

#[async_trait]
impl FallbackHandler for DefaultFallbackHandler {
    async fn execute_with_fallback(
        &self,
        request: &ApiRequest,
        rust_operation: Box<
            dyn FnOnce() -> BoxFuture<
                    'static,
                    std::result::Result<ApiResponse, Box<dyn std::error::Error + Send + Sync>>,
                > + Send,
        >,
        fallback_enabled: bool,
        timeout: Duration,
    ) -> Result<FallbackResult, FallbackError> {
        let start_time = Instant::now();

        // Try Rust implementation first
        log::debug!(
            "Attempting Rust implementation for {} {}",
            request.method,
            request.path
        );

        let rust_result = tokio::time::timeout(timeout, rust_operation()).await;

        match rust_result {
            Ok(Ok(response)) => {
                // Rust succeeded
                log::debug!(
                    "Rust implementation succeeded for {} {}",
                    request.method,
                    request.path
                );
                Ok(FallbackResult {
                    response,
                    used_fallback: false,
                    rust_error: None,
                    execution_time: start_time.elapsed(),
                })
            }
            Ok(Err(rust_error)) => {
                // Rust failed
                let rust_error_msg = rust_error.to_string();

                log::warn!(
                    "Rust implementation failed for {} {}: {}",
                    request.method,
                    request.path,
                    rust_error_msg
                );

                if !fallback_enabled {
                    return Err(FallbackError::Disabled);
                }

                // Try Perl fallback
                log::info!(
                    "Falling back to Perl for {} {}",
                    request.method,
                    request.path
                );

                match self.perl_client.call(request).await {
                    Ok(response) => {
                        log::info!(
                            "Perl fallback succeeded for {} {}",
                            request.method,
                            request.path
                        );
                        Ok(FallbackResult {
                            response,
                            used_fallback: true,
                            rust_error: Some(rust_error_msg),
                            execution_time: start_time.elapsed(),
                        })
                    }
                    Err(perl_error) => {
                        log::error!(
                            "Both Rust and Perl failed for {} {}: Rust={}, Perl={}",
                            request.method,
                            request.path,
                            rust_error_msg,
                            perl_error
                        );
                        Err(FallbackError::BothFailed {
                            rust_error: rust_error_msg,
                            perl_error: perl_error.to_string(),
                        })
                    }
                }
            }
            Err(_) => {
                // Timeout
                let rust_error_msg = "Timeout".to_string();

                log::warn!(
                    "Rust implementation timed out for {} {}",
                    request.method,
                    request.path
                );

                if !fallback_enabled {
                    return Err(FallbackError::Disabled);
                }

                // Try Perl fallback
                log::info!(
                    "Falling back to Perl for {} {}",
                    request.method,
                    request.path
                );

                match self.perl_client.call(request).await {
                    Ok(response) => {
                        log::info!(
                            "Perl fallback succeeded for {} {}",
                            request.method,
                            request.path
                        );
                        Ok(FallbackResult {
                            response,
                            used_fallback: true,
                            rust_error: Some(rust_error_msg),
                            execution_time: start_time.elapsed(),
                        })
                    }
                    Err(perl_error) => {
                        log::error!(
                            "Both Rust and Perl failed for {} {}: Rust={}, Perl={}",
                            request.method,
                            request.path,
                            rust_error_msg,
                            perl_error
                        );
                        Err(FallbackError::BothFailed {
                            rust_error: rust_error_msg,
                            perl_error: perl_error.to_string(),
                        })
                    }
                }
            }
        }
    }

    async fn check_perl_health(&self) -> bool {
        match self.perl_client.health_check().await {
            Ok(healthy) => healthy,
            Err(e) => {
                log::warn!("Perl health check failed: {}", e);
                false
            }
        }
    }
}

/// Metrics collector for fallback operations
#[derive(Debug, Default, Clone, Copy)]
pub struct FallbackMetrics {
    pub total_requests: u64,
    pub rust_successes: u64,
    pub rust_failures: u64,
    pub fallback_successes: u64,
    pub fallback_failures: u64,
    pub total_fallbacks: u64,
    pub average_rust_time: Duration,
    pub average_fallback_time: Duration,
}

impl FallbackMetrics {
    pub fn record_result(&mut self, result: &FallbackResult) {
        self.total_requests += 1;

        if result.used_fallback {
            self.total_fallbacks += 1;
            self.rust_failures += 1;
            self.fallback_successes += 1;

            // Update average fallback time
            let total_time =
                self.average_fallback_time.as_millis() as u64 * (self.fallback_successes - 1);
            let new_total = total_time + result.execution_time.as_millis() as u64;
            self.average_fallback_time = Duration::from_millis(new_total / self.fallback_successes);
        } else {
            self.rust_successes += 1;

            // Update average Rust time
            let total_time = self.average_rust_time.as_millis() as u64 * (self.rust_successes - 1);
            let new_total = total_time + result.execution_time.as_millis() as u64;
            self.average_rust_time = Duration::from_millis(new_total / self.rust_successes);
        }
    }

    pub fn record_failure(&mut self, used_fallback: bool) {
        self.total_requests += 1;

        if used_fallback {
            self.total_fallbacks += 1;
            self.rust_failures += 1;
            self.fallback_failures += 1;
        } else {
            self.rust_failures += 1;
        }
    }

    pub fn success_rate(&self) -> f64 {
        if self.total_requests == 0 {
            return 0.0;
        }

        let successes = self.rust_successes + self.fallback_successes;
        successes as f64 / self.total_requests as f64
    }

    pub fn fallback_rate(&self) -> f64 {
        if self.total_requests == 0 {
            return 0.0;
        }

        self.total_fallbacks as f64 / self.total_requests as f64
    }

    pub fn rust_success_rate(&self) -> f64 {
        let rust_attempts = self.rust_successes + self.rust_failures;
        if rust_attempts == 0 {
            return 0.0;
        }

        self.rust_successes as f64 / rust_attempts as f64
    }
}

/// Fallback handler with metrics collection
pub struct MetricsFallbackHandler {
    inner: DefaultFallbackHandler,
    metrics: std::sync::Mutex<FallbackMetrics>,
}

impl MetricsFallbackHandler {
    pub fn new(perl_client: Arc<dyn PerlApiClient>) -> Self {
        Self {
            inner: DefaultFallbackHandler::new(perl_client),
            metrics: std::sync::Mutex::new(FallbackMetrics::default()),
        }
    }

    pub fn get_metrics(&self) -> FallbackMetrics {
        *self.metrics.lock().unwrap()
    }

    pub fn reset_metrics(&self) {
        *self.metrics.lock().unwrap() = FallbackMetrics::default();
    }
}

#[async_trait]
impl FallbackHandler for MetricsFallbackHandler {
    async fn execute_with_fallback(
        &self,
        request: &ApiRequest,
        rust_operation: Box<
            dyn FnOnce() -> BoxFuture<
                    'static,
                    std::result::Result<ApiResponse, Box<dyn std::error::Error + Send + Sync>>,
                > + Send,
        >,
        fallback_enabled: bool,
        timeout: Duration,
    ) -> Result<FallbackResult, FallbackError> {
        let result = self
            .inner
            .execute_with_fallback(request, rust_operation, fallback_enabled, timeout)
            .await;

        // Record metrics
        match &result {
            Ok(fallback_result) => {
                self.metrics.lock().unwrap().record_result(fallback_result);
            }
            Err(_) => {
                self.metrics
                    .lock()
                    .unwrap()
                    .record_failure(fallback_enabled);
            }
        }

        result
    }

    async fn check_perl_health(&self) -> bool {
        self.inner.check_perl_health().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::perl_client::{ApiRequest, ApiResponse, MockPerlApiClient};
    use reqwest::Method;
    use serde_json::json;
    use std::collections::HashMap;

    #[tokio::test]
    async fn test_successful_rust_operation() {
        let perl_client = Arc::new(MockPerlApiClient::new());
        let handler = DefaultFallbackHandler::new(perl_client);

        let request = ApiRequest {
            method: Method::GET,
            path: "/test".to_string(),
            query_params: HashMap::new(),
            body: None,
            headers: HashMap::new(),
        };

        let rust_op = Box::new(|| {
            Box::pin(async {
                Ok(ApiResponse {
                    status: 200,
                    headers: HashMap::new(),
                    body: json!({"success": true}),
                })
            })
                as BoxFuture<
                    'static,
                    std::result::Result<ApiResponse, Box<dyn std::error::Error + Send + Sync>>,
                >
        });

        let result = handler
            .execute_with_fallback(&request, rust_op, true, Duration::from_secs(5))
            .await
            .unwrap();

        assert!(!result.used_fallback);
        assert!(result.rust_error.is_none());
        assert_eq!(result.response.status, 200);
    }

    #[tokio::test]
    async fn test_fallback_on_rust_failure() {
        let mut perl_client = MockPerlApiClient::new();
        perl_client.add_response(
            "GET /test".to_string(),
            ApiResponse {
                status: 200,
                headers: HashMap::new(),
                body: json!({"fallback": true}),
            },
        );

        let handler = DefaultFallbackHandler::new(Arc::new(perl_client));

        let request = ApiRequest {
            method: Method::GET,
            path: "/test".to_string(),
            query_params: HashMap::new(),
            body: None,
            headers: HashMap::new(),
        };

        let rust_op = Box::new(|| {
            Box::pin(async {
                Err(Box::new(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    "Rust failed",
                ))
                    as Box<dyn std::error::Error + Send + Sync>)
            })
                as BoxFuture<
                    'static,
                    std::result::Result<ApiResponse, Box<dyn std::error::Error + Send + Sync>>,
                >
        });

        let result = handler
            .execute_with_fallback(&request, rust_op, true, Duration::from_secs(5))
            .await
            .unwrap();

        assert!(result.used_fallback);
        assert!(result.rust_error.is_some());
        assert_eq!(result.response.body["fallback"], true);
    }

    #[tokio::test]
    async fn test_fallback_disabled() {
        let perl_client = Arc::new(MockPerlApiClient::new());
        let handler = DefaultFallbackHandler::new(perl_client);

        let request = ApiRequest {
            method: Method::GET,
            path: "/test".to_string(),
            query_params: HashMap::new(),
            body: None,
            headers: HashMap::new(),
        };

        let rust_op = Box::new(|| {
            Box::pin(async {
                Err(Box::new(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    "Rust failed",
                ))
                    as Box<dyn std::error::Error + Send + Sync>)
            })
                as BoxFuture<
                    'static,
                    std::result::Result<ApiResponse, Box<dyn std::error::Error + Send + Sync>>,
                >
        });

        let result = handler
            .execute_with_fallback(
                &request,
                rust_op,
                false, // Fallback disabled
                Duration::from_secs(5),
            )
            .await;

        assert!(matches!(result, Err(FallbackError::Disabled)));
    }

    #[test]
    fn test_metrics() {
        let mut metrics = FallbackMetrics::default();

        // Record successful Rust operation
        let rust_result = FallbackResult {
            response: ApiResponse {
                status: 200,
                headers: HashMap::new(),
                body: json!({}),
            },
            used_fallback: false,
            rust_error: None,
            execution_time: Duration::from_millis(100),
        };
        metrics.record_result(&rust_result);

        // Record fallback operation
        let fallback_result = FallbackResult {
            response: ApiResponse {
                status: 200,
                headers: HashMap::new(),
                body: json!({}),
            },
            used_fallback: true,
            rust_error: Some("Error".to_string()),
            execution_time: Duration::from_millis(200),
        };
        metrics.record_result(&fallback_result);

        assert_eq!(metrics.total_requests, 2);
        assert_eq!(metrics.rust_successes, 1);
        assert_eq!(metrics.rust_failures, 1);
        assert_eq!(metrics.total_fallbacks, 1);
        assert_eq!(metrics.fallback_rate(), 0.5);
        assert_eq!(metrics.rust_success_rate(), 0.5);
    }
}
