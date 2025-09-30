//! Contract tests for comparing Rust API with Perl API

use serde_json::Value;
use std::collections::HashMap;
use std::process::Command;
use tokio::time::{timeout, Duration};

use pve_network_api::NetworkAPI;

/// Contract test runner for comparing Perl and Rust API responses
pub struct ContractTester {
    rust_api: NetworkAPI,
    perl_base_url: String,
    test_node: String,
}

/// Test result for a single API endpoint
#[derive(Debug)]
pub struct ContractTestResult {
    pub endpoint: String,
    pub method: String,
    pub passed: bool,
    pub differences: Vec<String>,
    pub perl_response: Option<Value>,
    pub rust_response: Option<Value>,
    pub error: Option<String>,
}

/// Test suite results
#[derive(Debug)]
pub struct ContractTestSuite {
    pub results: Vec<ContractTestResult>,
    pub total_tests: usize,
    pub passed_tests: usize,
    pub failed_tests: usize,
}

impl ContractTester {
    /// Create new contract tester
    pub fn new(test_node: &str) -> Self {
        Self {
            rust_api: NetworkAPI::new(),
            perl_base_url: "http://localhost:8006".to_string(),
            test_node: test_node.to_string(),
        }
    }

    /// Create contract tester with custom Perl API URL
    pub fn with_perl_url(test_node: &str, perl_url: &str) -> Self {
        Self {
            rust_api: NetworkAPI::new(),
            perl_base_url: perl_url.to_string(),
            test_node: test_node.to_string(),
        }
    }

    /// Run all contract tests
    pub async fn run_all_tests(&self) -> ContractTestSuite {
        let mut results = Vec::new();

        // Test network interface listing
        results.push(self.test_list_interfaces().await);
        results.push(self.test_list_interfaces_with_type_filter().await);
        results.push(self.test_list_interfaces_with_enabled_filter().await);

        // Test specific interface retrieval
        let interfaces = self.get_test_interfaces().await;
        for interface_name in interfaces {
            results.push(self.test_get_interface(&interface_name).await);
            results.push(self.test_get_interface_detailed(&interface_name).await);
            results.push(self.test_get_interface_status(&interface_name).await);
        }

        let passed_tests = results.iter().filter(|r| r.passed).count();
        let failed_tests = results.len() - passed_tests;

        ContractTestSuite {
            total_tests: results.len(),
            passed_tests,
            failed_tests,
            results,
        }
    }

    /// Test network interface listing endpoint
    async fn test_list_interfaces(&self) -> ContractTestResult {
        let endpoint = format!("/api2/json/nodes/{}/network", self.test_node);

        let (perl_response, rust_response) = tokio::join!(
            self.call_perl_api(&endpoint, "GET", None),
            self.call_rust_api_list_interfaces(None, None)
        );

        self.compare_responses(
            "list_interfaces",
            "GET",
            &endpoint,
            perl_response,
            rust_response,
        )
    }

    /// Test network interface listing with type filter
    async fn test_list_interfaces_with_type_filter(&self) -> ContractTestResult {
        let endpoint = format!("/api2/json/nodes/{}/network?type=bridge", self.test_node);

        let (perl_response, rust_response) = tokio::join!(
            self.call_perl_api(&endpoint, "GET", None),
            self.call_rust_api_list_interfaces(Some("bridge".to_string()), None)
        );

        self.compare_responses(
            "list_interfaces_type_filter",
            "GET",
            &endpoint,
            perl_response,
            rust_response,
        )
    }

    /// Test network interface listing with enabled filter
    async fn test_list_interfaces_with_enabled_filter(&self) -> ContractTestResult {
        let endpoint = format!("/api2/json/nodes/{}/network?enabled=1", self.test_node);

        let (perl_response, rust_response) = tokio::join!(
            self.call_perl_api(&endpoint, "GET", None),
            self.call_rust_api_list_interfaces(None, Some(true))
        );

        self.compare_responses(
            "list_interfaces_enabled_filter",
            "GET",
            &endpoint,
            perl_response,
            rust_response,
        )
    }

    /// Test getting specific interface
    async fn test_get_interface(&self, interface_name: &str) -> ContractTestResult {
        let endpoint = format!(
            "/api2/json/nodes/{}/network/{}",
            self.test_node, interface_name
        );

        let (perl_response, rust_response) = tokio::join!(
            self.call_perl_api(&endpoint, "GET", None),
            self.call_rust_api_get_interface(interface_name, false)
        );

        self.compare_responses(
            &format!("get_interface_{}", interface_name),
            "GET",
            &endpoint,
            perl_response,
            rust_response,
        )
    }

    /// Test getting detailed interface information
    async fn test_get_interface_detailed(&self, interface_name: &str) -> ContractTestResult {
        let endpoint = format!(
            "/api2/json/nodes/{}/network/{}?detailed=1",
            self.test_node, interface_name
        );

        let (perl_response, rust_response) = tokio::join!(
            self.call_perl_api(&endpoint, "GET", None),
            self.call_rust_api_get_interface(interface_name, true)
        );

        self.compare_responses(
            &format!("get_interface_detailed_{}", interface_name),
            "GET",
            &endpoint,
            perl_response,
            rust_response,
        )
    }

    /// Test getting interface status
    async fn test_get_interface_status(&self, interface_name: &str) -> ContractTestResult {
        let endpoint = format!(
            "/api2/json/nodes/{}/network/{}/status",
            self.test_node, interface_name
        );

        let (perl_response, rust_response) = tokio::join!(
            self.call_perl_api(&endpoint, "GET", None),
            self.call_rust_api_get_interface_status(interface_name)
        );

        self.compare_responses(
            &format!("get_interface_status_{}", interface_name),
            "GET",
            &endpoint,
            perl_response,
            rust_response,
        )
    }

    /// Call Perl API endpoint
    async fn call_perl_api(
        &self,
        endpoint: &str,
        method: &str,
        _body: Option<&str>,
    ) -> Result<Value, String> {
        // In a real implementation, this would make HTTP requests to the Perl API
        // For now, we'll simulate by calling pvesh command or reading from test data

        let result = timeout(Duration::from_secs(10), async {
            self.call_pvesh_command(endpoint, method).await
        })
        .await;

        match result {
            Ok(response) => response,
            Err(_) => Err("Timeout calling Perl API".to_string()),
        }
    }

    /// Call pvesh command to get Perl API response
    async fn call_pvesh_command(&self, endpoint: &str, method: &str) -> Result<Value, String> {
        let output = Command::new("pvesh")
            .arg("get")
            .arg(endpoint)
            .arg("--output-format")
            .arg("json")
            .output();

        match output {
            Ok(output) => {
                if output.status.success() {
                    let stdout = String::from_utf8_lossy(&output.stdout);
                    serde_json::from_str(&stdout)
                        .map_err(|e| format!("Failed to parse JSON: {}", e))
                } else {
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    Err(format!("pvesh command failed: {}", stderr))
                }
            }
            Err(e) => {
                // If pvesh is not available, return mock data for testing
                log::warn!("pvesh command not available: {}, using mock data", e);
                self.get_mock_perl_response(endpoint, method)
            }
        }
    }

    /// Get mock Perl API response for testing when pvesh is not available
    fn get_mock_perl_response(&self, endpoint: &str, _method: &str) -> Result<Value, String> {
        if endpoint.contains("/network") && !endpoint.contains("/status") {
            if endpoint.contains("?") || endpoint.ends_with("/network") {
                // List interfaces response
                Ok(serde_json::json!({
                    "data": [
                        {
                            "iface": "lo",
                            "type": "loopback",
                            "method": "loopback",
                            "active": 1,
                            "autostart": 1
                        },
                        {
                            "iface": "eth0",
                            "type": "eth",
                            "method": "dhcp",
                            "active": 1,
                            "autostart": 1
                        },
                        {
                            "iface": "vmbr0",
                            "type": "bridge",
                            "method": "static",
                            "address": "192.168.1.1",
                            "netmask": "255.255.255.0",
                            "bridge_ports": "eth1",
                            "bridge_vlan_aware": 1,
                            "active": 1,
                            "autostart": 1
                        }
                    ]
                }))
            } else {
                // Single interface response
                let interface_name = endpoint.split('/').last().unwrap_or("unknown");
                Ok(serde_json::json!({
                    "data": {
                        "iface": interface_name,
                        "type": "eth",
                        "method": "dhcp",
                        "active": 1,
                        "autostart": 1
                    }
                }))
            }
        } else if endpoint.contains("/status") {
            // Interface status response
            let interface_name = endpoint.split('/').nth_back(1).unwrap_or("unknown");
            Ok(serde_json::json!({
                "data": {
                    "iface": interface_name,
                    "active": 1,
                    "link": true,
                    "speed": 1000,
                    "duplex": "full"
                }
            }))
        } else {
            Err("Unknown endpoint".to_string())
        }
    }

    /// Call Rust API for listing interfaces
    async fn call_rust_api_list_interfaces(
        &self,
        type_filter: Option<String>,
        enabled_filter: Option<bool>,
    ) -> Result<Value, String> {
        use pve_network_api::network::NetworkListQuery;

        let query = NetworkListQuery {
            interface_type: type_filter,
            enabled: enabled_filter,
        };

        match self.rust_api.list_interfaces(&self.test_node, query).await {
            Ok(interfaces) => {
                let data = serde_json::json!({ "data": interfaces });
                Ok(data)
            }
            Err(e) => Err(format!("Rust API error: {}", e)),
        }
    }

    /// Call Rust API for getting specific interface
    async fn call_rust_api_get_interface(
        &self,
        interface_name: &str,
        detailed: bool,
    ) -> Result<Value, String> {
        use pve_network_api::network::NetworkGetQuery;

        let query = NetworkGetQuery {
            detailed: Some(detailed),
        };

        match self
            .rust_api
            .get_interface(&self.test_node, interface_name, query)
            .await
        {
            Ok(interface) => {
                let data = serde_json::json!({ "data": interface });
                Ok(data)
            }
            Err(e) => Err(format!("Rust API error: {}", e)),
        }
    }

    /// Call Rust API for getting interface status
    async fn call_rust_api_get_interface_status(
        &self,
        interface_name: &str,
    ) -> Result<Value, String> {
        match self
            .rust_api
            .get_interface_status(&self.test_node, interface_name)
            .await
        {
            Ok(status) => {
                let data = serde_json::json!({ "data": status });
                Ok(data)
            }
            Err(e) => Err(format!("Rust API error: {}", e)),
        }
    }

    /// Compare Perl and Rust API responses
    fn compare_responses(
        &self,
        _test_name: &str,
        method: &str,
        endpoint: &str,
        perl_result: Result<Value, String>,
        rust_result: Result<Value, String>,
    ) -> ContractTestResult {
        match (perl_result, rust_result) {
            (Ok(perl_response), Ok(rust_response)) => {
                let differences = self.find_differences(&perl_response, &rust_response);
                let passed = differences.is_empty();

                ContractTestResult {
                    endpoint: endpoint.to_string(),
                    method: method.to_string(),
                    passed,
                    differences,
                    perl_response: Some(perl_response),
                    rust_response: Some(rust_response),
                    error: None,
                }
            }
            (Err(perl_error), Ok(rust_response)) => ContractTestResult {
                endpoint: endpoint.to_string(),
                method: method.to_string(),
                passed: false,
                differences: vec![format!("Perl API failed: {}", perl_error)],
                perl_response: None,
                rust_response: Some(rust_response),
                error: Some(perl_error),
            },
            (Ok(perl_response), Err(rust_error)) => ContractTestResult {
                endpoint: endpoint.to_string(),
                method: method.to_string(),
                passed: false,
                differences: vec![format!("Rust API failed: {}", rust_error)],
                perl_response: Some(perl_response),
                rust_response: None,
                error: Some(rust_error),
            },
            (Err(perl_error), Err(rust_error)) => ContractTestResult {
                endpoint: endpoint.to_string(),
                method: method.to_string(),
                passed: false,
                differences: vec![
                    format!("Perl API failed: {}", perl_error),
                    format!("Rust API failed: {}", rust_error),
                ],
                perl_response: None,
                rust_response: None,
                error: Some(format!(
                    "Both APIs failed - Perl: {}, Rust: {}",
                    perl_error, rust_error
                )),
            },
        }
    }

    /// Find differences between two JSON values
    fn find_differences(&self, perl_response: &Value, rust_response: &Value) -> Vec<String> {
        let mut differences = Vec::new();
        self.compare_json_values("", perl_response, rust_response, &mut differences);
        differences
    }

    /// Recursively compare JSON values
    fn compare_json_values(
        &self,
        path: &str,
        perl_value: &Value,
        rust_value: &Value,
        differences: &mut Vec<String>,
    ) {
        match (perl_value, rust_value) {
            (Value::Object(perl_obj), Value::Object(rust_obj)) => {
                // Check for missing keys in Rust response
                for key in perl_obj.keys() {
                    if !rust_obj.contains_key(key) {
                        differences.push(format!("{}.{}: missing in Rust response", path, key));
                    }
                }

                // Check for extra keys in Rust response
                for key in rust_obj.keys() {
                    if !perl_obj.contains_key(key) {
                        differences.push(format!("{}.{}: extra in Rust response", path, key));
                    }
                }

                // Compare common keys
                for key in perl_obj.keys() {
                    if let Some(rust_val) = rust_obj.get(key) {
                        let new_path = if path.is_empty() {
                            key.clone()
                        } else {
                            format!("{}.{}", path, key)
                        };
                        self.compare_json_values(&new_path, &perl_obj[key], rust_val, differences);
                    }
                }
            }
            (Value::Array(perl_arr), Value::Array(rust_arr)) => {
                if perl_arr.len() != rust_arr.len() {
                    differences.push(format!(
                        "{}: array length mismatch (Perl: {}, Rust: {})",
                        path,
                        perl_arr.len(),
                        rust_arr.len()
                    ));
                }

                let min_len = perl_arr.len().min(rust_arr.len());
                for i in 0..min_len {
                    let new_path = format!("{}[{}]", path, i);
                    self.compare_json_values(&new_path, &perl_arr[i], &rust_arr[i], differences);
                }
            }
            (perl_val, rust_val) => {
                if perl_val != rust_val {
                    differences.push(format!(
                        "{}: value mismatch (Perl: {}, Rust: {})",
                        path, perl_val, rust_val
                    ));
                }
            }
        }
    }

    /// Get list of test interfaces
    async fn get_test_interfaces(&self) -> Vec<String> {
        // Return common interface names for testing
        vec!["lo".to_string(), "eth0".to_string(), "vmbr0".to_string()]
    }
}

impl ContractTestSuite {
    /// Print test results summary
    pub fn print_summary(&self) {
        println!("Contract Test Results:");
        println!("======================");
        println!("Total tests: {}", self.total_tests);
        println!("Passed: {}", self.passed_tests);
        println!("Failed: {}", self.failed_tests);
        println!(
            "Success rate: {:.1}%",
            (self.passed_tests as f64 / self.total_tests as f64) * 100.0
        );
        println!();

        if self.failed_tests > 0 {
            println!("Failed tests:");
            for result in &self.results {
                if !result.passed {
                    println!(
                        "  {} {} - {}",
                        result.method,
                        result.endpoint,
                        result.error.as_deref().unwrap_or("Differences found")
                    );
                    for diff in &result.differences {
                        println!("    - {}", diff);
                    }
                    println!();
                }
            }
        }
    }

    /// Generate detailed test report
    pub fn generate_report(&self) -> String {
        let mut report = String::new();

        report.push_str("# Contract Test Report\n\n");
        report.push_str(&format!("**Total tests:** {}\n", self.total_tests));
        report.push_str(&format!("**Passed:** {}\n", self.passed_tests));
        report.push_str(&format!("**Failed:** {}\n", self.failed_tests));
        report.push_str(&format!(
            "**Success rate:** {:.1}%\n\n",
            (self.passed_tests as f64 / self.total_tests as f64) * 100.0
        ));

        if self.failed_tests > 0 {
            report.push_str("## Failed Tests\n\n");
            for result in &self.results {
                if !result.passed {
                    report.push_str(&format!("### {} {}\n\n", result.method, result.endpoint));

                    if let Some(error) = &result.error {
                        report.push_str(&format!("**Error:** {}\n\n", error));
                    }

                    if !result.differences.is_empty() {
                        report.push_str("**Differences:**\n");
                        for diff in &result.differences {
                            report.push_str(&format!("- {}\n", diff));
                        }
                        report.push_str("\n");
                    }
                }
            }
        }

        report.push_str("## All Test Results\n\n");
        for result in &self.results {
            let status = if result.passed {
                "✅ PASS"
            } else {
                "❌ FAIL"
            };
            report.push_str(&format!(
                "- {} {} {} - {}\n",
                status,
                result.method,
                result.endpoint,
                result.error.as_deref().unwrap_or("OK")
            ));
        }

        report
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_contract_tester_creation() {
        let tester = ContractTester::new("test-node");
        assert_eq!(tester.test_node, "test-node");
    }

    #[tokio::test]
    async fn test_mock_perl_response() {
        let tester = ContractTester::new("test-node");

        let response = tester.get_mock_perl_response("/api2/json/nodes/test-node/network", "GET");
        assert!(response.is_ok());

        let json = response.unwrap();
        assert!(json.get("data").is_some());
    }

    #[tokio::test]
    async fn test_json_comparison() {
        let tester = ContractTester::new("test-node");

        let json1 = serde_json::json!({
            "data": {
                "iface": "eth0",
                "type": "eth",
                "active": 1
            }
        });

        let json2 = serde_json::json!({
            "data": {
                "iface": "eth0",
                "type": "eth",
                "active": 1
            }
        });

        let differences = tester.find_differences(&json1, &json2);
        assert!(differences.is_empty());

        let json3 = serde_json::json!({
            "data": {
                "iface": "eth0",
                "type": "bridge",
                "active": 0
            }
        });

        let differences = tester.find_differences(&json1, &json3);
        assert!(!differences.is_empty());
    }

    #[tokio::test]
    async fn test_run_single_contract_test() {
        let tester = ContractTester::new("test-node");
        let result = tester.test_list_interfaces().await;

        // The test might fail due to missing Perl API, but it should not panic
        assert!(!result.endpoint.is_empty());
        assert!(!result.method.is_empty());
    }
}
