//! Storage network API endpoints
//!
//! This module provides REST API endpoints for managing storage network
//! configurations, including network storage backends, VLAN isolation,
//! and path resolution.

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::Json,
    routing::{get, post},
    Router,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use storage_integration::{
    FutureStorageIntegration, QosSettings, StorageBackendConfig, StorageBackendType,
    StorageIntegrationError, StorageNetworkConfig, StorageNetworkInfo, StorageNetworkManager,
    StorageNetworkStatus, StoragePathResolver, StorageResult, StorageVlanConfig, StorageVlanInfo,
    StorageVlanManager,
};

use crate::context::AppContext;

/// Storage network API handler
pub struct StorageNetworkAPI {
    storage_manager: Arc<dyn StorageNetworkManager + Send + Sync>,
    vlan_manager: Arc<tokio::sync::RwLock<StorageVlanManager>>,
    path_resolver: Arc<dyn StoragePathResolver + Send + Sync>,
    future_integration: Arc<dyn FutureStorageIntegration + Send + Sync>,
}

#[derive(Debug, Serialize)]
struct StorageResponse<T> {
    data: T,
}

#[derive(Debug, Serialize)]
struct StorageErrorResponse {
    error: String,
}

#[derive(Debug, Deserialize)]
struct StoragePathQuery {
    path: String,
}

impl StorageNetworkAPI {
    /// Build the axum router for storage network endpoints.
    pub fn router() -> Router<Arc<AppContext>> {
        Router::new()
            .route(
                "/api2/json/nodes/:node/storage/network",
                get(list_storage_networks),
            )
            .route(
                "/api2/json/nodes/:node/storage/network/:storage_id",
                get(get_storage_network)
                    .post(configure_storage_network)
                    .delete(remove_storage_network),
            )
            .route(
                "/api2/json/nodes/:node/storage/network/:storage_id/validate",
                post(validate_storage_network),
            )
            .route(
                "/api2/json/nodes/:node/storage/vlan",
                get(list_storage_vlans),
            )
            .route(
                "/api2/json/nodes/:node/storage/vlan/:storage_id",
                post(create_storage_vlan).delete(remove_storage_vlan),
            )
            .route(
                "/api2/json/nodes/:node/storage/path/:storage_id",
                get(resolve_storage_path),
            )
    }

    /// Create a new storage network API
    pub fn new(
        storage_manager: Arc<dyn StorageNetworkManager + Send + Sync>,
        vlan_manager: Arc<tokio::sync::RwLock<StorageVlanManager>>,
        path_resolver: Arc<dyn StoragePathResolver + Send + Sync>,
        future_integration: Arc<dyn FutureStorageIntegration + Send + Sync>,
    ) -> Self {
        Self {
            storage_manager,
            vlan_manager,
            path_resolver,
            future_integration,
        }
    }

    /// List all storage networks
    /// GET /api2/json/nodes/{node}/storage/network
    pub async fn list_storage_networks(
        &self,
        node: String,
    ) -> StorageResult<Vec<StorageNetworkInfo>> {
        log::info!("Listing storage networks for node {}", node);
        self.storage_manager
            .list_storage_networks()
            .await
            .map_err(StorageIntegrationError::from)
    }

    /// Get storage network configuration
    /// GET /api2/json/nodes/{node}/storage/network/{storage_id}
    pub async fn get_storage_network(
        &self,
        node: String,
        storage_id: String,
    ) -> StorageResult<StorageNetworkStatus> {
        log::info!("Getting storage network {} for node {}", storage_id, node);
        self.storage_manager
            .get_storage_network_status(&storage_id)
            .await
            .map_err(StorageIntegrationError::from)
    }

    /// Get storage network configuration (stub)
    pub async fn get_storage_network_config(
        &self,
        storage_id: String,
    ) -> StorageResult<StorageNetworkConfig> {
        Err(StorageIntegrationError::Configuration(format!(
            "retrieving storage network config for {} is not implemented",
            storage_id
        )))
    }

    /// Configure storage network
    /// POST /api2/json/nodes/{node}/storage/network/{storage_id}
    pub async fn configure_storage_network(
        &self,
        node: String,
        storage_id: String,
        config: StorageNetworkConfigRequest,
    ) -> StorageResult<()> {
        log::info!(
            "Configuring storage network {} for node {}",
            storage_id,
            node
        );

        let storage_config = StorageNetworkConfig {
            backend_type: config.backend_type,
            interface: config.interface,
            vlan_tag: config.vlan_tag,
            network_options: config.network_options.unwrap_or_default(),
            qos_settings: config.qos_settings,
        };

        self.storage_manager
            .configure_storage_network(&storage_id, &storage_config)
            .await
            .map_err(StorageIntegrationError::from)
    }

    /// Remove storage network configuration
    /// DELETE /api2/json/nodes/{node}/storage/network/{storage_id}
    pub async fn remove_storage_network(
        &self,
        node: String,
        storage_id: String,
    ) -> StorageResult<()> {
        log::info!("Removing storage network {} for node {}", storage_id, node);
        self.storage_manager
            .remove_storage_network(&storage_id)
            .await
            .map_err(StorageIntegrationError::from)
    }

    /// List storage VLANs
    /// GET /api2/json/nodes/{node}/storage/vlan
    pub async fn list_storage_vlans(&self, node: String) -> StorageResult<Vec<StorageVlanInfo>> {
        log::info!("Listing storage VLANs for node {}", node);
        let vlan_manager = self.vlan_manager.read().await;
        Ok(vlan_manager.list_storage_vlans())
    }

    /// Create storage VLAN
    /// POST /api2/json/nodes/{node}/storage/vlan/{storage_id}
    pub async fn create_storage_vlan(
        &self,
        node: String,
        storage_id: String,
        config: StorageVlanConfigRequest,
    ) -> StorageResult<String> {
        log::info!("Creating storage VLAN for {} on node {}", storage_id, node);

        let vlan_config = StorageVlanConfig {
            base_interface: config.base_interface,
            vlan_tag: config.vlan_tag,
            subnet: config.subnet,
            gateway: config.gateway,
            mtu: config.mtu,
            options: config.options.unwrap_or_default(),
        };

        let mut vlan_manager = self.vlan_manager.write().await;
        vlan_manager
            .create_storage_vlan(&storage_id, &vlan_config)
            .await
    }

    /// Remove storage VLAN
    /// DELETE /api2/json/nodes/{node}/storage/vlan/{storage_id}
    pub async fn remove_storage_vlan(&self, node: String, storage_id: String) -> StorageResult<()> {
        log::info!("Removing storage VLAN for {} on node {}", storage_id, node);
        let mut vlan_manager = self.vlan_manager.write().await;
        vlan_manager.remove_storage_vlan(&storage_id).await
    }

    /// Resolve storage path
    /// GET /api2/json/nodes/{node}/storage/path/{storage_id}
    pub async fn resolve_storage_path(
        &self,
        node: String,
        storage_id: String,
        path: String,
    ) -> StorageResult<StoragePathResponse> {
        log::info!(
            "Resolving storage path {} for {} on node {}",
            path,
            storage_id,
            node
        );

        let resolved_path = self.path_resolver.resolve_path(&storage_id, &path)?;
        let mount_point = self.path_resolver.get_mount_point(&storage_id)?;
        let is_accessible = self.path_resolver.is_path_accessible(&resolved_path)?;

        Ok(StoragePathResponse {
            storage_id,
            requested_path: path,
            resolved_path,
            mount_point,
            is_accessible,
        })
    }

    /// Validate storage network configuration
    /// POST /api2/json/nodes/{node}/storage/network/{storage_id}/validate
    pub async fn validate_storage_network(
        &self,
        node: String,
        storage_id: String,
        config: StorageNetworkConfigRequest,
    ) -> StorageResult<ValidationResponse> {
        log::info!(
            "Validating storage network {} for node {}",
            storage_id,
            node
        );

        let storage_config = StorageNetworkConfig {
            backend_type: config.backend_type,
            interface: config.interface,
            vlan_tag: config.vlan_tag,
            network_options: config.network_options.unwrap_or_default(),
            qos_settings: config.qos_settings,
        };

        match self
            .storage_manager
            .validate_storage_network(&storage_config)
            .await
        {
            Ok(()) => Ok(ValidationResponse {
                valid: true,
                errors: Vec::new(),
                warnings: Vec::new(),
            }),
            Err(e) => Ok(ValidationResponse {
                valid: false,
                errors: vec![e.to_string()],
                warnings: Vec::new(),
            }),
        }
    }

    /// Register storage backend (future integration)
    /// POST /api2/json/storage/backends/{storage_id}
    pub async fn register_storage_backend(
        &self,
        storage_id: String,
        config: StorageBackendConfigRequest,
    ) -> StorageResult<()> {
        log::info!("Registering storage backend {}", storage_id);

        let backend_config = StorageBackendConfig {
            storage_id: storage_id.clone(),
            backend_type: config.backend_type,
            network_config: config.network_config,
            mount_options: config.mount_options.unwrap_or_default(),
            performance_settings: config.performance_settings,
            security_settings: config.security_settings,
        };

        self.future_integration
            .register_storage_backend(&storage_id, backend_config)
            .await
            .map_err(StorageIntegrationError::from)
    }

    /// List storage backends (future integration)
    /// GET /api2/json/storage/backends
    pub async fn list_storage_backends(
        &self,
    ) -> StorageResult<Vec<storage_integration::StorageBackendInfo>> {
        log::info!("Listing storage backends");
        self.future_integration
            .list_storage_backends()
            .await
            .map_err(StorageIntegrationError::from)
    }

    /// Get storage backend status (future integration)
    /// GET /api2/json/storage/backends/{storage_id}/status
    pub async fn get_storage_backend_status(
        &self,
        storage_id: String,
    ) -> StorageResult<storage_integration::StorageNetworkStatus> {
        log::info!("Getting storage backend status for {}", storage_id);
        self.future_integration
            .get_storage_network_status(&storage_id)
            .await
            .map_err(StorageIntegrationError::from)
    }
}

/// Request structure for storage network configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageNetworkConfigRequest {
    pub backend_type: StorageBackendType,
    pub interface: String,
    pub vlan_tag: Option<u16>,
    pub network_options: Option<HashMap<String, String>>,
    pub qos_settings: Option<QosSettings>,
}

/// Request structure for storage VLAN configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageVlanConfigRequest {
    pub base_interface: String,
    pub vlan_tag: u16,
    pub subnet: Option<String>,
    pub gateway: Option<String>,
    pub mtu: Option<u16>,
    pub options: Option<HashMap<String, String>>,
}

/// Request structure for storage backend configuration (future integration)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageBackendConfigRequest {
    pub backend_type: StorageBackendType,
    pub network_config: storage_integration::StorageNetworkConfig,
    pub mount_options: Option<HashMap<String, String>>,
    pub performance_settings: storage_integration::PerformanceSettings,
    pub security_settings: storage_integration::SecuritySettings,
}

/// Response structure for storage path resolution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoragePathResponse {
    pub storage_id: String,
    pub requested_path: String,
    pub resolved_path: std::path::PathBuf,
    pub mount_point: std::path::PathBuf,
    pub is_accessible: bool,
}

/// Response structure for validation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationResponse {
    pub valid: bool,
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
}

fn map_storage_error(err: StorageIntegrationError) -> (StatusCode, Json<StorageErrorResponse>) {
    use StorageIntegrationError::*;

    let status = match err {
        Configuration(_)
        | UnsupportedBackend(_)
        | NetworkInterface(_)
        | VlanConfiguration(_)
        | PathResolution(_) => StatusCode::BAD_REQUEST,
        StoragePlugin(_) => StatusCode::BAD_GATEWAY,
        System(_) => StatusCode::INTERNAL_SERVER_ERROR,
    };

    (
        status,
        Json(StorageErrorResponse {
            error: err.to_string(),
        }),
    )
}

async fn list_storage_networks(
    State(context): State<Arc<AppContext>>,
    Path(node): Path<String>,
) -> Result<Json<StorageResponse<Vec<StorageNetworkInfo>>>, (StatusCode, Json<StorageErrorResponse>)>
{
    context
        .storage_api
        .list_storage_networks(node)
        .await
        .map(|data| Json(StorageResponse { data }))
        .map_err(map_storage_error)
}

async fn get_storage_network(
    State(context): State<Arc<AppContext>>,
    Path((node, storage_id)): Path<(String, String)>,
) -> Result<Json<StorageResponse<StorageNetworkStatus>>, (StatusCode, Json<StorageErrorResponse>)> {
    context
        .storage_api
        .get_storage_network(node, storage_id)
        .await
        .map(|data| Json(StorageResponse { data }))
        .map_err(map_storage_error)
}

async fn configure_storage_network(
    State(context): State<Arc<AppContext>>,
    Path((node, storage_id)): Path<(String, String)>,
    Json(body): Json<StorageNetworkConfigRequest>,
) -> Result<Json<StorageResponse<()>>, (StatusCode, Json<StorageErrorResponse>)> {
    context
        .storage_api
        .configure_storage_network(node, storage_id, body)
        .await
        .map(|_| Json(StorageResponse { data: () }))
        .map_err(map_storage_error)
}

async fn remove_storage_network(
    State(context): State<Arc<AppContext>>,
    Path((node, storage_id)): Path<(String, String)>,
) -> Result<Json<StorageResponse<()>>, (StatusCode, Json<StorageErrorResponse>)> {
    context
        .storage_api
        .remove_storage_network(node, storage_id)
        .await
        .map(|_| Json(StorageResponse { data: () }))
        .map_err(map_storage_error)
}

async fn validate_storage_network(
    State(context): State<Arc<AppContext>>,
    Path((node, storage_id)): Path<(String, String)>,
    Json(body): Json<StorageNetworkConfigRequest>,
) -> Result<Json<StorageResponse<ValidationResponse>>, (StatusCode, Json<StorageErrorResponse>)> {
    context
        .storage_api
        .validate_storage_network(node, storage_id, body)
        .await
        .map(|data| Json(StorageResponse { data }))
        .map_err(map_storage_error)
}

async fn list_storage_vlans(
    State(context): State<Arc<AppContext>>,
    Path(node): Path<String>,
) -> Result<Json<StorageResponse<Vec<StorageVlanInfo>>>, (StatusCode, Json<StorageErrorResponse>)> {
    context
        .storage_api
        .list_storage_vlans(node)
        .await
        .map(|data| Json(StorageResponse { data }))
        .map_err(map_storage_error)
}

async fn create_storage_vlan(
    State(context): State<Arc<AppContext>>,
    Path((node, storage_id)): Path<(String, String)>,
    Json(body): Json<StorageVlanConfigRequest>,
) -> Result<Json<StorageResponse<String>>, (StatusCode, Json<StorageErrorResponse>)> {
    context
        .storage_api
        .create_storage_vlan(node, storage_id, body)
        .await
        .map(|data| Json(StorageResponse { data }))
        .map_err(map_storage_error)
}

async fn remove_storage_vlan(
    State(context): State<Arc<AppContext>>,
    Path((node, storage_id)): Path<(String, String)>,
) -> Result<Json<StorageResponse<()>>, (StatusCode, Json<StorageErrorResponse>)> {
    context
        .storage_api
        .remove_storage_vlan(node, storage_id)
        .await
        .map(|_| Json(StorageResponse { data: () }))
        .map_err(map_storage_error)
}

async fn resolve_storage_path(
    State(context): State<Arc<AppContext>>,
    Path((node, storage_id)): Path<(String, String)>,
    Query(query): Query<StoragePathQuery>,
) -> Result<Json<StorageResponse<StoragePathResponse>>, (StatusCode, Json<StorageErrorResponse>)> {
    context
        .storage_api
        .resolve_storage_path(node, storage_id, query.path)
        .await
        .map(|data| Json(StorageResponse { data }))
        .map_err(map_storage_error)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use tokio_test;

    #[tokio::test]
    async fn test_storage_network_config_request_serialization() {
        let request = StorageNetworkConfigRequest {
            backend_type: StorageBackendType::Nfs {
                server: "192.168.1.100".to_string(),
                export: "/export/data".to_string(),
                version: Some("4".to_string()),
                options: HashMap::new(),
            },
            interface: "eth0".to_string(),
            vlan_tag: Some(100),
            network_options: Some({
                let mut opts = HashMap::new();
                opts.insert("timeout".to_string(), "30".to_string());
                opts
            }),
            qos_settings: Some(QosSettings {
                bandwidth_limit: Some(1000),
                priority: Some(5),
                dscp: Some(46),
            }),
        };

        let json = serde_json::to_string(&request).unwrap();
        let deserialized: StorageNetworkConfigRequest = serde_json::from_str(&json).unwrap();

        assert_eq!(request.interface, deserialized.interface);
        assert_eq!(request.vlan_tag, deserialized.vlan_tag);
    }

    #[tokio::test]
    async fn test_storage_vlan_config_request_serialization() {
        let request = StorageVlanConfigRequest {
            base_interface: "eth0".to_string(),
            vlan_tag: 100,
            subnet: Some("192.168.100.0/24".to_string()),
            gateway: Some("192.168.100.1".to_string()),
            mtu: Some(1500),
            options: Some({
                let mut opts = HashMap::new();
                opts.insert("priority".to_string(), "5".to_string());
                opts
            }),
        };

        let json = serde_json::to_string(&request).unwrap();
        let deserialized: StorageVlanConfigRequest = serde_json::from_str(&json).unwrap();

        assert_eq!(request.base_interface, deserialized.base_interface);
        assert_eq!(request.vlan_tag, deserialized.vlan_tag);
        assert_eq!(request.subnet, deserialized.subnet);
    }

    #[tokio::test]
    async fn test_storage_path_response_serialization() {
        let response = StoragePathResponse {
            storage_id: "nfs1".to_string(),
            requested_path: "images/vm-100-disk-0.qcow2".to_string(),
            resolved_path: PathBuf::from("/mnt/nfs-nfs1/images/vm-100-disk-0.qcow2"),
            mount_point: PathBuf::from("/mnt/nfs-nfs1"),
            is_accessible: true,
        };

        let json = serde_json::to_string(&response).unwrap();
        let deserialized: StoragePathResponse = serde_json::from_str(&json).unwrap();

        assert_eq!(response.storage_id, deserialized.storage_id);
        assert_eq!(response.requested_path, deserialized.requested_path);
        assert_eq!(response.is_accessible, deserialized.is_accessible);
    }

    #[tokio::test]
    async fn test_validation_response_serialization() {
        let response = ValidationResponse {
            valid: false,
            errors: vec![
                "Invalid VLAN tag: 5000".to_string(),
                "Interface not found: eth99".to_string(),
            ],
            warnings: vec!["High bandwidth limit may affect performance".to_string()],
        };

        let json = serde_json::to_string(&response).unwrap();
        let deserialized: ValidationResponse = serde_json::from_str(&json).unwrap();

        assert_eq!(response.valid, deserialized.valid);
        assert_eq!(response.errors.len(), deserialized.errors.len());
        assert_eq!(response.warnings.len(), deserialized.warnings.len());
    }
}
