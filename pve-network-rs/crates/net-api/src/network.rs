//! Network API endpoints

use std::collections::HashMap;
use std::sync::Arc;

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::Json,
    routing::{get, post},
    Router,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use pve_network_config::{InterfaceConfig, InterfacesParser, NetworkConfigManager};
use pve_network_core::{AddressMethod, Interface, InterfaceType, NetworkError, Result};
use pve_shared_types::{BondMode, IpAddress};

use crate::context::AppContext;

/// Query parameters for network interface listing
#[derive(Debug, Deserialize)]
pub struct NetworkListQuery {
    /// Interface type filter
    #[serde(rename = "type")]
    pub interface_type: Option<String>,
    /// Only return enabled interfaces
    pub enabled: Option<bool>,
}

/// Query parameters for specific interface
#[derive(Debug, Deserialize)]
pub struct NetworkGetQuery {
    /// Include detailed configuration
    pub detailed: Option<bool>,
}

/// Network interface response format (compatible with Perl API)
#[derive(Debug, Serialize)]
pub struct NetworkInterfaceResponse {
    /// Interface name
    pub iface: String,
    /// Interface type
    #[serde(rename = "type")]
    pub interface_type: String,
    /// Address method
    pub method: String,
    /// IP address (first address if multiple)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub address: Option<String>,
    /// Network mask or prefix length
    #[serde(skip_serializing_if = "Option::is_none")]
    pub netmask: Option<String>,
    /// Gateway address
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gateway: Option<String>,
    /// MTU size
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mtu: Option<u16>,
    /// Interface is enabled/active
    pub active: u8,
    /// Interface is auto-started
    pub autostart: u8,
    /// Bridge ports (for bridge interfaces)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bridge_ports: Option<String>,
    /// Bridge VLAN aware (for bridge interfaces)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bridge_vlan_aware: Option<u8>,
    /// Bond slaves (for bond interfaces)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub slaves: Option<String>,
    /// Bond mode (for bond interfaces)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bond_mode: Option<String>,
    /// VLAN tag (for VLAN interfaces)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vlan_id: Option<u16>,
    /// VLAN parent interface (for VLAN interfaces)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vlan_raw_device: Option<String>,
    /// Additional options
    #[serde(flatten)]
    pub options: HashMap<String, Value>,
    /// Comments
    #[serde(skip_serializing_if = "Option::is_none")]
    pub comments: Option<String>,
}

/// Detailed network interface response
#[derive(Debug, Serialize)]
pub struct NetworkInterfaceDetailResponse {
    /// Interface name
    pub iface: String,
    /// Interface type
    #[serde(rename = "type")]
    pub interface_type: String,
    /// Address method
    pub method: String,
    /// All IP addresses
    pub addresses: Vec<String>,
    /// Gateway address
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gateway: Option<String>,
    /// MTU size
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mtu: Option<u16>,
    /// Interface is enabled/active
    pub active: u8,
    /// Interface is auto-started
    pub autostart: u8,
    /// Interface configuration details
    pub config: Value,
    /// Comments
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub comments: Vec<String>,
}

/// Request body for creating/updating network interfaces
#[derive(Debug, Deserialize)]
pub struct NetworkInterfaceRequest {
    /// Interface name
    pub iface: String,
    /// Interface type
    #[serde(rename = "type")]
    pub interface_type: String,
    /// Address method
    pub method: String,
    /// IP address
    #[serde(skip_serializing_if = "Option::is_none")]
    pub address: Option<String>,
    /// Network mask or prefix length
    #[serde(skip_serializing_if = "Option::is_none")]
    pub netmask: Option<String>,
    /// Gateway address
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gateway: Option<String>,
    /// MTU size
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mtu: Option<u16>,
    /// Interface is auto-started
    #[serde(skip_serializing_if = "Option::is_none")]
    pub autostart: Option<u8>,
    /// Bridge ports (for bridge interfaces)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bridge_ports: Option<String>,
    /// Bridge VLAN aware (for bridge interfaces)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bridge_vlan_aware: Option<u8>,
    /// Bond slaves (for bond interfaces)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub slaves: Option<String>,
    /// Bond mode (for bond interfaces)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bond_mode: Option<String>,
    /// VLAN tag (for VLAN interfaces)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vlan_id: Option<u16>,
    /// VLAN parent interface (for VLAN interfaces)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vlan_raw_device: Option<String>,
    /// Additional options
    #[serde(flatten)]
    pub options: HashMap<String, Value>,
    /// Comments
    #[serde(skip_serializing_if = "Option::is_none")]
    pub comments: Option<String>,
}

/// Response for write operations
#[derive(Debug, Serialize)]
pub struct NetworkOperationResponse {
    /// Success status
    pub success: bool,
    /// Operation message
    pub message: String,
    /// Task ID for async operations
    #[serde(skip_serializing_if = "Option::is_none")]
    pub task_id: Option<String>,
}

/// Network API handler
pub struct NetworkAPI {
    config_manager: Arc<NetworkConfigManager>,
    interfaces_parser: Arc<InterfacesParser>,
}

impl NetworkAPI {
    /// Create new NetworkAPI instance
    pub fn new() -> Self {
        Self {
            config_manager: Arc::new(NetworkConfigManager::new()),
            interfaces_parser: Arc::new(InterfacesParser::new()),
        }
    }

    /// Create with custom dependencies
    pub fn with_config_manager(config_manager: NetworkConfigManager) -> Self {
        Self {
            config_manager: Arc::new(config_manager),
            interfaces_parser: Arc::new(InterfacesParser::new()),
        }
    }

    /// Create with a shared configuration manager instance.
    pub fn with_shared_config_manager(config_manager: Arc<NetworkConfigManager>) -> Self {
        Self {
            config_manager,
            interfaces_parser: Arc::new(InterfacesParser::new()),
        }
    }

    /// Get API router
    pub fn router() -> Router<Arc<AppContext>> {
        Router::new()
            .route(
                "/api2/json/nodes/:node/network",
                get(list_interfaces).post(create_interface),
            )
            .route(
                "/api2/json/nodes/:node/network/:iface",
                get(get_interface)
                    .put(update_interface)
                    .delete(delete_interface),
            )
            .route(
                "/api2/json/nodes/:node/network/:iface/status",
                get(get_interface_status),
            )
            .route(
                "/api2/json/nodes/:node/network/reload",
                post(reload_network),
            )
    }

    /// List all network interfaces
    pub async fn list_interfaces(
        &self,
        node: &str,
        query: NetworkListQuery,
    ) -> Result<Vec<NetworkInterfaceResponse>> {
        log::debug!("Listing network interfaces for node: {}", node);

        let network_config = self.config_manager.load_network_config().await?;
        let mut interfaces = Vec::new();

        for (_name, interface) in &network_config.interfaces {
            // Apply filters
            if let Some(ref type_filter) = query.interface_type {
                let interface_type_str = self.interface_type_to_string(&interface.iface_type);
                if interface_type_str != *type_filter {
                    continue;
                }
            }

            if let Some(enabled_filter) = query.enabled {
                if interface.enabled != enabled_filter {
                    continue;
                }
            }

            let response = self.interface_to_response(interface, &network_config)?;
            interfaces.push(response);
        }

        // Sort by interface name for consistent output
        interfaces.sort_by(|a, b| a.iface.cmp(&b.iface));

        Ok(interfaces)
    }

    /// Get specific network interface
    pub async fn get_interface(
        &self,
        node: &str,
        iface: &str,
        query: NetworkGetQuery,
    ) -> Result<Value> {
        log::debug!("Getting network interface {} for node: {}", iface, node);

        let network_config = self.config_manager.load_network_config().await?;

        let interface = network_config.interfaces.get(iface).ok_or_else(|| {
            NetworkError::Api(pve_network_core::error::ApiError::NotFound {
                resource: format!("interface {}", iface),
            })
        })?;

        if query.detailed.unwrap_or(false) {
            let detailed_response =
                self.interface_to_detailed_response(interface, &network_config)?;
            Ok(serde_json::to_value(detailed_response)?)
        } else {
            let response = self.interface_to_response(interface, &network_config)?;
            Ok(serde_json::to_value(response)?)
        }
    }

    /// Get interface status
    pub async fn get_interface_status(&self, node: &str, iface: &str) -> Result<Value> {
        log::debug!("Getting interface status {} for node: {}", iface, node);

        // This would typically query the actual system state
        // For now, return basic status information
        let status = serde_json::json!({
            "iface": iface,
            "active": 1,
            "link": true,
            "speed": 1000,
            "duplex": "full"
        });

        Ok(status)
    }

    /// Create new network interface
    pub async fn create_interface(
        &self,
        node: &str,
        request: NetworkInterfaceRequest,
    ) -> Result<NetworkOperationResponse> {
        log::debug!(
            "Creating network interface {} for node: {}",
            request.iface,
            node
        );

        // Validate interface name
        self.validate_interface_name(&request.iface)?;

        // Check if interface already exists
        let network_config = self.config_manager.load_network_config().await?;
        if network_config.interfaces.contains_key(&request.iface) {
            return Err(NetworkError::Api(
                pve_network_core::error::ApiError::Conflict {
                    resource: format!("interface {}", request.iface),
                    message: "Interface already exists".to_string(),
                },
            ));
        }

        // Convert request to interface config
        let interface_config = self.request_to_interface_config(&request)?;

        // Create interface using config manager
        self.config_manager
            .update_interface(node, &request.iface, interface_config)
            .await
            .map_err(|e| {
                NetworkError::System(pve_network_core::error::SystemError::ConfigWrite {
                    path: format!("/etc/network/interfaces"),
                    source: e.into(),
                })
            })?;

        Ok(NetworkOperationResponse {
            success: true,
            message: format!("Interface {} created successfully", request.iface),
            task_id: None,
        })
    }

    /// Update existing network interface
    pub async fn update_interface(
        &self,
        node: &str,
        iface: &str,
        request: NetworkInterfaceRequest,
    ) -> Result<NetworkOperationResponse> {
        log::debug!("Updating network interface {} for node: {}", iface, node);

        // Validate interface name matches
        if request.iface != iface {
            return Err(NetworkError::Api(
                pve_network_core::error::ApiError::BadRequest {
                    message: "Interface name in request body must match URL parameter".to_string(),
                },
            ));
        }

        // Check if interface exists
        let network_config = self.config_manager.load_network_config().await?;
        if !network_config.interfaces.contains_key(iface) {
            return Err(NetworkError::Api(
                pve_network_core::error::ApiError::NotFound {
                    resource: format!("interface {}", iface),
                },
            ));
        }

        // Convert request to interface config
        let interface_config = self.request_to_interface_config(&request)?;

        // Update interface using config manager
        self.config_manager
            .update_interface(node, iface, interface_config)
            .await
            .map_err(|e| {
                NetworkError::System(pve_network_core::error::SystemError::ConfigWrite {
                    path: format!("/etc/network/interfaces"),
                    source: e.into(),
                })
            })?;

        Ok(NetworkOperationResponse {
            success: true,
            message: format!("Interface {} updated successfully", iface),
            task_id: None,
        })
    }

    /// Delete network interface
    pub async fn delete_interface(
        &self,
        node: &str,
        iface: &str,
    ) -> Result<NetworkOperationResponse> {
        log::debug!("Deleting network interface {} for node: {}", iface, node);

        // Check if interface exists
        let network_config = self.config_manager.load_network_config().await?;
        if !network_config.interfaces.contains_key(iface) {
            return Err(NetworkError::Api(
                pve_network_core::error::ApiError::NotFound {
                    resource: format!("interface {}", iface),
                },
            ));
        }

        // Prevent deletion of critical interfaces
        if iface == "lo" {
            return Err(NetworkError::Api(
                pve_network_core::error::ApiError::BadRequest {
                    message: "Cannot delete loopback interface".to_string(),
                },
            ));
        }

        // Delete interface using config manager
        self.config_manager
            .remove_interface(node, iface)
            .await
            .map_err(|e| {
                NetworkError::System(pve_network_core::error::SystemError::ConfigWrite {
                    path: format!("/etc/network/interfaces"),
                    source: e.into(),
                })
            })?;

        Ok(NetworkOperationResponse {
            success: true,
            message: format!("Interface {} deleted successfully", iface),
            task_id: None,
        })
    }

    /// Reload network configuration
    pub async fn reload_network(&self, node: &str) -> Result<NetworkOperationResponse> {
        log::debug!("Reloading network configuration for node: {}", node);

        // This would typically trigger a network reload task
        // For now, just return success
        Ok(NetworkOperationResponse {
            success: true,
            message: "Network configuration reload initiated".to_string(),
            task_id: Some(format!(
                "UPID:{}:network_reload:{}",
                node,
                chrono::Utc::now().timestamp()
            )),
        })
    }

    /// Convert interface to API response format
    fn interface_to_response(
        &self,
        interface: &Interface,
        network_config: &pve_network_core::NetworkConfiguration,
    ) -> Result<NetworkInterfaceResponse> {
        let interface_type = self.interface_type_to_string(&interface.iface_type);
        let method = self.address_method_to_string(&interface.method);

        // Get first address for compatibility
        let (address, netmask) = if let Some(first_addr) = interface.addresses.first() {
            let addr_str = first_addr.addr.to_string();
            let mask_str = if let Some(prefix_len) = first_addr.prefix_len {
                Some(self.prefix_len_to_netmask(prefix_len, first_addr.addr.is_ipv4()))
            } else {
                None
            };
            (Some(addr_str), mask_str)
        } else {
            (None, None)
        };

        let gateway = interface.gateway.as_ref().map(|gw| gw.addr.to_string());

        let active = if interface.enabled { 1 } else { 0 };
        let autostart = if network_config.auto_interfaces.contains(&interface.name) {
            1
        } else {
            0
        };

        let mut response = NetworkInterfaceResponse {
            iface: interface.name.clone(),
            interface_type,
            method,
            address,
            netmask,
            gateway,
            mtu: interface.mtu,
            active,
            autostart,
            bridge_ports: None,
            bridge_vlan_aware: None,
            slaves: None,
            bond_mode: None,
            vlan_id: None,
            vlan_raw_device: None,
            options: HashMap::new(),
            comments: None,
        };

        // Add interface-specific fields
        match &interface.iface_type {
            InterfaceType::Bridge { ports, vlan_aware } => {
                response.bridge_ports = Some(if ports.is_empty() {
                    "none".to_string()
                } else {
                    ports.join(" ")
                });
                response.bridge_vlan_aware = Some(if *vlan_aware { 1 } else { 0 });
            }
            InterfaceType::Bond { slaves, mode, .. } => {
                response.slaves = Some(if slaves.is_empty() {
                    "none".to_string()
                } else {
                    slaves.join(" ")
                });
                response.bond_mode = Some(self.bond_mode_to_string(mode));
            }
            InterfaceType::Vlan { parent, tag } => {
                response.vlan_id = Some(*tag);
                response.vlan_raw_device = Some(parent.clone());
            }
            _ => {}
        }

        // Add additional options as flattened fields
        for (key, value) in &interface.options {
            response
                .options
                .insert(key.clone(), Value::String(value.clone()));
        }

        // Add comments
        if !interface.comments.is_empty() {
            response.comments = Some(interface.comments.join("\n"));
        }

        Ok(response)
    }

    /// Convert interface to detailed API response format
    fn interface_to_detailed_response(
        &self,
        interface: &Interface,
        network_config: &pve_network_core::NetworkConfiguration,
    ) -> Result<NetworkInterfaceDetailResponse> {
        let interface_type = self.interface_type_to_string(&interface.iface_type);
        let method = self.address_method_to_string(&interface.method);

        let addresses: Vec<String> = interface
            .addresses
            .iter()
            .map(|addr| addr.to_string())
            .collect();

        let gateway = interface.gateway.as_ref().map(|gw| gw.addr.to_string());

        let active = if interface.enabled { 1 } else { 0 };
        let autostart = if network_config.auto_interfaces.contains(&interface.name) {
            1
        } else {
            0
        };

        // Build detailed config object
        let mut config = serde_json::Map::new();
        config.insert("type".to_string(), Value::String(interface_type.clone()));
        config.insert("method".to_string(), Value::String(method.clone()));

        if let Some(mtu) = interface.mtu {
            config.insert("mtu".to_string(), Value::Number(mtu.into()));
        }

        // Add interface-specific config
        match &interface.iface_type {
            InterfaceType::Bridge { ports, vlan_aware } => {
                config.insert("bridge_ports".to_string(), Value::String(ports.join(" ")));
                config.insert("bridge_vlan_aware".to_string(), Value::Bool(*vlan_aware));
            }
            InterfaceType::Bond {
                slaves,
                mode,
                options,
            } => {
                config.insert("bond_slaves".to_string(), Value::String(slaves.join(" ")));
                config.insert(
                    "bond_mode".to_string(),
                    Value::String(self.bond_mode_to_string(mode)),
                );
                for (key, value) in options {
                    config.insert(key.clone(), Value::String(value.clone()));
                }
            }
            InterfaceType::Vlan { parent, tag } => {
                config.insert("vlan_raw_device".to_string(), Value::String(parent.clone()));
                config.insert("vlan_id".to_string(), Value::Number((*tag).into()));
            }
            InterfaceType::Vxlan {
                id,
                local,
                remote,
                dstport,
            } => {
                config.insert("vxlan_id".to_string(), Value::Number((*id).into()));
                config.insert(
                    "vxlan_local".to_string(),
                    Value::String(local.addr.to_string()),
                );
                if let Some(remote_addr) = remote {
                    config.insert(
                        "vxlan_remote".to_string(),
                        Value::String(remote_addr.addr.to_string()),
                    );
                }
                if let Some(port) = dstport {
                    config.insert("vxlan_dstport".to_string(), Value::Number((*port).into()));
                }
            }
            _ => {}
        }

        // Add additional options
        for (key, value) in &interface.options {
            config.insert(key.clone(), Value::String(value.clone()));
        }

        Ok(NetworkInterfaceDetailResponse {
            iface: interface.name.clone(),
            interface_type,
            method,
            addresses,
            gateway,
            mtu: interface.mtu,
            active,
            autostart,
            config: Value::Object(config),
            comments: interface.comments.clone(),
        })
    }

    /// Convert interface type to string
    fn interface_type_to_string(&self, iface_type: &InterfaceType) -> String {
        match iface_type {
            InterfaceType::Physical => "eth".to_string(),
            InterfaceType::Bridge { .. } => "bridge".to_string(),
            InterfaceType::Bond { .. } => "bond".to_string(),
            InterfaceType::Vlan { .. } => "vlan".to_string(),
            InterfaceType::Vxlan { .. } => "vxlan".to_string(),
            InterfaceType::Loopback => "loopback".to_string(),
        }
    }

    /// Convert address method to string
    fn address_method_to_string(&self, method: &AddressMethod) -> String {
        match method {
            AddressMethod::Static => "static".to_string(),
            AddressMethod::Dhcp => "dhcp".to_string(),
            AddressMethod::Manual => "manual".to_string(),
            AddressMethod::None => "none".to_string(),
        }
    }

    /// Convert bond mode to string
    fn bond_mode_to_string(&self, mode: &pve_network_core::BondMode) -> String {
        match mode {
            pve_network_core::BondMode::RoundRobin => "balance-rr".to_string(),
            pve_network_core::BondMode::ActiveBackup => "active-backup".to_string(),
            pve_network_core::BondMode::Xor => "balance-xor".to_string(),
            pve_network_core::BondMode::Broadcast => "broadcast".to_string(),
            pve_network_core::BondMode::Ieee8023ad => "802.3ad".to_string(),
            pve_network_core::BondMode::BalanceTlb => "balance-tlb".to_string(),
            pve_network_core::BondMode::BalanceAlb => "balance-alb".to_string(),
        }
    }

    /// Convert prefix length to netmask string
    fn prefix_len_to_netmask(&self, prefix_len: u8, is_ipv4: bool) -> String {
        if is_ipv4 {
            match prefix_len {
                32 => "255.255.255.255".to_string(),
                31 => "255.255.255.254".to_string(),
                30 => "255.255.255.252".to_string(),
                29 => "255.255.255.248".to_string(),
                28 => "255.255.255.240".to_string(),
                27 => "255.255.255.224".to_string(),
                26 => "255.255.255.192".to_string(),
                25 => "255.255.255.128".to_string(),
                24 => "255.255.255.0".to_string(),
                23 => "255.255.254.0".to_string(),
                22 => "255.255.252.0".to_string(),
                21 => "255.255.248.0".to_string(),
                20 => "255.255.240.0".to_string(),
                19 => "255.255.224.0".to_string(),
                18 => "255.255.192.0".to_string(),
                17 => "255.255.128.0".to_string(),
                16 => "255.255.0.0".to_string(),
                15 => "255.254.0.0".to_string(),
                14 => "255.252.0.0".to_string(),
                13 => "255.248.0.0".to_string(),
                12 => "255.240.0.0".to_string(),
                11 => "255.224.0.0".to_string(),
                10 => "255.192.0.0".to_string(),
                9 => "255.128.0.0".to_string(),
                8 => "255.0.0.0".to_string(),
                7 => "254.0.0.0".to_string(),
                6 => "252.0.0.0".to_string(),
                5 => "248.0.0.0".to_string(),
                4 => "240.0.0.0".to_string(),
                3 => "224.0.0.0".to_string(),
                2 => "192.0.0.0".to_string(),
                1 => "128.0.0.0".to_string(),
                0 => "0.0.0.0".to_string(),
                _ => format!("/{}", prefix_len), // Fallback to CIDR notation
            }
        } else {
            format!("/{}", prefix_len) // IPv6 always uses CIDR notation
        }
    }

    /// Validate interface name
    fn validate_interface_name(&self, name: &str) -> Result<()> {
        if name.is_empty() {
            return Err(NetworkError::Api(
                pve_network_core::error::ApiError::BadRequest {
                    message: "Interface name cannot be empty".to_string(),
                },
            ));
        }

        if name.len() > 15 {
            return Err(NetworkError::Api(
                pve_network_core::error::ApiError::BadRequest {
                    message: "Interface name too long (max 15 characters)".to_string(),
                },
            ));
        }

        // Check for valid characters
        if !name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-' || c == '.')
        {
            return Err(NetworkError::Api(
                pve_network_core::error::ApiError::BadRequest {
                    message: "Interface name contains invalid characters".to_string(),
                },
            ));
        }

        // Must start with letter
        if !name.chars().next().unwrap_or('0').is_ascii_alphabetic() {
            return Err(NetworkError::Api(
                pve_network_core::error::ApiError::BadRequest {
                    message: "Interface name must start with a letter".to_string(),
                },
            ));
        }

        Ok(())
    }

    /// Convert API request to interface config
    fn request_to_interface_config(
        &self,
        request: &NetworkInterfaceRequest,
    ) -> Result<InterfaceConfig> {
        use pve_network_config::network_config::{
            AddressMethod as ConfigAddressMethod, InterfaceType as ConfigInterfaceType,
        };

        // Parse address method
        let method = match request.method.as_str() {
            "static" => ConfigAddressMethod::Static,
            "dhcp" => ConfigAddressMethod::Dhcp,
            "manual" => ConfigAddressMethod::Manual,
            _ => {
                return Err(NetworkError::Api(
                    pve_network_core::error::ApiError::BadRequest {
                        message: format!("Invalid address method: {}", request.method),
                    },
                ))
            }
        };

        // Build addresses list
        let mut addresses: Vec<IpAddress> = Vec::new();
        if let Some(ref address) = request.address {
            let addr_str = if let Some(ref netmask) = request.netmask {
                if netmask.contains('.') {
                    let prefix_len = self.netmask_to_prefix_len(netmask)?;
                    format!("{}/{}", address, prefix_len)
                } else if netmask.starts_with('/') {
                    format!("{}{}", address, netmask)
                } else {
                    format!("{}/{}", address, netmask)
                }
            } else {
                address.clone()
            };
            let parsed = addr_str.parse().map_err(NetworkError::from)?;
            addresses.push(parsed);
        }

        // Pre-compute type-specific attributes
        let bridge_ports: Vec<String> = request
            .bridge_ports
            .as_ref()
            .map(|ports| ports.split_whitespace().map(|p| p.to_string()).collect())
            .unwrap_or_default();
        let bridge_vlan_aware = request.bridge_vlan_aware.map(|v| v != 0).unwrap_or(false);

        let bond_slaves: Vec<String> = request
            .slaves
            .as_ref()
            .map(|slaves| slaves.split_whitespace().map(|s| s.to_string()).collect())
            .unwrap_or_default();
        let bond_mode = match request.bond_mode.as_ref() {
            Some(mode) => mode.parse::<BondMode>().map_err(NetworkError::from)?,
            None => BondMode::ActiveBackup,
        };

        let vlan_parent = request
            .vlan_raw_device
            .clone()
            .unwrap_or_else(|| "unknown".to_string());
        let vlan_tag = request.vlan_id.unwrap_or_default();

        let option_string = |key: &str| -> Option<String> {
            request.options.get(key).map(|value| match value {
                Value::String(s) => s.clone(),
                Value::Number(n) => n.to_string(),
                Value::Bool(b) => {
                    if *b {
                        "true".to_string()
                    } else {
                        "false".to_string()
                    }
                }
                other => other.to_string(),
            })
        };

        let vxlan_id = option_string("vxlan-id")
            .and_then(|s| s.parse::<u32>().ok())
            .or_else(|| request.vlan_id.map(|id| id as u32))
            .unwrap_or_default();
        let vxlan_local = option_string("vxlan-local")
            .map(|s| s.parse::<IpAddress>().map_err(NetworkError::from))
            .transpose()?
            .unwrap_or_else(|| "127.0.0.1".parse().expect("static IPv4 address"));
        let vxlan_remote = option_string("vxlan-remote")
            .map(|s| s.parse::<IpAddress>().map_err(NetworkError::from))
            .transpose()?;
        let vxlan_dstport = option_string("vxlan-dstport").and_then(|s| s.parse::<u16>().ok());

        // Build options map
        let mut options = HashMap::new();

        match request.interface_type.as_str() {
            "bridge" => {
                if let Some(ports) = request.bridge_ports.as_ref() {
                    options.insert("bridge-ports".to_string(), ports.clone());
                }
                if let Some(vlan_aware) = request.bridge_vlan_aware {
                    options.insert(
                        "bridge-vlan-aware".to_string(),
                        if vlan_aware == 1 { "yes" } else { "no" }.to_string(),
                    );
                }
            }
            "bond" => {
                if let Some(slaves) = request.slaves.as_ref() {
                    options.insert("bond-slaves".to_string(), slaves.clone());
                }
                if let Some(mode) = request.bond_mode.as_ref() {
                    options.insert("bond-mode".to_string(), mode.clone());
                }
            }
            "vlan" => {
                if let Some(parent) = request.vlan_raw_device.as_ref() {
                    options.insert("vlan-raw-device".to_string(), parent.clone());
                }
                if let Some(vlan_id) = request.vlan_id {
                    options.insert("vlan-id".to_string(), vlan_id.to_string());
                }
            }
            _ => {}
        }

        // Add additional options from request payload
        for (key, value) in &request.options {
            if let Value::String(s) = value {
                options.insert(key.clone(), s.clone());
            } else {
                options.insert(key.clone(), value.to_string());
            }
        }

        // Derive interface type using collected attributes
        let iface_type = match request.interface_type.as_str() {
            "eth" | "physical" => ConfigInterfaceType::Physical,
            "bridge" => ConfigInterfaceType::Bridge {
                ports: bridge_ports,
                vlan_aware: bridge_vlan_aware,
            },
            "bond" => ConfigInterfaceType::Bond {
                slaves: bond_slaves,
                mode: bond_mode,
                options: options.clone(),
            },
            "vlan" => ConfigInterfaceType::Vlan {
                parent: vlan_parent,
                tag: vlan_tag,
            },
            "vxlan" => ConfigInterfaceType::Vxlan {
                id: vxlan_id,
                local: vxlan_local,
                remote: vxlan_remote,
                dstport: vxlan_dstport,
            },
            _ => {
                return Err(NetworkError::Api(
                    pve_network_core::error::ApiError::BadRequest {
                        message: format!("Invalid interface type: {}", request.interface_type),
                    },
                ))
            }
        };

        let gateway = match &request.gateway {
            Some(gw) => Some(gw.parse().map_err(NetworkError::from)?),
            None => None,
        };

        let comments = request
            .comments
            .as_ref()
            .map(|c| c.lines().map(|line| line.to_string()).collect())
            .unwrap_or_default();

        Ok(InterfaceConfig {
            name: request.iface.clone(),
            iface_type,
            method,
            addresses,
            gateway,
            mtu: request.mtu,
            options,
            enabled: true,
            comments,
        })
    }

    /// Convert dotted decimal netmask to prefix length
    fn netmask_to_prefix_len(&self, netmask: &str) -> Result<u8> {
        match netmask {
            "255.255.255.255" => Ok(32),
            "255.255.255.254" => Ok(31),
            "255.255.255.252" => Ok(30),
            "255.255.255.248" => Ok(29),
            "255.255.255.240" => Ok(28),
            "255.255.255.224" => Ok(27),
            "255.255.255.192" => Ok(26),
            "255.255.255.128" => Ok(25),
            "255.255.255.0" => Ok(24),
            "255.255.254.0" => Ok(23),
            "255.255.252.0" => Ok(22),
            "255.255.248.0" => Ok(21),
            "255.255.240.0" => Ok(20),
            "255.255.224.0" => Ok(19),
            "255.255.192.0" => Ok(18),
            "255.255.128.0" => Ok(17),
            "255.255.0.0" => Ok(16),
            "255.254.0.0" => Ok(15),
            "255.252.0.0" => Ok(14),
            "255.248.0.0" => Ok(13),
            "255.240.0.0" => Ok(12),
            "255.224.0.0" => Ok(11),
            "255.192.0.0" => Ok(10),
            "255.128.0.0" => Ok(9),
            "255.0.0.0" => Ok(8),
            "254.0.0.0" => Ok(7),
            "252.0.0.0" => Ok(6),
            "248.0.0.0" => Ok(5),
            "240.0.0.0" => Ok(4),
            "224.0.0.0" => Ok(3),
            "192.0.0.0" => Ok(2),
            "128.0.0.0" => Ok(1),
            "0.0.0.0" => Ok(0),
            _ => Err(NetworkError::Api(
                pve_network_core::error::ApiError::BadRequest {
                    message: format!("Invalid netmask: {}", netmask),
                },
            )),
        }
    }
}

impl Default for NetworkAPI {
    fn default() -> Self {
        Self::new()
    }
}

/// Axum handler for listing interfaces
async fn list_interfaces(
    State(context): State<Arc<AppContext>>,
    Path(node): Path<String>,
    Query(query): Query<NetworkListQuery>,
) -> std::result::Result<Json<Vec<NetworkInterfaceResponse>>, (StatusCode, String)> {
    match context.network_api.list_interfaces(&node, query).await {
        Ok(interfaces) => Ok(Json(interfaces)),
        Err(e) => {
            log::error!("Failed to list interfaces: {}", e);
            Err((StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))
        }
    }
}

/// Axum handler for getting specific interface
async fn get_interface(
    State(context): State<Arc<AppContext>>,
    Path((node, iface)): Path<(String, String)>,
    Query(query): Query<NetworkGetQuery>,
) -> std::result::Result<Json<Value>, (StatusCode, String)> {
    match context
        .network_api
        .get_interface(&node, &iface, query)
        .await
    {
        Ok(interface) => Ok(Json(interface)),
        Err(NetworkError::Api(pve_network_core::error::ApiError::NotFound { .. })) => Err((
            StatusCode::NOT_FOUND,
            format!("Interface {} not found", iface),
        )),
        Err(e) => {
            log::error!("Failed to get interface {}: {}", iface, e);
            Err((StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))
        }
    }
}

/// Axum handler for getting interface status
async fn get_interface_status(
    State(context): State<Arc<AppContext>>,
    Path((node, iface)): Path<(String, String)>,
) -> std::result::Result<Json<Value>, (StatusCode, String)> {
    match context
        .network_api
        .get_interface_status(&node, &iface)
        .await
    {
        Ok(status) => Ok(Json(status)),
        Err(e) => {
            log::error!("Failed to get interface status {}: {}", iface, e);
            Err((StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))
        }
    }
}

/// Axum handler for creating interface
async fn create_interface(
    State(context): State<Arc<AppContext>>,
    Path(node): Path<String>,
    Json(request): Json<NetworkInterfaceRequest>,
) -> std::result::Result<Json<NetworkOperationResponse>, (StatusCode, String)> {
    match context.network_api.create_interface(&node, request).await {
        Ok(response) => Ok(Json(response)),
        Err(NetworkError::Api(pve_network_core::error::ApiError::Conflict { .. })) => {
            Err((StatusCode::CONFLICT, "Interface already exists".to_string()))
        }
        Err(NetworkError::Api(pve_network_core::error::ApiError::BadRequest { message })) => {
            Err((StatusCode::BAD_REQUEST, message))
        }
        Err(e) => {
            log::error!("Failed to create interface: {}", e);
            Err((StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))
        }
    }
}

/// Axum handler for updating interface
async fn update_interface(
    State(context): State<Arc<AppContext>>,
    Path((node, iface)): Path<(String, String)>,
    Json(request): Json<NetworkInterfaceRequest>,
) -> std::result::Result<Json<NetworkOperationResponse>, (StatusCode, String)> {
    match context
        .network_api
        .update_interface(&node, &iface, request)
        .await
    {
        Ok(response) => Ok(Json(response)),
        Err(NetworkError::Api(pve_network_core::error::ApiError::NotFound { .. })) => Err((
            StatusCode::NOT_FOUND,
            format!("Interface {} not found", iface),
        )),
        Err(NetworkError::Api(pve_network_core::error::ApiError::BadRequest { message })) => {
            Err((StatusCode::BAD_REQUEST, message))
        }
        Err(e) => {
            log::error!("Failed to update interface {}: {}", iface, e);
            Err((StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))
        }
    }
}

/// Axum handler for deleting interface
async fn delete_interface(
    State(context): State<Arc<AppContext>>,
    Path((node, iface)): Path<(String, String)>,
) -> std::result::Result<Json<NetworkOperationResponse>, (StatusCode, String)> {
    match context.network_api.delete_interface(&node, &iface).await {
        Ok(response) => Ok(Json(response)),
        Err(NetworkError::Api(pve_network_core::error::ApiError::NotFound { .. })) => Err((
            StatusCode::NOT_FOUND,
            format!("Interface {} not found", iface),
        )),
        Err(NetworkError::Api(pve_network_core::error::ApiError::BadRequest { message })) => {
            Err((StatusCode::BAD_REQUEST, message))
        }
        Err(e) => {
            log::error!("Failed to delete interface {}: {}", iface, e);
            Err((StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))
        }
    }
}

/// Axum handler for reloading network
async fn reload_network(
    State(context): State<Arc<AppContext>>,
    Path(node): Path<String>,
) -> std::result::Result<Json<NetworkOperationResponse>, (StatusCode, String)> {
    match context.network_api.reload_network(&node).await {
        Ok(response) => Ok(Json(response)),
        Err(e) => {
            log::error!("Failed to reload network for node {}: {}", node, e);
            Err((StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))
        }
    }
}
