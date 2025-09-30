//! Perl API client for fallback operations
//!
//! Provides HTTP client to communicate with existing Perl implementation

use async_trait::async_trait;
use reqwest::{Client, Method, Response};
use serde_json::Value;
use std::collections::HashMap;
use std::time::Duration;
use thiserror::Error;

/// Perl API client errors
#[derive(Debug, Error)]
pub enum PerlApiError {
    #[error("HTTP request failed: {0}")]
    Http(#[from] reqwest::Error),

    #[error("Invalid URL: {0}")]
    InvalidUrl(String),

    #[error("Authentication failed")]
    Authentication,

    #[error("Perl API returned error: {status} - {message}")]
    ApiError { status: u16, message: String },

    #[error("Response parsing failed: {0}")]
    ParseError(String),

    #[error("Timeout waiting for Perl API response")]
    Timeout,
}

/// HTTP request representation
#[derive(Debug, Clone)]
pub struct ApiRequest {
    pub method: Method,
    pub path: String,
    pub query_params: HashMap<String, String>,
    pub body: Option<Value>,
    pub headers: HashMap<String, String>,
}

/// HTTP response representation
#[derive(Debug, Clone)]
pub struct ApiResponse {
    pub status: u16,
    pub headers: HashMap<String, String>,
    pub body: Value,
}

/// Trait for Perl API communication
#[async_trait]
pub trait PerlApiClient: Send + Sync {
    async fn call(&self, request: &ApiRequest) -> Result<ApiResponse, PerlApiError>;
    async fn health_check(&self) -> Result<bool, PerlApiError>;
}

/// HTTP-based Perl API client implementation
pub struct HttpPerlApiClient {
    client: Client,
    base_url: String,
    timeout: Duration,
    auth_token: Option<String>,
}

impl HttpPerlApiClient {
    pub fn new(base_url: String, timeout: Duration) -> Self {
        let client = Client::builder()
            .timeout(timeout)
            .build()
            .expect("Failed to create HTTP client");

        Self {
            client,
            base_url,
            timeout,
            auth_token: None,
        }
    }

    pub fn with_auth_token(mut self, token: String) -> Self {
        self.auth_token = Some(token);
        self
    }

    fn build_url(
        &self,
        path: &str,
        query_params: &HashMap<String, String>,
    ) -> Result<String, PerlApiError> {
        let mut url = format!("{}{}", self.base_url, path);

        if !query_params.is_empty() {
            url.push('?');
            let query_string: Vec<String> = query_params
                .iter()
                .map(|(k, v)| format!("{}={}", urlencoding::encode(k), urlencoding::encode(v)))
                .collect();
            url.push_str(&query_string.join("&"));
        }

        Ok(url)
    }

    async fn execute_request(&self, request: &ApiRequest) -> Result<Response, PerlApiError> {
        let url = self.build_url(&request.path, &request.query_params)?;

        let mut req_builder = self.client.request(request.method.clone(), &url);

        // Add authentication if available
        if let Some(ref token) = self.auth_token {
            req_builder = req_builder.header("Authorization", format!("Bearer {}", token));
        }

        // Add custom headers
        for (key, value) in &request.headers {
            req_builder = req_builder.header(key, value);
        }

        // Add body for non-GET requests
        if let Some(ref body) = request.body {
            req_builder = req_builder.json(body);
        }

        let response = req_builder.send().await?;
        Ok(response)
    }
}

#[async_trait]
impl PerlApiClient for HttpPerlApiClient {
    async fn call(&self, request: &ApiRequest) -> Result<ApiResponse, PerlApiError> {
        log::debug!("Calling Perl API: {} {}", request.method, request.path);

        let response = self.execute_request(request).await?;
        let status = response.status().as_u16();

        // Extract headers
        let mut headers = HashMap::new();
        for (key, value) in response.headers() {
            if let Ok(value_str) = value.to_str() {
                headers.insert(key.to_string(), value_str.to_string());
            }
        }

        // Parse response body
        let body_text = response.text().await?;
        let body: Value = if body_text.is_empty() {
            Value::Null
        } else {
            serde_json::from_str(&body_text)
                .map_err(|e| PerlApiError::ParseError(format!("JSON parse error: {}", e)))?
        };

        // Check for API errors
        if status >= 400 {
            let message = body
                .get("message")
                .and_then(|m| m.as_str())
                .unwrap_or("Unknown error")
                .to_string();

            return Err(PerlApiError::ApiError { status, message });
        }

        log::debug!(
            "Perl API response: status={}, body_size={}",
            status,
            body_text.len()
        );

        Ok(ApiResponse {
            status,
            headers,
            body,
        })
    }

    async fn health_check(&self) -> Result<bool, PerlApiError> {
        let request = ApiRequest {
            method: Method::GET,
            path: "/api2/json/version".to_string(),
            query_params: HashMap::new(),
            body: None,
            headers: HashMap::new(),
        };

        match self.call(&request).await {
            Ok(response) => Ok(response.status == 200),
            Err(PerlApiError::Http(_)) => Ok(false), // Network issues = unhealthy
            Err(_) => Ok(false),                     // Other errors = unhealthy
        }
    }
}

/// Mock Perl API client for testing
pub struct MockPerlApiClient {
    responses: HashMap<String, ApiResponse>,
    health_status: bool,
}

impl MockPerlApiClient {
    pub fn new() -> Self {
        Self {
            responses: HashMap::new(),
            health_status: true,
        }
    }

    pub fn add_response(&mut self, method_path: String, response: ApiResponse) {
        self.responses.insert(method_path, response);
    }

    pub fn set_health_status(&mut self, healthy: bool) {
        self.health_status = healthy;
    }

    fn make_key(&self, request: &ApiRequest) -> String {
        format!("{} {}", request.method, request.path)
    }
}

impl Default for MockPerlApiClient {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl PerlApiClient for MockPerlApiClient {
    async fn call(&self, request: &ApiRequest) -> Result<ApiResponse, PerlApiError> {
        let key = self.make_key(request);

        if let Some(response) = self.responses.get(&key) {
            Ok(response.clone())
        } else {
            Err(PerlApiError::ApiError {
                status: 404,
                message: format!("Mock response not found for: {}", key),
            })
        }
    }

    async fn health_check(&self) -> Result<bool, PerlApiError> {
        Ok(self.health_status)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use reqwest::Method;
    use serde_json::json;

    #[tokio::test]
    async fn test_mock_client() {
        let mut client = MockPerlApiClient::new();

        let response = ApiResponse {
            status: 200,
            headers: HashMap::new(),
            body: json!({"data": "test"}),
        };

        client.add_response("GET /api2/json/test".to_string(), response);

        let request = ApiRequest {
            method: Method::GET,
            path: "/api2/json/test".to_string(),
            query_params: HashMap::new(),
            body: None,
            headers: HashMap::new(),
        };

        let result = client.call(&request).await.unwrap();
        assert_eq!(result.status, 200);
        assert_eq!(result.body["data"], "test");
    }

    #[tokio::test]
    async fn test_health_check() {
        let client = MockPerlApiClient::new();
        assert!(client.health_check().await.unwrap());

        let mut unhealthy_client = MockPerlApiClient::new();
        unhealthy_client.set_health_status(false);
        assert!(!unhealthy_client.health_check().await.unwrap());
    }

    #[test]
    fn test_url_building() {
        let client =
            HttpPerlApiClient::new("http://localhost:8006".to_string(), Duration::from_secs(30));

        let mut params = HashMap::new();
        params.insert("node".to_string(), "test-node".to_string());
        params.insert("type".to_string(), "bridge".to_string());

        let url = client
            .build_url("/api2/json/nodes/{node}/network", &params)
            .unwrap();
        assert!(url.contains("node=test-node"));
        assert!(url.contains("type=bridge"));
    }
}
