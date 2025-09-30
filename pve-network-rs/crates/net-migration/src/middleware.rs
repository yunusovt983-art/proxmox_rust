//! Migration middleware for HTTP request routing
//!
//! Provides middleware to route requests between Perl and Rust implementations

use crate::config::MigrationConfig;
use crate::fallback::{BoxFuture, FallbackHandler, FallbackResult, MetricsFallbackHandler};
use crate::perl_client::{ApiRequest, ApiResponse, PerlApiClient};
use crate::{MigrationError, Result};

use async_trait::async_trait;
use http::{Method, StatusCode};
use hyper::{Body, Request, Response};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

/// Trait for Rust API handlers
#[async_trait]
pub trait RustApiHandler: Send + Sync {
    async fn handle_request(&self, request: &ApiRequest) -> Result<ApiResponse>;
}

/// Migration middleware for routing requests
pub struct MigrationMiddleware {
    config: Arc<MigrationConfig>,
    rust_handler: Arc<dyn RustApiHandler>,
    fallback_handler: MetricsFallbackHandler,
}

impl MigrationMiddleware {
    pub fn new(
        config: MigrationConfig,
        rust_handler: Arc<dyn RustApiHandler>,
        perl_client: Arc<dyn PerlApiClient>,
    ) -> Self {
        let fallback_handler = MetricsFallbackHandler::new(perl_client);

        Self {
            config: Arc::new(config),
            rust_handler,
            fallback_handler,
        }
    }

    /// Handle incoming HTTP request
    pub async fn handle_request(&self, req: Request<Body>) -> Result<Response<Body>> {
        let (parts, body) = req.into_parts();

        // Extract request information
        let method = parts.method.clone();
        let path = parts.uri.path().to_string();
        let query_params = self.extract_query_params(&parts.uri);
        let headers = self.extract_headers(&parts.headers);

        // Parse body if present
        let body_bytes = hyper::body::to_bytes(body)
            .await
            .map_err(|e| MigrationError::Fallback(format!("Failed to read request body: {}", e)))?;

        let body_value = if body_bytes.is_empty() {
            None
        } else {
            let body_str = String::from_utf8_lossy(&body_bytes);
            Some(
                serde_json::from_str(&body_str)
                    .map_err(|e| MigrationError::Fallback(format!("Invalid JSON body: {}", e)))?,
            )
        };

        let api_request = ApiRequest {
            method: self.convert_method(&method),
            path: path.clone(),
            query_params,
            body: body_value,
            headers,
        };

        // Log the request
        if self.config.log_migration_decisions {
            log::info!("Processing request: {} {}", method, path);
        }

        // Determine routing strategy
        let should_use_rust = self.config.should_use_rust(&path, method.as_str());
        let should_fallback = self.config.should_fallback(&path);
        let timeout = Duration::from_secs(self.config.get_rust_timeout(&path));

        if self.config.log_migration_decisions {
            log::debug!(
                "Routing decision for {} {}: rust={}, fallback={}",
                method,
                path,
                should_use_rust,
                should_fallback
            );
        }

        let result = if should_use_rust {
            // Try Rust with potential fallback
            let rust_handler = Arc::clone(&self.rust_handler);
            let api_request_clone = api_request.clone();

            let rust_operation = Box::new(|| {
                Box::pin(async move {
                    rust_handler
                        .handle_request(&api_request_clone)
                        .await
                        .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
                })
                    as BoxFuture<
                        'static,
                        std::result::Result<ApiResponse, Box<dyn std::error::Error + Send + Sync>>,
                    >
            });

            self.fallback_handler
                .execute_with_fallback(&api_request, rust_operation, should_fallback, timeout)
                .await
        } else {
            // Direct Perl call
            let perl_response = self
                .fallback_handler
                .check_perl_health()
                .await
                .then(|| async {
                    // Create a dummy Rust operation that always fails
                    let failing_operation = Box::new(|| {
                        Box::pin(async {
                            Err(Box::new(MigrationError::EndpointNotAvailable)
                                as Box<dyn std::error::Error + Send + Sync>)
                        })
                            as BoxFuture<
                                'static,
                                std::result::Result<
                                    ApiResponse,
                                    Box<dyn std::error::Error + Send + Sync>,
                                >,
                            >
                    });

                    self.fallback_handler
                        .execute_with_fallback(
                            &api_request,
                            failing_operation,
                            true, // Force fallback
                            timeout,
                        )
                        .await
                });

            match perl_response {
                Some(result) => result.await,
                None => Err(crate::fallback::FallbackError::PerlApi(
                    crate::perl_client::PerlApiError::ApiError {
                        status: 503,
                        message: "Perl API unavailable".to_string(),
                    },
                )
                .into()),
            }
        };

        // Convert result to HTTP response
        match result {
            Ok(fallback_result) => {
                if self.config.log_migration_decisions {
                    log::info!(
                        "Request {} {} completed: fallback={}, time={:?}",
                        method,
                        path,
                        fallback_result.used_fallback,
                        fallback_result.execution_time
                    );
                }

                self.build_http_response(fallback_result)
            }
            Err(e) => {
                log::error!("Request {} {} failed: {}", method, path, e);
                self.build_error_response(e)
            }
        }
    }

    fn extract_query_params(&self, uri: &hyper::Uri) -> HashMap<String, String> {
        let mut params = HashMap::new();

        if let Some(query) = uri.query() {
            for pair in query.split('&') {
                if let Some((key, value)) = pair.split_once('=') {
                    params.insert(
                        urlencoding::decode(key).unwrap_or_default().into_owned(),
                        urlencoding::decode(value).unwrap_or_default().into_owned(),
                    );
                }
            }
        }

        params
    }

    fn extract_headers(&self, headers: &hyper::HeaderMap) -> HashMap<String, String> {
        let mut header_map = HashMap::new();

        for (key, value) in headers {
            if let Ok(value_str) = value.to_str() {
                header_map.insert(key.to_string(), value_str.to_string());
            }
        }

        header_map
    }

    fn convert_method(&self, method: &Method) -> reqwest::Method {
        match *method {
            Method::GET => reqwest::Method::GET,
            Method::POST => reqwest::Method::POST,
            Method::PUT => reqwest::Method::PUT,
            Method::DELETE => reqwest::Method::DELETE,
            Method::PATCH => reqwest::Method::PATCH,
            Method::HEAD => reqwest::Method::HEAD,
            Method::OPTIONS => reqwest::Method::OPTIONS,
            _ => reqwest::Method::GET, // Default fallback
        }
    }

    fn build_http_response(&self, result: FallbackResult) -> Result<Response<Body>> {
        let mut response_builder = Response::builder().status(result.response.status);

        // Add headers
        for (key, value) in result.response.headers {
            response_builder = response_builder.header(key, value);
        }

        // Add migration metadata headers
        response_builder = response_builder
            .header(
                "X-PVE-Migration-Used-Fallback",
                result.used_fallback.to_string(),
            )
            .header(
                "X-PVE-Migration-Execution-Time",
                format!("{}ms", result.execution_time.as_millis()),
            );

        if let Some(ref rust_error) = result.rust_error {
            response_builder = response_builder.header("X-PVE-Migration-Rust-Error", rust_error);
        }

        // Serialize body
        let body_str = serde_json::to_string(&result.response.body).map_err(|e| {
            MigrationError::Fallback(format!("Failed to serialize response: {}", e))
        })?;

        let response = response_builder
            .body(Body::from(body_str))
            .map_err(|e| MigrationError::Fallback(format!("Failed to build response: {}", e)))?;

        Ok(response)
    }

    fn build_error_response(
        &self,
        error: crate::fallback::FallbackError,
    ) -> Result<Response<Body>> {
        let (status, message) = match error {
            crate::fallback::FallbackError::Disabled => (
                StatusCode::SERVICE_UNAVAILABLE,
                "Migration fallback disabled".to_string(),
            ),
            crate::fallback::FallbackError::Timeout => {
                (StatusCode::GATEWAY_TIMEOUT, "Operation timeout".to_string())
            }
            crate::fallback::FallbackError::BothFailed {
                rust_error,
                perl_error,
            } => (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!(
                    "Both implementations failed: Rust={}, Perl={}",
                    rust_error, perl_error
                ),
            ),
            crate::fallback::FallbackError::PerlApi(e) => match e {
                crate::perl_client::PerlApiError::ApiError { status, message } => (
                    StatusCode::from_u16(status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR),
                    message,
                ),
                _ => (StatusCode::BAD_GATEWAY, e.to_string()),
            },
        };

        let error_body = serde_json::json!({
            "error": message,
            "migration_error": true
        });

        let response = Response::builder()
            .status(status)
            .header("Content-Type", "application/json")
            .header("X-PVE-Migration-Error", "true")
            .body(Body::from(error_body.to_string()))
            .map_err(|e| {
                MigrationError::Fallback(format!("Failed to build error response: {}", e))
            })?;

        Ok(response)
    }

    /// Get current migration metrics
    pub fn get_metrics(&self) -> crate::fallback::FallbackMetrics {
        self.fallback_handler.get_metrics()
    }

    /// Update migration configuration
    pub fn update_config(&mut self, new_config: MigrationConfig) {
        self.config = Arc::new(new_config);
    }

    /// Check system health
    pub async fn health_check(&self) -> HashMap<String, Value> {
        let mut health = HashMap::new();

        // Check Perl API health
        let perl_healthy = self.fallback_handler.check_perl_health().await;
        health.insert("perl_api".to_string(), Value::Bool(perl_healthy));

        // Add migration config info
        health.insert(
            "migration_phase".to_string(),
            Value::String(format!("{:?}", self.config.phase)),
        );
        health.insert(
            "fallback_enabled".to_string(),
            Value::Bool(self.config.fallback_enabled),
        );

        // Add metrics
        let metrics = self.get_metrics();
        health.insert(
            "metrics".to_string(),
            serde_json::json!({
                "total_requests": metrics.total_requests,
                "rust_success_rate": metrics.rust_success_rate(),
                "fallback_rate": metrics.fallback_rate(),
                "overall_success_rate": metrics.success_rate(),
            }),
        );

        health
    }
}

// Simplified metrics access - removed complex downcasting

/// Mock Rust API handler for testing
pub struct MockRustApiHandler {
    responses: HashMap<String, ApiResponse>,
    should_fail: bool,
}

impl MockRustApiHandler {
    pub fn new() -> Self {
        Self {
            responses: HashMap::new(),
            should_fail: false,
        }
    }

    pub fn add_response(&mut self, method_path: String, response: ApiResponse) {
        self.responses.insert(method_path, response);
    }

    pub fn set_should_fail(&mut self, should_fail: bool) {
        self.should_fail = should_fail;
    }

    fn make_key(&self, request: &ApiRequest) -> String {
        format!("{} {}", request.method, request.path)
    }
}

impl Default for MockRustApiHandler {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl RustApiHandler for MockRustApiHandler {
    async fn handle_request(&self, request: &ApiRequest) -> Result<ApiResponse> {
        if self.should_fail {
            return Err(MigrationError::Fallback("Mock failure".to_string()));
        }

        let key = self.make_key(request);

        if let Some(response) = self.responses.get(&key) {
            Ok(response.clone())
        } else {
            Err(MigrationError::EndpointNotAvailable)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::perl_client::MockPerlApiClient;
    use serde_json::json;

    #[tokio::test]
    async fn test_middleware_rust_success() {
        let config = MigrationConfig {
            phase: crate::config::MigrationPhase::RustFull,
            ..Default::default()
        };

        let mut rust_handler = MockRustApiHandler::new();
        rust_handler.add_response(
            "GET /api2/json/test".to_string(),
            ApiResponse {
                status: 200,
                headers: HashMap::new(),
                body: json!({"rust": true}),
            },
        );

        let perl_client = Arc::new(MockPerlApiClient::new());
        let middleware = MigrationMiddleware::new(config, Arc::new(rust_handler), perl_client);

        let request = Request::builder()
            .method("GET")
            .uri("/api2/json/test")
            .body(Body::empty())
            .unwrap();

        let response = middleware.handle_request(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let body_bytes = hyper::body::to_bytes(response.into_body()).await.unwrap();
        let body_str = String::from_utf8(body_bytes.to_vec()).unwrap();
        let body_json: Value = serde_json::from_str(&body_str).unwrap();

        assert_eq!(body_json["rust"], true);
    }

    #[tokio::test]
    async fn test_middleware_fallback() {
        let config = MigrationConfig {
            phase: crate::config::MigrationPhase::RustFull,
            fallback_enabled: true,
            ..Default::default()
        };

        let mut rust_handler = MockRustApiHandler::new();
        rust_handler.set_should_fail(true);

        let mut perl_client = MockPerlApiClient::new();
        perl_client.add_response(
            "GET /api2/json/test".to_string(),
            ApiResponse {
                status: 200,
                headers: HashMap::new(),
                body: json!({"perl": true}),
            },
        );

        let middleware =
            MigrationMiddleware::new(config, Arc::new(rust_handler), Arc::new(perl_client));

        let request = Request::builder()
            .method("GET")
            .uri("/api2/json/test")
            .body(Body::empty())
            .unwrap();

        let response = middleware.handle_request(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        // Check fallback header
        assert_eq!(
            response
                .headers()
                .get("X-PVE-Migration-Used-Fallback")
                .unwrap(),
            "true"
        );
    }

    #[tokio::test]
    async fn test_health_check() {
        let config = MigrationConfig::default();
        let rust_handler = Arc::new(MockRustApiHandler::new());
        let perl_client = Arc::new(MockPerlApiClient::new());

        let middleware = MigrationMiddleware::new(config, rust_handler, perl_client);

        let health = middleware.health_check().await;

        assert!(health.contains_key("perl_api"));
        assert!(health.contains_key("migration_phase"));
        assert!(health.contains_key("fallback_enabled"));
    }
}
