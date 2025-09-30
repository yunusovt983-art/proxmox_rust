//! SDN API endpoints

use anyhow::Result;
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::Json,
    routing::{get, post},
    Router,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::context::AppContext;
use pve_sdn_core::{
    IpAllocation, IpAllocationRequest, IpamConfig, IpamManager, IpamType, SdnConfiguration,
    SubnetConfig, VNetConfig, ZoneConfig,
};
use std::net::IpAddr;

/// SDN API state
#[derive(Clone)]
pub struct SdnApiState {
    pub config: Arc<RwLock<SdnConfiguration>>,
    pub ipam_manager: Arc<RwLock<IpamManager>>,
}

impl SdnApiState {
    pub fn new() -> Self {
        Self {
            config: Arc::new(RwLock::new(SdnConfiguration::new())),
            ipam_manager: Arc::new(RwLock::new(IpamManager::new())),
        }
    }
}

/// SDN API handler
pub struct SDNAPI;

impl SDNAPI {
    /// Create new SDNAPI instance
    pub fn new() -> Self {
        Self
    }

    /// Get API router
    pub fn router() -> Router<Arc<AppContext>> {
        Router::new()
            // Zone endpoints
            .route("/sdn/zones", get(list_zones).post(create_zone))
            .route(
                "/sdn/zones/:zone",
                get(get_zone).put(update_zone).delete(delete_zone),
            )
            // VNet endpoints
            .route("/sdn/vnets", get(list_vnets).post(create_vnet))
            .route(
                "/sdn/vnets/:vnet",
                get(get_vnet).put(update_vnet).delete(delete_vnet),
            )
            // Subnet endpoints
            .route("/sdn/subnets", get(list_subnets).post(create_subnet))
            .route(
                "/sdn/subnets/:subnet",
                get(get_subnet).put(update_subnet).delete(delete_subnet),
            )
            // Configuration endpoints
            .route("/sdn/config", get(get_config).put(update_config))
            .route("/sdn/reload", post(reload_config))
            // IPAM endpoints
            .route("/sdn/ipam", get(list_ipam_configs).post(create_ipam_config))
            .route(
                "/sdn/ipam/:ipam",
                get(get_ipam_config)
                    .put(update_ipam_config)
                    .delete(delete_ipam_config),
            )
            .route("/sdn/ipam/:ipam/status", get(get_ipam_status))
            // IP allocation endpoints
            .route(
                "/sdn/subnets/:subnet/ips",
                get(list_subnet_ips).post(allocate_ip),
            )
            .route(
                "/sdn/subnets/:subnet/ips/:ip",
                get(get_ip_allocation)
                    .put(update_ip_allocation)
                    .delete(release_ip),
            )
            .route("/sdn/subnets/:subnet/next-free-ip", get(get_next_free_ip))
    }

    /// List defined SDN zones (stub implementation)
    pub async fn list_zones(&self) -> Result<Vec<ZoneConfig>> {
        Err(anyhow::anyhow!("SDNAPI::list_zones is not implemented"))
    }

    /// Get a single SDN zone (stub implementation)
    pub async fn get_zone(&self, _zone: String) -> Result<ZoneConfig> {
        Err(anyhow::anyhow!("SDNAPI::get_zone is not implemented"))
    }

    /// Create a new SDN zone (stub implementation)
    pub async fn create_zone(&self, _config: ZoneConfig) -> Result<()> {
        Err(anyhow::anyhow!("SDNAPI::create_zone is not implemented"))
    }

    /// Update an existing SDN zone (stub implementation)
    pub async fn update_zone(&self, _zone: String, _config: ZoneConfig) -> Result<()> {
        Err(anyhow::anyhow!("SDNAPI::update_zone is not implemented"))
    }

    /// Delete an SDN zone (stub implementation)
    pub async fn delete_zone(&self, _zone: String) -> Result<()> {
        Err(anyhow::anyhow!("SDNAPI::delete_zone is not implemented"))
    }

    /// List defined VNets (stub implementation)
    pub async fn list_vnets(&self) -> Result<Vec<VNetConfig>> {
        Err(anyhow::anyhow!("SDNAPI::list_vnets is not implemented"))
    }

    /// Get a single VNet (stub implementation)
    pub async fn get_vnet(&self, _vnet: String) -> Result<VNetConfig> {
        Err(anyhow::anyhow!("SDNAPI::get_vnet is not implemented"))
    }

    /// Create a VNet (stub implementation)
    pub async fn create_vnet(&self, _config: VNetConfig) -> Result<()> {
        Err(anyhow::anyhow!("SDNAPI::create_vnet is not implemented"))
    }

    /// Update a VNet (stub implementation)
    pub async fn update_vnet(&self, _vnet: String, _config: VNetConfig) -> Result<()> {
        Err(anyhow::anyhow!("SDNAPI::update_vnet is not implemented"))
    }

    /// Delete a VNet (stub implementation)
    pub async fn delete_vnet(&self, _vnet: String) -> Result<()> {
        Err(anyhow::anyhow!("SDNAPI::delete_vnet is not implemented"))
    }
}

impl Default for SDNAPI {
    fn default() -> Self {
        Self::new()
    }
}

/// Query parameters for listing
#[derive(Debug, Deserialize)]
pub struct ListQuery {
    pub zone: Option<String>,
    pub vnet: Option<String>,
    pub node: Option<String>,
}

/// Query parameters for IP allocation
#[derive(Debug, Deserialize)]
pub struct IpAllocationQuery {
    pub vmid: Option<u32>,
    pub hostname: Option<String>,
    pub mac: Option<String>,
    pub description: Option<String>,
    pub ip: Option<IpAddr>,
}

/// IP allocation request
#[derive(Debug, Deserialize)]
pub struct IpAllocationRequestApi {
    pub vmid: Option<u32>,
    pub hostname: Option<String>,
    pub mac: Option<String>,
    pub description: Option<String>,
    pub ip: Option<IpAddr>,
}

/// IPAM status response
#[derive(Debug, Serialize)]
pub struct IpamStatusResponse {
    pub name: String,
    pub plugin_type: IpamType,
    pub status: String,
    pub message: Option<String>,
}

/// API response wrapper
#[derive(Debug, Serialize)]
pub struct ApiResponse<T> {
    pub data: T,
}

/// Error response
#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    pub error: String,
}

// Zone endpoints

/// List all zones
pub async fn list_zones(
    State(context): State<Arc<AppContext>>,
    Query(_params): Query<ListQuery>,
) -> Result<Json<ApiResponse<Vec<ZoneConfig>>>, (StatusCode, Json<ErrorResponse>)> {
    let state = context.sdn_state.clone();
    let config = state.config.read().await;
    let zones: Vec<ZoneConfig> = config.zones.values().cloned().collect();

    Ok(Json(ApiResponse { data: zones }))
}

/// Get specific zone
pub async fn get_zone(
    State(context): State<Arc<AppContext>>,
    Path(zone_name): Path<String>,
) -> Result<Json<ApiResponse<ZoneConfig>>, (StatusCode, Json<ErrorResponse>)> {
    let state = context.sdn_state.clone();
    let config = state.config.read().await;

    match config.zones.get(&zone_name) {
        Some(zone) => Ok(Json(ApiResponse { data: zone.clone() })),
        None => Err((
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: format!("Zone '{}' not found", zone_name),
            }),
        )),
    }
}

/// Create new zone
pub async fn create_zone(
    State(context): State<Arc<AppContext>>,
    Json(zone_config): Json<ZoneConfig>,
) -> Result<Json<ApiResponse<()>>, (StatusCode, Json<ErrorResponse>)> {
    let state = context.sdn_state.clone();
    let mut config = state.config.write().await;

    match config.add_zone(zone_config) {
        Ok(()) => Ok(Json(ApiResponse { data: () })),
        Err(e) => Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: e.to_string(),
            }),
        )),
    }
}

/// Update existing zone
pub async fn update_zone(
    State(context): State<Arc<AppContext>>,
    Path(zone_name): Path<String>,
    Json(mut zone_config): Json<ZoneConfig>,
) -> Result<Json<ApiResponse<()>>, (StatusCode, Json<ErrorResponse>)> {
    let state = context.sdn_state.clone();
    let mut config = state.config.write().await;

    // Ensure zone name matches path
    zone_config.zone = zone_name.clone();

    if !config.zones.contains_key(&zone_name) {
        return Err((
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: format!("Zone '{}' not found", zone_name),
            }),
        ));
    }

    match zone_config.validate() {
        Ok(()) => {
            config.zones.insert(zone_name, zone_config);
            Ok(Json(ApiResponse { data: () }))
        }
        Err(e) => Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: e.to_string(),
            }),
        )),
    }
}

/// Delete zone
pub async fn delete_zone(
    State(context): State<Arc<AppContext>>,
    Path(zone_name): Path<String>,
) -> Result<Json<ApiResponse<()>>, (StatusCode, Json<ErrorResponse>)> {
    let state = context.sdn_state.clone();
    let mut config = state.config.write().await;

    match config.remove_zone(&zone_name) {
        Ok(()) => Ok(Json(ApiResponse { data: () })),
        Err(e) => Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: e.to_string(),
            }),
        )),
    }
}

// VNet endpoints

/// List all VNets
pub async fn list_vnets(
    State(context): State<Arc<AppContext>>,
    Query(params): Query<ListQuery>,
) -> Result<Json<ApiResponse<Vec<VNetConfig>>>, (StatusCode, Json<ErrorResponse>)> {
    let state = context.sdn_state.clone();
    let config = state.config.read().await;
    let mut vnets: Vec<VNetConfig> = config.vnets.values().cloned().collect();

    // Filter by zone if specified
    if let Some(zone_filter) = params.zone {
        vnets.retain(|vnet| vnet.zone == zone_filter);
    }

    Ok(Json(ApiResponse { data: vnets }))
}

/// Get specific VNet
pub async fn get_vnet(
    State(context): State<Arc<AppContext>>,
    Path(vnet_name): Path<String>,
) -> Result<Json<ApiResponse<VNetConfig>>, (StatusCode, Json<ErrorResponse>)> {
    let state = context.sdn_state.clone();
    let config = state.config.read().await;

    match config.vnets.get(&vnet_name) {
        Some(vnet) => Ok(Json(ApiResponse { data: vnet.clone() })),
        None => Err((
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: format!("VNet '{}' not found", vnet_name),
            }),
        )),
    }
}

/// Create new VNet
pub async fn create_vnet(
    State(context): State<Arc<AppContext>>,
    Json(vnet_config): Json<VNetConfig>,
) -> Result<Json<ApiResponse<()>>, (StatusCode, Json<ErrorResponse>)> {
    let state = context.sdn_state.clone();
    let mut config = state.config.write().await;

    match config.add_vnet(vnet_config) {
        Ok(()) => Ok(Json(ApiResponse { data: () })),
        Err(e) => Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: e.to_string(),
            }),
        )),
    }
}

/// Update existing VNet
pub async fn update_vnet(
    State(context): State<Arc<AppContext>>,
    Path(vnet_name): Path<String>,
    Json(mut vnet_config): Json<VNetConfig>,
) -> Result<Json<ApiResponse<()>>, (StatusCode, Json<ErrorResponse>)> {
    let state = context.sdn_state.clone();
    let mut config = state.config.write().await;

    // Ensure VNet name matches path
    vnet_config.vnet = vnet_name.clone();

    if !config.vnets.contains_key(&vnet_name) {
        return Err((
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: format!("VNet '{}' not found", vnet_name),
            }),
        ));
    }

    match vnet_config.validate() {
        Ok(()) => {
            // Validate zone exists
            if !config.zones.contains_key(&vnet_config.zone) {
                return Err((
                    StatusCode::BAD_REQUEST,
                    Json(ErrorResponse {
                        error: format!("Zone '{}' does not exist", vnet_config.zone),
                    }),
                ));
            }

            config.vnets.insert(vnet_name, vnet_config);
            Ok(Json(ApiResponse { data: () }))
        }
        Err(e) => Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: e.to_string(),
            }),
        )),
    }
}

/// Delete VNet
pub async fn delete_vnet(
    State(context): State<Arc<AppContext>>,
    Path(vnet_name): Path<String>,
) -> Result<Json<ApiResponse<()>>, (StatusCode, Json<ErrorResponse>)> {
    let state = context.sdn_state.clone();
    let mut config = state.config.write().await;

    match config.remove_vnet(&vnet_name) {
        Ok(()) => Ok(Json(ApiResponse { data: () })),
        Err(e) => Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: e.to_string(),
            }),
        )),
    }
}

// Subnet endpoints

/// List all subnets
pub async fn list_subnets(
    State(context): State<Arc<AppContext>>,
    Query(params): Query<ListQuery>,
) -> Result<Json<ApiResponse<Vec<SubnetConfig>>>, (StatusCode, Json<ErrorResponse>)> {
    let state = context.sdn_state.clone();
    let config = state.config.read().await;
    let mut subnets: Vec<SubnetConfig> = config.subnets.values().cloned().collect();

    // Filter by VNet if specified
    if let Some(vnet_filter) = params.vnet {
        subnets.retain(|subnet| subnet.vnet == vnet_filter);
    }

    Ok(Json(ApiResponse { data: subnets }))
}

/// Get specific subnet
pub async fn get_subnet(
    State(context): State<Arc<AppContext>>,
    Path(subnet_name): Path<String>,
) -> Result<Json<ApiResponse<SubnetConfig>>, (StatusCode, Json<ErrorResponse>)> {
    let state = context.sdn_state.clone();
    let config = state.config.read().await;

    match config.subnets.get(&subnet_name) {
        Some(subnet) => Ok(Json(ApiResponse {
            data: subnet.clone(),
        })),
        None => Err((
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: format!("Subnet '{}' not found", subnet_name),
            }),
        )),
    }
}

/// Create new subnet
pub async fn create_subnet(
    State(context): State<Arc<AppContext>>,
    Json(subnet_config): Json<SubnetConfig>,
) -> Result<Json<ApiResponse<()>>, (StatusCode, Json<ErrorResponse>)> {
    let state = context.sdn_state.clone();
    let mut config = state.config.write().await;

    match config.add_subnet(subnet_config) {
        Ok(()) => Ok(Json(ApiResponse { data: () })),
        Err(e) => Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: e.to_string(),
            }),
        )),
    }
}

/// Update existing subnet
pub async fn update_subnet(
    State(context): State<Arc<AppContext>>,
    Path(subnet_name): Path<String>,
    Json(mut subnet_config): Json<SubnetConfig>,
) -> Result<Json<ApiResponse<()>>, (StatusCode, Json<ErrorResponse>)> {
    let state = context.sdn_state.clone();
    let mut config = state.config.write().await;

    // Ensure subnet name matches path
    subnet_config.subnet = subnet_name.clone();

    if !config.subnets.contains_key(&subnet_name) {
        return Err((
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: format!("Subnet '{}' not found", subnet_name),
            }),
        ));
    }

    match subnet_config.validate() {
        Ok(()) => {
            // Validate VNet exists
            if !config.vnets.contains_key(&subnet_config.vnet) {
                return Err((
                    StatusCode::BAD_REQUEST,
                    Json(ErrorResponse {
                        error: format!("VNet '{}' does not exist", subnet_config.vnet),
                    }),
                ));
            }

            config.subnets.insert(subnet_name, subnet_config);
            Ok(Json(ApiResponse { data: () }))
        }
        Err(e) => Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: e.to_string(),
            }),
        )),
    }
}

/// Delete subnet
pub async fn delete_subnet(
    State(context): State<Arc<AppContext>>,
    Path(subnet_name): Path<String>,
) -> Result<Json<ApiResponse<()>>, (StatusCode, Json<ErrorResponse>)> {
    let state = context.sdn_state.clone();
    let mut config = state.config.write().await;

    match config.remove_subnet(&subnet_name) {
        Ok(()) => Ok(Json(ApiResponse { data: () })),
        Err(e) => Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: e.to_string(),
            }),
        )),
    }
}

// Configuration endpoints

/// Get complete SDN configuration
pub async fn get_config(
    State(context): State<Arc<AppContext>>,
) -> Result<Json<ApiResponse<SdnConfiguration>>, (StatusCode, Json<ErrorResponse>)> {
    let state = context.sdn_state.clone();
    let config = state.config.read().await;
    Ok(Json(ApiResponse {
        data: config.clone(),
    }))
}

/// Update complete SDN configuration
pub async fn update_config(
    State(context): State<Arc<AppContext>>,
    Json(new_config): Json<SdnConfiguration>,
) -> Result<Json<ApiResponse<()>>, (StatusCode, Json<ErrorResponse>)> {
    let state = context.sdn_state.clone();
    match new_config.validate() {
        Ok(()) => {
            let mut config = state.config.write().await;
            *config = new_config;
            Ok(Json(ApiResponse { data: () }))
        }
        Err(e) => Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: e.to_string(),
            }),
        )),
    }
}

/// Reload SDN configuration
pub async fn reload_config(
    State(context): State<Arc<AppContext>>,
) -> Result<Json<ApiResponse<String>>, (StatusCode, Json<ErrorResponse>)> {
    let state = context.sdn_state.clone();
    let config = state.config.read().await;

    match config.validate() {
        Ok(()) => {
            // In a real implementation, this would:
            // 1. Apply all zone configurations
            // 2. Regenerate network configurations
            // 3. Reload network services

            log::info!("SDN configuration reloaded successfully");
            Ok(Json(ApiResponse {
                data: "SDN configuration reloaded successfully".to_string(),
            }))
        }
        Err(e) => Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: format!("Configuration validation failed: {}", e),
            }),
        )),
    }
}

// IPAM Configuration endpoints

/// List all IPAM configurations
pub async fn list_ipam_configs(
    State(context): State<Arc<AppContext>>,
) -> Result<Json<ApiResponse<Vec<IpamConfig>>>, (StatusCode, Json<ErrorResponse>)> {
    let state = context.sdn_state.clone();
    let config = state.config.read().await;
    let ipam_configs: Vec<IpamConfig> = config.ipams.values().cloned().collect();

    Ok(Json(ApiResponse { data: ipam_configs }))
}

/// Get specific IPAM configuration
pub async fn get_ipam_config(
    State(context): State<Arc<AppContext>>,
    Path(ipam_name): Path<String>,
) -> Result<Json<ApiResponse<IpamConfig>>, (StatusCode, Json<ErrorResponse>)> {
    let state = context.sdn_state.clone();
    let config = state.config.read().await;

    match config.ipams.get(&ipam_name) {
        Some(ipam) => Ok(Json(ApiResponse { data: ipam.clone() })),
        None => Err((
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: format!("IPAM '{}' not found", ipam_name),
            }),
        )),
    }
}

/// Create new IPAM configuration
pub async fn create_ipam_config(
    State(context): State<Arc<AppContext>>,
    Json(ipam_config): Json<IpamConfig>,
) -> Result<Json<ApiResponse<()>>, (StatusCode, Json<ErrorResponse>)> {
    let state = context.sdn_state.clone();
    // Validate configuration
    if let Err(e) = ipam_config.validate() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: format!("IPAM configuration validation failed: {}", e),
            }),
        ));
    }

    let mut config = state.config.write().await;

    if config.ipams.contains_key(&ipam_config.name) {
        return Err((
            StatusCode::CONFLICT,
            Json(ErrorResponse {
                error: format!("IPAM '{}' already exists", ipam_config.name),
            }),
        ));
    }

    config.ipams.insert(ipam_config.name.clone(), ipam_config);

    Ok(Json(ApiResponse { data: () }))
}

/// Update existing IPAM configuration
pub async fn update_ipam_config(
    State(context): State<Arc<AppContext>>,
    Path(ipam_name): Path<String>,
    Json(mut ipam_config): Json<IpamConfig>,
) -> Result<Json<ApiResponse<()>>, (StatusCode, Json<ErrorResponse>)> {
    let state = context.sdn_state.clone();
    // Ensure IPAM name matches path
    ipam_config.name = ipam_name.clone();

    // Validate configuration
    if let Err(e) = ipam_config.validate() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: format!("IPAM configuration validation failed: {}", e),
            }),
        ));
    }

    let mut config = state.config.write().await;

    if !config.ipams.contains_key(&ipam_name) {
        return Err((
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: format!("IPAM '{}' not found", ipam_name),
            }),
        ));
    }

    config.ipams.insert(ipam_name, ipam_config);

    Ok(Json(ApiResponse { data: () }))
}

/// Delete IPAM configuration
pub async fn delete_ipam_config(
    State(context): State<Arc<AppContext>>,
    Path(ipam_name): Path<String>,
) -> Result<Json<ApiResponse<()>>, (StatusCode, Json<ErrorResponse>)> {
    let state = context.sdn_state.clone();
    let mut config = state.config.write().await;

    if config.ipams.remove(&ipam_name).is_none() {
        return Err((
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: format!("IPAM '{}' not found", ipam_name),
            }),
        ));
    }

    Ok(Json(ApiResponse { data: () }))
}

/// Get IPAM status
pub async fn get_ipam_status(
    State(context): State<Arc<AppContext>>,
    Path(ipam_name): Path<String>,
) -> Result<Json<ApiResponse<IpamStatusResponse>>, (StatusCode, Json<ErrorResponse>)> {
    let state = context.sdn_state.clone();
    let config = state.config.read().await;
    let ipam_manager = state.ipam_manager.read().await;

    let ipam_config = config.ipams.get(&ipam_name).ok_or_else(|| {
        (
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: format!("IPAM '{}' not found", ipam_name),
            }),
        )
    })?;

    // Try to get the plugin to check status
    let status = match ipam_manager.get_plugin(&ipam_name) {
        Ok(plugin) => {
            // Test plugin connectivity
            match plugin.validate_config(ipam_config).await {
                Ok(()) => IpamStatusResponse {
                    name: ipam_name.clone(),
                    plugin_type: plugin.plugin_type(),
                    status: "active".to_string(),
                    message: Some("Plugin is operational".to_string()),
                },
                Err(e) => IpamStatusResponse {
                    name: ipam_name.clone(),
                    plugin_type: plugin.plugin_type(),
                    status: "error".to_string(),
                    message: Some(format!("Plugin validation failed: {}", e)),
                },
            }
        }
        Err(_) => IpamStatusResponse {
            name: ipam_name.clone(),
            plugin_type: ipam_config.ipam_type.clone(),
            status: "inactive".to_string(),
            message: Some("Plugin not loaded".to_string()),
        },
    };

    Ok(Json(ApiResponse { data: status }))
}

// IP Allocation endpoints

/// List all IP allocations in a subnet
pub async fn list_subnet_ips(
    State(context): State<Arc<AppContext>>,
    Path(subnet_name): Path<String>,
    Query(_params): Query<ListQuery>,
) -> Result<Json<ApiResponse<Vec<IpAllocation>>>, (StatusCode, Json<ErrorResponse>)> {
    let state = context.sdn_state.clone();
    let config = state.config.read().await;
    let ipam_manager = state.ipam_manager.read().await;

    // Get subnet configuration
    let subnet_config = config.subnets.get(&subnet_name).ok_or_else(|| {
        (
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: format!("Subnet '{}' not found", subnet_name),
            }),
        )
    })?;

    // Determine IPAM plugin to use
    let ipam_name = subnet_config
        .options
        .get("ipam")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    match ipam_manager
        .list_subnet_ips(ipam_name.as_deref(), &subnet_name)
        .await
    {
        Ok(allocations) => Ok(Json(ApiResponse { data: allocations })),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: format!("Failed to list subnet IPs: {}", e),
            }),
        )),
    }
}

/// Allocate IP address in subnet
pub async fn allocate_ip(
    State(context): State<Arc<AppContext>>,
    Path(subnet_name): Path<String>,
    Json(request): Json<IpAllocationRequestApi>,
) -> Result<Json<ApiResponse<IpAllocation>>, (StatusCode, Json<ErrorResponse>)> {
    let state = context.sdn_state.clone();
    let config = state.config.read().await;
    let ipam_manager = state.ipam_manager.read().await;

    // Get subnet configuration
    let subnet_config = config.subnets.get(&subnet_name).ok_or_else(|| {
        (
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: format!("Subnet '{}' not found", subnet_name),
            }),
        )
    })?;

    // Determine IPAM plugin to use
    let ipam_name = subnet_config
        .options
        .get("ipam")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    let allocation_request = IpAllocationRequest {
        subnet: subnet_name.clone(),
        vmid: request.vmid,
        hostname: request.hostname,
        mac: request.mac,
        description: request.description,
        requested_ip: request.ip,
    };

    match ipam_manager
        .allocate_ip(ipam_name.as_deref(), &allocation_request)
        .await
    {
        Ok(allocation) => Ok(Json(ApiResponse { data: allocation })),
        Err(e) => Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: format!("Failed to allocate IP: {}", e),
            }),
        )),
    }
}

/// Get IP allocation information
pub async fn get_ip_allocation(
    State(context): State<Arc<AppContext>>,
    Path((subnet_name, ip_str)): Path<(String, String)>,
) -> Result<Json<ApiResponse<IpAllocation>>, (StatusCode, Json<ErrorResponse>)> {
    let state = context.sdn_state.clone();
    let config = state.config.read().await;
    let ipam_manager = state.ipam_manager.read().await;

    // Parse IP address
    let ip: IpAddr = ip_str.parse().map_err(|_| {
        (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: format!("Invalid IP address: {}", ip_str),
            }),
        )
    })?;

    // Get subnet configuration
    let subnet_config = config.subnets.get(&subnet_name).ok_or_else(|| {
        (
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: format!("Subnet '{}' not found", subnet_name),
            }),
        )
    })?;

    // Determine IPAM plugin to use
    let ipam_name = subnet_config
        .options
        .get("ipam")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    match ipam_manager
        .get_ip(ipam_name.as_deref(), &subnet_name, &ip)
        .await
    {
        Ok(Some(allocation)) => Ok(Json(ApiResponse { data: allocation })),
        Ok(None) => Err((
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: format!("IP {} not allocated in subnet {}", ip, subnet_name),
            }),
        )),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: format!("Failed to get IP allocation: {}", e),
            }),
        )),
    }
}

/// Update IP allocation
pub async fn update_ip_allocation(
    State(context): State<Arc<AppContext>>,
    Path((subnet_name, ip_str)): Path<(String, String)>,
    Json(allocation): Json<IpAllocation>,
) -> Result<Json<ApiResponse<()>>, (StatusCode, Json<ErrorResponse>)> {
    let state = context.sdn_state.clone();
    let config = state.config.read().await;
    let ipam_manager = state.ipam_manager.read().await;

    // Parse IP address
    let ip: IpAddr = ip_str.parse().map_err(|_| {
        (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: format!("Invalid IP address: {}", ip_str),
            }),
        )
    })?;

    // Get subnet configuration
    let subnet_config = config.subnets.get(&subnet_name).ok_or_else(|| {
        (
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: format!("Subnet '{}' not found", subnet_name),
            }),
        )
    })?;

    // Determine IPAM plugin to use
    let ipam_name = subnet_config
        .options
        .get("ipam")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    match ipam_manager
        .update_ip(ipam_name.as_deref(), &subnet_name, &ip, &allocation)
        .await
    {
        Ok(()) => Ok(Json(ApiResponse { data: () })),
        Err(e) => Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: format!("Failed to update IP allocation: {}", e),
            }),
        )),
    }
}

/// Release IP address
pub async fn release_ip(
    State(context): State<Arc<AppContext>>,
    Path((subnet_name, ip_str)): Path<(String, String)>,
) -> Result<Json<ApiResponse<()>>, (StatusCode, Json<ErrorResponse>)> {
    let state = context.sdn_state.clone();
    let config = state.config.read().await;
    let ipam_manager = state.ipam_manager.read().await;

    // Parse IP address
    let ip: IpAddr = ip_str.parse().map_err(|_| {
        (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: format!("Invalid IP address: {}", ip_str),
            }),
        )
    })?;

    // Get subnet configuration
    let subnet_config = config.subnets.get(&subnet_name).ok_or_else(|| {
        (
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: format!("Subnet '{}' not found", subnet_name),
            }),
        )
    })?;

    // Determine IPAM plugin to use
    let ipam_name = subnet_config
        .options
        .get("ipam")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    match ipam_manager
        .release_ip(ipam_name.as_deref(), &subnet_name, &ip)
        .await
    {
        Ok(()) => Ok(Json(ApiResponse { data: () })),
        Err(e) => Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: format!("Failed to release IP: {}", e),
            }),
        )),
    }
}

/// Get next free IP in subnet
pub async fn get_next_free_ip(
    State(context): State<Arc<AppContext>>,
    Path(subnet_name): Path<String>,
) -> Result<Json<ApiResponse<Option<IpAddr>>>, (StatusCode, Json<ErrorResponse>)> {
    let state = context.sdn_state.clone();
    let config = state.config.read().await;
    let ipam_manager = state.ipam_manager.read().await;

    // Get subnet configuration
    let subnet_config = config.subnets.get(&subnet_name).ok_or_else(|| {
        (
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: format!("Subnet '{}' not found", subnet_name),
            }),
        )
    })?;

    // Determine IPAM plugin to use
    let ipam_name = subnet_config
        .options
        .get("ipam")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    match ipam_manager
        .get_next_free_ip(ipam_name.as_deref(), &subnet_name)
        .await
    {
        Ok(ip) => Ok(Json(ApiResponse { data: ip })),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: format!("Failed to get next free IP: {}", e),
            }),
        )),
    }
}
