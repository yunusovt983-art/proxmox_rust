//! Migration integration for net-api
//!
//! Integrates the migration middleware with the existing API handlers

use crate::{
    context::AppContext,
    network::{NetworkGetQuery, NetworkListQuery},
    sdn::SDNAPI,
};
use async_trait::async_trait;
use net_migration::{
    middleware::RustApiHandler,
    perl_client::{ApiRequest, ApiResponse},
    MigrationError, Result as MigrationResult,
};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;

/// Rust API handler that delegates to existing API implementations
pub struct NetApiRustHandler {
    context: Arc<AppContext>,
}

impl NetApiRustHandler {
    pub fn new(context: Arc<AppContext>) -> Self {
        Self { context }
    }
}

#[async_trait]
impl RustApiHandler for NetApiRustHandler {
    async fn handle_request(&self, request: &ApiRequest) -> MigrationResult<ApiResponse> {
        log::debug!(
            "Handling Rust API request: {} {}",
            request.method,
            request.path
        );

        // Route to appropriate API handler based on path
        let result = match self.route_request(request).await {
            Ok(response_data) => ApiResponse {
                status: 200,
                headers: HashMap::new(),
                body: response_data,
            },
            Err(e) => {
                log::error!("Rust API handler error: {}", e);
                return Err(e);
            }
        };

        Ok(result)
    }
}

impl NetApiRustHandler {
    async fn route_request(&self, request: &ApiRequest) -> MigrationResult<Value> {
        let path = &request.path;
        let method = request.method.as_str();

        // Extract path parameters
        let path_parts: Vec<&str> = path.split('/').collect();

        match (method, path_parts.as_slice()) {
            // Network API routes
            ("GET", ["", "api2", "json", "nodes", node, "network"]) => {
                self.handle_list_interfaces(node).await
            }
            ("GET", ["", "api2", "json", "nodes", node, "network", iface]) => {
                self.handle_get_interface(node, iface).await
            }
            ("POST", ["", "api2", "json", "nodes", node, "network"]) => {
                self.handle_create_interface(node, request).await
            }
            ("PUT", ["", "api2", "json", "nodes", node, "network", iface]) => {
                self.handle_update_interface(node, iface, request).await
            }
            ("DELETE", ["", "api2", "json", "nodes", node, "network", iface]) => {
                self.handle_delete_interface(node, iface).await
            }
            ("POST", ["", "api2", "json", "nodes", node, "network", "reload"]) => {
                self.handle_reload_network(node).await
            }

            // SDN API routes
            ("GET", ["", "api2", "json", "sdn", "zones"]) => self.handle_list_zones().await,
            ("GET", ["", "api2", "json", "sdn", "zones", zone]) => self.handle_get_zone(zone).await,
            ("POST", ["", "api2", "json", "sdn", "zones"]) => {
                self.handle_create_zone(request).await
            }
            ("PUT", ["", "api2", "json", "sdn", "zones", zone]) => {
                self.handle_update_zone(zone, request).await
            }
            ("DELETE", ["", "api2", "json", "sdn", "zones", zone]) => {
                self.handle_delete_zone(zone).await
            }

            ("GET", ["", "api2", "json", "sdn", "vnets"]) => self.handle_list_vnets().await,
            ("GET", ["", "api2", "json", "sdn", "vnets", vnet]) => self.handle_get_vnet(vnet).await,
            ("POST", ["", "api2", "json", "sdn", "vnets"]) => {
                self.handle_create_vnet(request).await
            }
            ("PUT", ["", "api2", "json", "sdn", "vnets", vnet]) => {
                self.handle_update_vnet(vnet, request).await
            }
            ("DELETE", ["", "api2", "json", "sdn", "vnets", vnet]) => {
                self.handle_delete_vnet(vnet).await
            }

            // Container integration routes
            ("GET", ["", "api2", "json", "nodes", node, "lxc", vmid, "network"]) => {
                self.handle_container_network_list(node, vmid).await
            }
            ("POST", ["", "api2", "json", "nodes", node, "lxc", vmid, "network"]) => {
                self.handle_container_network_create(node, vmid, request)
                    .await
            }

            // Storage integration routes
            ("GET", ["", "api2", "json", "storage", storage, "network"]) => {
                self.handle_storage_network_config(storage).await
            }

            _ => Err(MigrationError::EndpointNotAvailable),
        }
    }

    // Network API handlers
    async fn handle_list_interfaces(&self, node: &str) -> MigrationResult<Value> {
        let default_query = NetworkListQuery {
            interface_type: None,
            enabled: None,
        };

        match self
            .context
            .network_api
            .list_interfaces(node, default_query)
            .await
        {
            Ok(interfaces) => Ok(serde_json::json!({ "data": interfaces })),
            Err(e) => Err(MigrationError::Fallback(e.to_string())),
        }
    }

    async fn handle_get_interface(&self, node: &str, iface: &str) -> MigrationResult<Value> {
        let query = NetworkGetQuery { detailed: None };
        match self
            .context
            .network_api
            .get_interface(node, iface, query)
            .await
        {
            Ok(interface) => Ok(serde_json::json!({ "data": interface })),
            Err(e) => Err(MigrationError::Fallback(e.to_string())),
        }
    }

    async fn handle_create_interface(
        &self,
        node: &str,
        request: &ApiRequest,
    ) -> MigrationResult<Value> {
        let config = request
            .body
            .as_ref()
            .ok_or_else(|| MigrationError::Fallback("Missing request body".to_string()))?;

        // Convert JSON to interface config (this would need proper deserialization)
        let interface_config = serde_json::from_value(config.clone())
            .map_err(|e| MigrationError::Fallback(format!("Invalid interface config: {}", e)))?;

        match self
            .context
            .network_api
            .create_interface(node, interface_config)
            .await
        {
            Ok(_) => Ok(serde_json::json!({ "success": true })),
            Err(e) => Err(MigrationError::Fallback(e.to_string())),
        }
    }

    async fn handle_update_interface(
        &self,
        node: &str,
        iface: &str,
        request: &ApiRequest,
    ) -> MigrationResult<Value> {
        let config = request
            .body
            .as_ref()
            .ok_or_else(|| MigrationError::Fallback("Missing request body".to_string()))?;

        let interface_config = serde_json::from_value(config.clone())
            .map_err(|e| MigrationError::Fallback(format!("Invalid interface config: {}", e)))?;

        match self
            .context
            .network_api
            .update_interface(node, iface, interface_config)
            .await
        {
            Ok(_) => Ok(serde_json::json!({ "success": true })),
            Err(e) => Err(MigrationError::Fallback(e.to_string())),
        }
    }

    async fn handle_delete_interface(&self, node: &str, iface: &str) -> MigrationResult<Value> {
        match self.context.network_api.delete_interface(node, iface).await {
            Ok(_) => Ok(serde_json::json!({ "success": true })),
            Err(e) => Err(MigrationError::Fallback(e.to_string())),
        }
    }

    async fn handle_reload_network(&self, node: &str) -> MigrationResult<Value> {
        match self.context.network_api.reload_network(node).await {
            Ok(task_id) => Ok(serde_json::json!({ "data": task_id })),
            Err(e) => Err(MigrationError::Fallback(e.to_string())),
        }
    }

    // SDN API handlers
    async fn handle_list_zones(&self) -> MigrationResult<Value> {
        let api = SDNAPI::new();
        match api.list_zones().await {
            Ok(zones) => Ok(serde_json::json!({ "data": zones })),
            Err(e) => Err(MigrationError::Fallback(e.to_string())),
        }
    }

    async fn handle_get_zone(&self, zone: &str) -> MigrationResult<Value> {
        let api = SDNAPI::new();
        match api.get_zone(zone.to_string()).await {
            Ok(zone_data) => Ok(serde_json::json!({ "data": zone_data })),
            Err(e) => Err(MigrationError::Fallback(e.to_string())),
        }
    }

    async fn handle_create_zone(&self, request: &ApiRequest) -> MigrationResult<Value> {
        let config = request
            .body
            .as_ref()
            .ok_or_else(|| MigrationError::Fallback("Missing request body".to_string()))?;

        let zone_config = serde_json::from_value(config.clone())
            .map_err(|e| MigrationError::Fallback(format!("Invalid zone config: {}", e)))?;

        let api = SDNAPI::new();
        match api.create_zone(zone_config).await {
            Ok(_) => Ok(serde_json::json!({ "success": true })),
            Err(e) => Err(MigrationError::Fallback(e.to_string())),
        }
    }

    async fn handle_update_zone(&self, zone: &str, request: &ApiRequest) -> MigrationResult<Value> {
        let config = request
            .body
            .as_ref()
            .ok_or_else(|| MigrationError::Fallback("Missing request body".to_string()))?;

        let zone_config = serde_json::from_value(config.clone())
            .map_err(|e| MigrationError::Fallback(format!("Invalid zone config: {}", e)))?;

        let api = SDNAPI::new();
        match api.update_zone(zone.to_string(), zone_config).await {
            Ok(_) => Ok(serde_json::json!({ "success": true })),
            Err(e) => Err(MigrationError::Fallback(e.to_string())),
        }
    }

    async fn handle_delete_zone(&self, zone: &str) -> MigrationResult<Value> {
        let api = SDNAPI::new();
        match api.delete_zone(zone.to_string()).await {
            Ok(_) => Ok(serde_json::json!({ "success": true })),
            Err(e) => Err(MigrationError::Fallback(e.to_string())),
        }
    }

    async fn handle_list_vnets(&self) -> MigrationResult<Value> {
        let api = SDNAPI::new();
        match api.list_vnets().await {
            Ok(vnets) => Ok(serde_json::json!({ "data": vnets })),
            Err(e) => Err(MigrationError::Fallback(e.to_string())),
        }
    }

    async fn handle_get_vnet(&self, vnet: &str) -> MigrationResult<Value> {
        let api = SDNAPI::new();
        match api.get_vnet(vnet.to_string()).await {
            Ok(vnet_data) => Ok(serde_json::json!({ "data": vnet_data })),
            Err(e) => Err(MigrationError::Fallback(e.to_string())),
        }
    }

    async fn handle_create_vnet(&self, request: &ApiRequest) -> MigrationResult<Value> {
        let config = request
            .body
            .as_ref()
            .ok_or_else(|| MigrationError::Fallback("Missing request body".to_string()))?;

        let vnet_config = serde_json::from_value(config.clone())
            .map_err(|e| MigrationError::Fallback(format!("Invalid vnet config: {}", e)))?;

        let api = SDNAPI::new();
        match api.create_vnet(vnet_config).await {
            Ok(_) => Ok(serde_json::json!({ "success": true })),
            Err(e) => Err(MigrationError::Fallback(e.to_string())),
        }
    }

    async fn handle_update_vnet(&self, vnet: &str, request: &ApiRequest) -> MigrationResult<Value> {
        let config = request
            .body
            .as_ref()
            .ok_or_else(|| MigrationError::Fallback("Missing request body".to_string()))?;

        let vnet_config = serde_json::from_value(config.clone())
            .map_err(|e| MigrationError::Fallback(format!("Invalid vnet config: {}", e)))?;

        let api = SDNAPI::new();
        match api.update_vnet(vnet.to_string(), vnet_config).await {
            Ok(_) => Ok(serde_json::json!({ "success": true })),
            Err(e) => Err(MigrationError::Fallback(e.to_string())),
        }
    }

    async fn handle_delete_vnet(&self, vnet: &str) -> MigrationResult<Value> {
        let api = SDNAPI::new();
        match api.delete_vnet(vnet.to_string()).await {
            Ok(_) => Ok(serde_json::json!({ "success": true })),
            Err(e) => Err(MigrationError::Fallback(e.to_string())),
        }
    }

    // Container integration handlers
    async fn handle_container_network_list(
        &self,
        node: &str,
        vmid: &str,
    ) -> MigrationResult<Value> {
        let vmid_num: u32 = vmid
            .parse()
            .map_err(|_| MigrationError::Fallback("Invalid VMID".to_string()))?;

        match self
            .context
            .container_api
            .list_container_networks(node.to_string(), vmid_num)
            .await
        {
            Ok(networks) => Ok(serde_json::json!({ "data": networks })),
            Err(e) => Err(MigrationError::Fallback(e.to_string())),
        }
    }

    async fn handle_container_network_create(
        &self,
        node: &str,
        vmid: &str,
        request: &ApiRequest,
    ) -> MigrationResult<Value> {
        let vmid_num: u32 = vmid
            .parse()
            .map_err(|_| MigrationError::Fallback("Invalid VMID".to_string()))?;

        let config = request
            .body
            .as_ref()
            .ok_or_else(|| MigrationError::Fallback("Missing request body".to_string()))?;

        let network_config = serde_json::from_value(config.clone())
            .map_err(|e| MigrationError::Fallback(format!("Invalid network config: {}", e)))?;

        match self
            .context
            .container_api
            .create_container_network(node.to_string(), vmid_num, network_config)
            .await
        {
            Ok(_) => Ok(serde_json::json!({ "success": true })),
            Err(e) => Err(MigrationError::Fallback(e.to_string())),
        }
    }

    // Storage integration handlers
    async fn handle_storage_network_config(&self, storage: &str) -> MigrationResult<Value> {
        match self
            .context
            .storage_api
            .get_storage_network_config(storage.to_string())
            .await
        {
            Ok(config) => Ok(serde_json::json!({ "data": config })),
            Err(e) => Err(MigrationError::Fallback(e.to_string())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use net_migration::perl_client::ApiRequest;
    use reqwest::Method;
    use serde_json::json;
    use std::collections::HashMap;

    // Mock implementations would be needed for testing
    // This is a simplified example showing the structure

    #[tokio::test]
    async fn test_route_network_list() {
        // This test would require mock implementations of the API structs
        // For now, it demonstrates the expected structure

        let request = ApiRequest {
            method: Method::GET,
            path: "/api2/json/nodes/test/network".to_string(),
            query_params: HashMap::new(),
            body: None,
            headers: HashMap::new(),
        };

        // Would test routing logic here with mocked APIs
        assert_eq!(request.path, "/api2/json/nodes/test/network");
    }
}
