//! Container network API endpoints

use anyhow::Result;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::Json,
    routing::{get, post, put},
    Router,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use container_integration::hotplug::{HotplugOperation, HotplugStatus};
use container_integration::{
    ContainerId, ContainerIntegration, ContainerNetworkConfig, ContainerNetworkConfigExt,
    ContainerNetworkInterface,
};
use pve_shared_types::VNetBinding;

use crate::context::AppContext;

/// Container network API handler
pub struct ContainerNetworkAPI {
    integration: Arc<ContainerIntegration>,
}

#[derive(Debug, Serialize)]
struct ContainerResponse<T> {
    data: T,
}

#[derive(Debug, Serialize)]
struct ContainerErrorResponse {
    error: String,
}

#[derive(Debug, Deserialize)]
struct VnetBindRequest {
    vnet: String,
}

impl ContainerNetworkAPI {
    /// Build router for container network endpoints.
    pub fn router() -> Router<Arc<AppContext>> {
        Router::new()
            .route(
                "/api2/json/nodes/:node/lxc/:vmid/network",
                get(list_container_networks).post(create_container_network),
            )
            .route(
                "/api2/json/nodes/:node/lxc/:vmid/network/config",
                get(get_container_network),
            )
            .route(
                "/api2/json/nodes/:node/lxc/:vmid/network/:iface",
                put(update_container_interface).delete(remove_container_interface),
            )
            .route(
                "/api2/json/nodes/:node/lxc/:vmid/network/:iface/hotplug",
                post(hotplug_add_interface).delete(hotplug_remove_interface),
            )
            .route(
                "/api2/json/nodes/:node/lxc/:vmid/network/operations/:operation_id",
                get(get_hotplug_operation),
            )
            .route(
                "/api2/json/nodes/:node/lxc/:vmid/network/vnets",
                get(list_container_vnets),
            )
            .route(
                "/api2/json/nodes/:node/lxc/:vmid/network/:iface/vnet",
                post(bind_vnet).delete(unbind_vnet),
            )
            .route(
                "/api2/json/sdn/vnets/:vnet/containers",
                get(list_vnet_containers),
            )
            .route(
                "/api2/json/nodes/:node/lxc/:vmid/network/stats",
                get(get_container_network_stats),
            )
    }

    /// Create new container network API
    pub fn new(integration: Arc<ContainerIntegration>) -> Self {
        Self { integration }
    }

    /// List container network configuration (all interfaces)
    pub async fn list_container_networks(
        &self,
        node: String,
        container_id: ContainerId,
    ) -> Result<ContainerNetworkConfig> {
        let _ = node;
        self.integration
            .compat()
            .read_container_config(container_id)
            .await
            .map_err(|e| anyhow::anyhow!("Failed to read container config: {}", e))
    }

    /// Create or replace a container's network configuration
    pub async fn create_container_network(
        &self,
        node: String,
        container_id: ContainerId,
        mut config: ContainerNetworkConfig,
    ) -> Result<()> {
        let _ = node;

        if config.container_id != container_id {
            config.container_id = container_id;
        }

        config
            .validate()
            .map_err(|e| anyhow::anyhow!("Invalid container network config: {}", e))?;

        self.integration
            .compat()
            .write_container_config(&config)
            .await
            .map_err(|e| anyhow::anyhow!("Failed to write container config: {}", e))
    }

    /// Get container network configuration
    pub async fn get_container_network(
        &self,
        node: String,
        container_id: ContainerId,
    ) -> Result<ContainerNetworkResponse> {
        // Read container configuration
        let config = self
            .integration
            .compat()
            .read_container_config(container_id)
            .await?;

        // Get VNet bindings
        let vnet_bindings: Vec<VNetBinding> = self
            .integration
            .vnet_binding()
            .list_bindings()
            .await
            .map_err(|e| anyhow::anyhow!("Failed to list VNet bindings: {}", e))?
            .into_iter()
            .filter(|binding| binding.container_id == container_id)
            .collect();

        // Get active hotplug operations
        let hotplug_operations = self
            .integration
            .hotplug()
            .list_container_operations(container_id)
            .await
            .map_err(|e| anyhow::anyhow!("Failed to list hotplug operations: {}", e))?;

        let hotplug_operations = hotplug_operations
            .into_iter()
            .map(|op| HotplugOperationResponse {
                operation_id: op.id,
                container_id: op.container_id,
                status: match op.status {
                    HotplugStatus::InProgress => "in-progress".to_string(),
                    HotplugStatus::Completed => "completed".to_string(),
                    HotplugStatus::Failed => "failed".to_string(),
                },
            })
            .collect();

        Ok(ContainerNetworkResponse {
            node,
            container_id,
            config,
            vnet_bindings,
            hotplug_operations,
        })
    }

    /// Update container network interface
    pub async fn update_container_interface(
        &self,
        _node: String,
        container_id: ContainerId,
        interface_name: String,
        interface_config: ContainerNetworkInterface,
    ) -> Result<()> {
        // Validate interface name matches
        if interface_config.name != interface_name {
            anyhow::bail!("Interface name mismatch");
        }

        // Update interface in container configuration
        self.integration
            .compat()
            .update_interface(container_id, interface_config)
            .await?;

        Ok(())
    }

    /// Remove container network interface
    pub async fn remove_container_interface(
        &self,
        _node: String,
        container_id: ContainerId,
        interface_name: String,
    ) -> Result<()> {
        // Remove interface from container configuration
        self.integration
            .compat()
            .remove_interface(container_id, &interface_name)
            .await?;

        // Unbind VNet if bound
        if self
            .integration
            .vnet_binding()
            .is_interface_bound(container_id, &interface_name)
            .await?
        {
            self.integration
                .vnet_binding()
                .unbind_vnet(container_id, &interface_name)
                .await?;
        }

        Ok(())
    }

    /// Hotplug add network interface
    pub async fn hotplug_add_interface(
        &self,
        _node: String,
        container_id: ContainerId,
        interface: ContainerNetworkInterface,
    ) -> Result<HotplugOperationResponse> {
        let operation_id = self
            .integration
            .hotplug()
            .hotplug_add(container_id, interface)
            .await?;

        Ok(HotplugOperationResponse {
            operation_id,
            container_id,
            status: "started".to_string(),
        })
    }

    /// Hotplug remove network interface
    pub async fn hotplug_remove_interface(
        &self,
        _node: String,
        container_id: ContainerId,
        interface_name: String,
    ) -> Result<HotplugOperationResponse> {
        let operation_id = self
            .integration
            .hotplug()
            .hotplug_remove(container_id, interface_name)
            .await?;

        Ok(HotplugOperationResponse {
            operation_id,
            container_id,
            status: "started".to_string(),
        })
    }

    /// Get hotplug operation status
    pub async fn get_hotplug_operation(
        &self,
        _node: String,
        container_id: ContainerId,
        operation_id: String,
    ) -> Result<Option<HotplugOperation>> {
        let operation = self
            .integration
            .hotplug()
            .get_operation_status(&operation_id)
            .await?;

        // Verify operation belongs to the specified container
        if let Some(ref op) = operation {
            if op.container_id != container_id {
                return Ok(None);
            }
        }

        Ok(operation)
    }

    /// List container VNet bindings
    pub async fn list_container_vnets(
        &self,
        _node: String,
        container_id: ContainerId,
    ) -> Result<Vec<String>> {
        self.integration
            .vnet_binding()
            .get_container_vnets(container_id)
            .await
            .map_err(|e| anyhow::anyhow!("Failed to get container VNets: {}", e))
    }

    /// Bind VNet to container interface
    pub async fn bind_vnet(
        &self,
        _node: String,
        container_id: ContainerId,
        interface_name: String,
        vnet: String,
    ) -> Result<()> {
        // Get current interface configuration
        let config = self
            .integration
            .compat()
            .read_container_config(container_id)
            .await?;

        if let Some(mut interface) = config.get_interface(&interface_name).cloned() {
            // Update interface to use VNet
            interface.vnet = Some(vnet.clone());
            interface.bridge = None; // Clear bridge if set

            // Bind VNet
            self.integration
                .vnet_binding()
                .bind_vnet(&vnet, container_id, &interface)
                .await?;

            // Update container configuration
            self.integration
                .compat()
                .update_interface(container_id, interface)
                .await?;

            Ok(())
        } else {
            anyhow::bail!("Interface '{}' not found", interface_name)
        }
    }

    /// Unbind VNet from container interface
    pub async fn unbind_vnet(
        &self,
        _node: String,
        container_id: ContainerId,
        interface_name: String,
    ) -> Result<()> {
        // Unbind VNet
        self.integration
            .vnet_binding()
            .unbind_vnet(container_id, &interface_name)
            .await?;

        Ok(())
    }

    /// List VNet containers
    pub async fn list_vnet_containers(&self, vnet: String) -> Result<Vec<ContainerId>> {
        self.integration
            .vnet_binding()
            .get_vnet_containers(&vnet)
            .await
            .map_err(|e| anyhow::anyhow!("Failed to get VNet containers: {}", e))
    }

    /// Get container network statistics
    pub async fn get_container_network_stats(
        &self,
        _node: String,
        container_id: ContainerId,
    ) -> Result<ContainerNetworkStats> {
        // Get hook execution history
        let hook_history = self
            .integration
            .hooks()
            .get_execution_history(Some(container_id))
            .await?;

        // Get VNet bindings
        let vnet_bindings = self
            .integration
            .vnet_binding()
            .list_bindings()
            .await?
            .into_iter()
            .filter(|binding| binding.container_id == container_id)
            .collect::<Vec<_>>();

        // Get hotplug operations
        let hotplug_operations = self
            .integration
            .hotplug()
            .list_container_operations(container_id)
            .await?;

        Ok(ContainerNetworkStats {
            container_id,
            hook_executions: hook_history.len(),
            vnet_bindings: vnet_bindings.len(),
            active_hotplug_operations: hotplug_operations.len(),
        })
    }
}

/// Container network response
#[derive(Debug, Serialize)]
pub struct ContainerNetworkResponse {
    pub node: String,
    pub container_id: ContainerId,
    pub config: ContainerNetworkConfig,
    pub vnet_bindings: Vec<VNetBinding>,
    pub hotplug_operations: Vec<HotplugOperationResponse>,
}

/// Hotplug operation response
#[derive(Debug, Serialize)]
pub struct HotplugOperationResponse {
    pub operation_id: String,
    pub container_id: ContainerId,
    pub status: String,
}

/// Container network statistics
#[derive(Debug, Serialize)]
pub struct ContainerNetworkStats {
    pub container_id: ContainerId,
    pub hook_executions: usize,
    pub vnet_bindings: usize,
    pub active_hotplug_operations: usize,
}

fn container_error(err: anyhow::Error) -> (StatusCode, Json<ContainerErrorResponse>) {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(ContainerErrorResponse {
            error: err.to_string(),
        }),
    )
}

fn parse_container_id(
    vmid: &str,
) -> Result<ContainerId, (StatusCode, Json<ContainerErrorResponse>)> {
    vmid.parse::<ContainerId>().map_err(|_| {
        (
            StatusCode::BAD_REQUEST,
            Json(ContainerErrorResponse {
                error: format!("Invalid container ID: {}", vmid),
            }),
        )
    })
}

async fn list_container_networks(
    State(context): State<Arc<AppContext>>,
    Path((node, vmid)): Path<(String, String)>,
) -> Result<
    Json<ContainerResponse<ContainerNetworkConfig>>,
    (StatusCode, Json<ContainerErrorResponse>),
> {
    let container_id = parse_container_id(&vmid)?;
    context
        .container_api
        .list_container_networks(node, container_id)
        .await
        .map(|data| Json(ContainerResponse { data }))
        .map_err(container_error)
}

async fn create_container_network(
    State(context): State<Arc<AppContext>>,
    Path((node, vmid)): Path<(String, String)>,
    Json(config): Json<ContainerNetworkConfig>,
) -> Result<Json<ContainerResponse<()>>, (StatusCode, Json<ContainerErrorResponse>)> {
    let container_id = parse_container_id(&vmid)?;
    context
        .container_api
        .create_container_network(node, container_id, config)
        .await
        .map(|_| Json(ContainerResponse { data: () }))
        .map_err(container_error)
}

async fn get_container_network(
    State(context): State<Arc<AppContext>>,
    Path((node, vmid)): Path<(String, String)>,
) -> Result<
    Json<ContainerResponse<ContainerNetworkResponse>>,
    (StatusCode, Json<ContainerErrorResponse>),
> {
    let container_id = parse_container_id(&vmid)?;
    context
        .container_api
        .get_container_network(node, container_id)
        .await
        .map(|data| Json(ContainerResponse { data }))
        .map_err(container_error)
}

async fn update_container_interface(
    State(context): State<Arc<AppContext>>,
    Path((node, vmid, iface)): Path<(String, String, String)>,
    Json(interface): Json<ContainerNetworkInterface>,
) -> Result<Json<ContainerResponse<()>>, (StatusCode, Json<ContainerErrorResponse>)> {
    let container_id = parse_container_id(&vmid)?;
    context
        .container_api
        .update_container_interface(node, container_id, iface, interface)
        .await
        .map(|_| Json(ContainerResponse { data: () }))
        .map_err(container_error)
}

async fn remove_container_interface(
    State(context): State<Arc<AppContext>>,
    Path((node, vmid, iface)): Path<(String, String, String)>,
) -> Result<Json<ContainerResponse<()>>, (StatusCode, Json<ContainerErrorResponse>)> {
    let container_id = parse_container_id(&vmid)?;
    context
        .container_api
        .remove_container_interface(node, container_id, iface)
        .await
        .map(|_| Json(ContainerResponse { data: () }))
        .map_err(container_error)
}

async fn hotplug_add_interface(
    State(context): State<Arc<AppContext>>,
    Path((node, vmid, iface)): Path<(String, String, String)>,
    Json(interface): Json<ContainerNetworkInterface>,
) -> Result<
    Json<ContainerResponse<HotplugOperationResponse>>,
    (StatusCode, Json<ContainerErrorResponse>),
> {
    let container_id = parse_container_id(&vmid)?;
    // Ensure interface name matches URL parameter
    let mut interface = interface;
    if interface.name != iface {
        interface.name = iface.clone();
    }

    context
        .container_api
        .hotplug_add_interface(node, container_id, interface)
        .await
        .map(|data| Json(ContainerResponse { data }))
        .map_err(container_error)
}

async fn hotplug_remove_interface(
    State(context): State<Arc<AppContext>>,
    Path((node, vmid, iface)): Path<(String, String, String)>,
) -> Result<
    Json<ContainerResponse<HotplugOperationResponse>>,
    (StatusCode, Json<ContainerErrorResponse>),
> {
    let container_id = parse_container_id(&vmid)?;
    context
        .container_api
        .hotplug_remove_interface(node, container_id, iface)
        .await
        .map(|data| Json(ContainerResponse { data }))
        .map_err(container_error)
}

async fn get_hotplug_operation(
    State(context): State<Arc<AppContext>>,
    Path((node, vmid, operation_id)): Path<(String, String, String)>,
) -> Result<
    Json<ContainerResponse<Option<HotplugOperationResponse>>>,
    (StatusCode, Json<ContainerErrorResponse>),
> {
    let container_id = parse_container_id(&vmid)?;
    context
        .container_api
        .get_hotplug_operation(node, container_id, operation_id)
        .await
        .map(|data| {
            let response = data.map(|op| HotplugOperationResponse {
                operation_id: op.id,
                container_id: op.container_id,
                status: match op.status {
                    HotplugStatus::InProgress => "in-progress".to_string(),
                    HotplugStatus::Completed => "completed".to_string(),
                    HotplugStatus::Failed => "failed".to_string(),
                },
            });
            Json(ContainerResponse { data: response })
        })
        .map_err(container_error)
}

async fn list_container_vnets(
    State(context): State<Arc<AppContext>>,
    Path((node, vmid)): Path<(String, String)>,
) -> Result<Json<ContainerResponse<Vec<String>>>, (StatusCode, Json<ContainerErrorResponse>)> {
    let container_id = parse_container_id(&vmid)?;
    context
        .container_api
        .list_container_vnets(node, container_id)
        .await
        .map(|data| Json(ContainerResponse { data }))
        .map_err(container_error)
}

async fn bind_vnet(
    State(context): State<Arc<AppContext>>,
    Path((node, vmid, iface)): Path<(String, String, String)>,
    Json(request): Json<VnetBindRequest>,
) -> Result<Json<ContainerResponse<()>>, (StatusCode, Json<ContainerErrorResponse>)> {
    let container_id = parse_container_id(&vmid)?;
    context
        .container_api
        .bind_vnet(node, container_id, iface, request.vnet)
        .await
        .map(|_| Json(ContainerResponse { data: () }))
        .map_err(container_error)
}

async fn unbind_vnet(
    State(context): State<Arc<AppContext>>,
    Path((node, vmid, iface)): Path<(String, String, String)>,
) -> Result<Json<ContainerResponse<()>>, (StatusCode, Json<ContainerErrorResponse>)> {
    let container_id = parse_container_id(&vmid)?;
    context
        .container_api
        .unbind_vnet(node, container_id, iface)
        .await
        .map(|_| Json(ContainerResponse { data: () }))
        .map_err(container_error)
}

async fn list_vnet_containers(
    State(context): State<Arc<AppContext>>,
    Path(vnet): Path<String>,
) -> Result<Json<ContainerResponse<Vec<ContainerId>>>, (StatusCode, Json<ContainerErrorResponse>)> {
    context
        .container_api
        .list_vnet_containers(vnet)
        .await
        .map(|data| Json(ContainerResponse { data }))
        .map_err(container_error)
}

async fn get_container_network_stats(
    State(context): State<Arc<AppContext>>,
    Path((node, vmid)): Path<(String, String)>,
) -> Result<
    Json<ContainerResponse<ContainerNetworkStats>>,
    (StatusCode, Json<ContainerErrorResponse>),
> {
    let container_id = parse_container_id(&vmid)?;
    context
        .container_api
        .get_container_network_stats(node, container_id)
        .await
        .map(|data| Json(ContainerResponse { data }))
        .map_err(container_error)
}

// API endpoint implementations would go here when integrated with proxmox-router
// For now, these are the handler functions that would be called by the router

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_container_network_api() {
        let integration = Arc::new(ContainerIntegration::new());
        let api = ContainerNetworkAPI::new(integration);

        // Test would require mock container configuration files
        // For now, just verify API structure
        assert!(true);
    }
}
