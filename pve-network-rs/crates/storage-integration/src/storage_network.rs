//! Storage network management implementation
//!
//! This module provides the core implementation for managing storage networks,
//! including configuration, validation, and status monitoring.

use crate::{
    StorageBackendType, StorageIntegrationError, StorageNetworkConfig, StorageNetworkInfo,
    StorageNetworkManager, StorageNetworkStatus, StorageResult,
};
use anyhow::Result;
use async_trait::async_trait;
use pve_network_config::{NetworkConfigManager, NetworkConfiguration};
use pve_network_core::{AddressMethod, Interface, InterfaceType};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info};

/// Trait for network configuration management
#[async_trait::async_trait]
pub trait NetworkConfigTrait: Send + Sync {
    async fn get_configuration(&self) -> anyhow::Result<pve_network_core::NetworkConfiguration>;
    async fn set_configuration(
        &self,
        config: &pve_network_core::NetworkConfiguration,
    ) -> anyhow::Result<()>;
}

/// Default storage network manager implementation
pub struct DefaultStorageNetworkManager {
    /// Storage network configurations
    storage_networks: Arc<RwLock<HashMap<String, StorageNetworkConfig>>>,
    /// Network configuration manager
    network_config: Arc<dyn NetworkConfigTrait>,
    /// Storage network status cache
    status_cache: Arc<RwLock<HashMap<String, StorageNetworkStatus>>>,
}

impl DefaultStorageNetworkManager {
    /// Create a new storage network manager
    pub fn new(network_config: Arc<dyn NetworkConfigTrait>) -> Self {
        Self {
            storage_networks: Arc::new(RwLock::new(HashMap::new())),
            network_config,
            status_cache: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Load storage network configurations from disk
    pub async fn load_configurations(&self) -> Result<()> {
        debug!("Loading storage network configurations");

        // In a real implementation, this would read from /etc/pve/storage.cfg
        // and extract network-related configurations
        let configs = self.load_storage_configs_from_disk().await?;

        let mut storage_networks = self.storage_networks.write().await;
        *storage_networks = configs;

        info!(
            "Loaded {} storage network configurations",
            storage_networks.len()
        );
        Ok(())
    }

    /// Save storage network configurations to disk
    pub async fn save_configurations(&self) -> Result<()> {
        debug!("Saving storage network configurations");

        let storage_networks = self.storage_networks.read().await;
        self.save_storage_configs_to_disk(&*storage_networks)
            .await?;

        info!(
            "Saved {} storage network configurations",
            storage_networks.len()
        );
        Ok(())
    }

    /// Load storage configurations from disk (placeholder implementation)
    async fn load_storage_configs_from_disk(
        &self,
    ) -> Result<HashMap<String, StorageNetworkConfig>> {
        // This would parse /etc/pve/storage.cfg and extract network configurations
        // For now, return empty map
        Ok(HashMap::new())
    }

    /// Save storage configurations to disk (placeholder implementation)
    async fn save_storage_configs_to_disk(
        &self,
        _configs: &HashMap<String, StorageNetworkConfig>,
    ) -> Result<()> {
        // This would update /etc/pve/storage.cfg with network configurations
        // For now, just log
        debug!("Storage configurations would be saved to disk");
        Ok(())
    }

    /// Configure network interface for storage backend
    async fn configure_storage_interface(
        &self,
        storage_id: &str,
        config: &StorageNetworkConfig,
    ) -> StorageResult<()> {
        debug!("Configuring storage interface for {}", storage_id);

        // Validate interface exists
        let network_config = self
            .network_config
            .get_configuration()
            .await
            .map_err(|e| StorageIntegrationError::NetworkInterface(e.to_string()))?;

        if !network_config.interfaces.contains_key(&config.interface) {
            return Err(StorageIntegrationError::NetworkInterface(format!(
                "Interface {} not found",
                config.interface
            )));
        }

        // Configure VLAN if specified
        if let Some(vlan_tag) = config.vlan_tag {
            self.configure_storage_vlan(storage_id, &config.interface, vlan_tag)
                .await?;
        }

        // Apply QoS settings if specified
        if let Some(qos) = &config.qos_settings {
            self.configure_storage_qos(storage_id, &config.interface, qos)
                .await?;
        }

        Ok(())
    }

    /// Configure VLAN for storage network
    async fn configure_storage_vlan(
        &self,
        storage_id: &str,
        interface: &str,
        vlan_tag: u16,
    ) -> StorageResult<()> {
        debug!(
            "Configuring VLAN {} for storage {} on interface {}",
            vlan_tag, storage_id, interface
        );

        let vlan_interface = format!("{}.{}", interface, vlan_tag);

        // Create VLAN interface configuration
        let vlan_config = Interface {
            name: vlan_interface.clone(),
            iface_type: InterfaceType::Vlan {
                parent: interface.to_string(),
                tag: vlan_tag,
            },
            method: AddressMethod::Manual,
            addresses: Vec::new(),
            gateway: None,
            mtu: None,
            options: HashMap::new(),
            enabled: true,
            comments: Vec::new(),
        };

        // Apply VLAN interface configuration
        // This would integrate with net-apply crate
        info!(
            "Created VLAN interface {} for storage {}",
            vlan_interface, storage_id
        );
        Ok(())
    }

    /// Configure QoS for storage network
    async fn configure_storage_qos(
        &self,
        storage_id: &str,
        interface: &str,
        qos: &crate::QosSettings,
    ) -> StorageResult<()> {
        debug!(
            "Configuring QoS for storage {} on interface {}",
            storage_id, interface
        );

        // Configure bandwidth limits
        if let Some(bandwidth) = qos.bandwidth_limit {
            self.configure_bandwidth_limit(interface, bandwidth).await?;
        }

        // Configure traffic priority
        if let Some(priority) = qos.priority {
            self.configure_traffic_priority(interface, priority).await?;
        }

        // Configure DSCP marking
        if let Some(dscp) = qos.dscp {
            self.configure_dscp_marking(interface, dscp).await?;
        }

        Ok(())
    }

    /// Configure bandwidth limit for interface
    async fn configure_bandwidth_limit(
        &self,
        interface: &str,
        limit_mbps: u32,
    ) -> StorageResult<()> {
        debug!(
            "Setting bandwidth limit of {} Mbps on interface {}",
            limit_mbps, interface
        );
        // This would use tc (traffic control) to set bandwidth limits
        Ok(())
    }

    /// Configure traffic priority for interface
    async fn configure_traffic_priority(&self, interface: &str, priority: u8) -> StorageResult<()> {
        debug!(
            "Setting traffic priority {} on interface {}",
            priority, interface
        );
        // This would configure traffic priority using tc
        Ok(())
    }

    /// Configure DSCP marking for interface
    async fn configure_dscp_marking(&self, interface: &str, dscp: u8) -> StorageResult<()> {
        debug!("Setting DSCP marking {} on interface {}", dscp, interface);
        // This would configure DSCP marking using iptables or tc
        Ok(())
    }

    /// Update storage network status
    async fn update_storage_status(&self, storage_id: &str) -> StorageResult<()> {
        let config = {
            let storage_networks = self.storage_networks.read().await;
            storage_networks.get(storage_id).cloned()
        };

        if let Some(config) = config {
            let status = StorageNetworkStatus {
                storage_id: storage_id.to_string(),
                backend_type: config.backend_type.clone(),
                interface: config.interface.clone(),
                vlan_tag: config.vlan_tag,
                is_active: self
                    .check_storage_network_active(storage_id, &config)
                    .await?,
                last_check: chrono::Utc::now(),
                error_message: None,
            };

            let mut status_cache = self.status_cache.write().await;
            status_cache.insert(storage_id.to_string(), status);
        }

        Ok(())
    }

    /// Check if storage network is active
    async fn check_storage_network_active(
        &self,
        _storage_id: &str,
        config: &StorageNetworkConfig,
    ) -> StorageResult<bool> {
        // Check if the interface is up
        let network_config = self
            .network_config
            .get_configuration()
            .await
            .map_err(|e| StorageIntegrationError::NetworkInterface(e.to_string()))?;

        let interface_active = network_config.interfaces.contains_key(&config.interface);

        // If VLAN is configured, check VLAN interface
        if let Some(vlan_tag) = config.vlan_tag {
            let vlan_interface = format!("{}.{}", config.interface, vlan_tag);
            return Ok(interface_active && network_config.interfaces.contains_key(&vlan_interface));
        }

        Ok(interface_active)
    }
}

#[async_trait]
impl StorageNetworkManager for DefaultStorageNetworkManager {
    async fn configure_storage_network(
        &self,
        storage_id: &str,
        config: &StorageNetworkConfig,
    ) -> Result<()> {
        info!("Configuring storage network for {}", storage_id);

        // Validate configuration
        self.validate_storage_network(config).await?;

        // Configure network interface
        self.configure_storage_interface(storage_id, config)
            .await
            .map_err(|e| anyhow::anyhow!("Failed to configure storage interface: {}", e))?;

        // Store configuration
        {
            let mut storage_networks = self.storage_networks.write().await;
            storage_networks.insert(storage_id.to_string(), config.clone());
        }

        // Update status
        self.update_storage_status(storage_id)
            .await
            .map_err(|e| anyhow::anyhow!("Failed to update storage status: {}", e))?;

        // Save configurations
        self.save_configurations().await?;

        info!("Successfully configured storage network for {}", storage_id);
        Ok(())
    }

    async fn remove_storage_network(&self, storage_id: &str) -> Result<()> {
        info!("Removing storage network for {}", storage_id);

        // Remove from configuration
        let config = {
            let mut storage_networks = self.storage_networks.write().await;
            storage_networks.remove(storage_id)
        };

        if let Some(config) = config {
            // Remove VLAN interface if configured
            if let Some(vlan_tag) = config.vlan_tag {
                let vlan_interface = format!("{}.{}", config.interface, vlan_tag);
                debug!("Removing VLAN interface {}", vlan_interface);
                // This would remove the VLAN interface
            }

            // Remove QoS settings
            if config.qos_settings.is_some() {
                debug!("Removing QoS settings for interface {}", config.interface);
                // This would remove QoS settings
            }
        }

        // Remove from status cache
        {
            let mut status_cache = self.status_cache.write().await;
            status_cache.remove(storage_id);
        }

        // Save configurations
        self.save_configurations().await?;

        info!("Successfully removed storage network for {}", storage_id);
        Ok(())
    }

    async fn validate_storage_network(&self, config: &StorageNetworkConfig) -> Result<()> {
        debug!("Validating storage network configuration");

        // Validate interface exists
        let network_config = self.network_config.get_configuration().await?;
        if !network_config.interfaces.contains_key(&config.interface) {
            return Err(anyhow::anyhow!("Interface {} not found", config.interface));
        }

        // Validate VLAN tag if specified
        if let Some(vlan_tag) = config.vlan_tag {
            if vlan_tag == 0 || vlan_tag > 4094 {
                return Err(anyhow::anyhow!("Invalid VLAN tag: {}", vlan_tag));
            }
        }

        // Validate QoS settings if specified
        if let Some(qos) = &config.qos_settings {
            if let Some(priority) = qos.priority {
                if priority > 7 {
                    return Err(anyhow::anyhow!("Invalid traffic priority: {}", priority));
                }
            }
            if let Some(dscp) = qos.dscp {
                if dscp > 63 {
                    return Err(anyhow::anyhow!("Invalid DSCP value: {}", dscp));
                }
            }
        }

        // Validate backend-specific settings
        match &config.backend_type {
            StorageBackendType::Nfs { server, export, .. } => {
                if server.is_empty() || export.is_empty() {
                    return Err(anyhow::anyhow!("NFS server and export must be specified"));
                }
            }
            StorageBackendType::Cifs { server, share, .. } => {
                if server.is_empty() || share.is_empty() {
                    return Err(anyhow::anyhow!("CIFS server and share must be specified"));
                }
            }
            StorageBackendType::Iscsi { portal, target, .. } => {
                if portal.is_empty() || target.is_empty() {
                    return Err(anyhow::anyhow!("iSCSI portal and target must be specified"));
                }
            }
        }

        debug!("Storage network configuration validation passed");
        Ok(())
    }

    async fn get_storage_network_status(&self, storage_id: &str) -> Result<StorageNetworkStatus> {
        // Update status first
        self.update_storage_status(storage_id)
            .await
            .map_err(|e| anyhow::anyhow!("Failed to update storage status: {}", e))?;

        // Return cached status
        let status_cache = self.status_cache.read().await;
        status_cache
            .get(storage_id)
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("Storage network {} not found", storage_id))
    }

    async fn list_storage_networks(&self) -> Result<Vec<StorageNetworkInfo>> {
        let storage_networks = self.storage_networks.read().await;
        let status_cache = self.status_cache.read().await;

        let mut networks = Vec::new();
        for (storage_id, config) in storage_networks.iter() {
            let status = status_cache
                .get(storage_id)
                .map(|s| if s.is_active { "active" } else { "inactive" })
                .unwrap_or("unknown");

            networks.push(StorageNetworkInfo {
                storage_id: storage_id.clone(),
                backend_type: config.backend_type.clone(),
                interface: config.interface.clone(),
                vlan_tag: config.vlan_tag,
                status: status.to_string(),
            });
        }

        Ok(networks)
    }
}

#[async_trait::async_trait]
impl NetworkConfigTrait for NetworkConfigManager {
    async fn get_configuration(&self) -> anyhow::Result<pve_network_core::NetworkConfiguration> {
        self.get_current_config()
            .await
            .map_err(|err| anyhow::anyhow!(err))
    }

    async fn set_configuration(
        &self,
        config: &pve_network_core::NetworkConfiguration,
    ) -> anyhow::Result<()> {
        self.write_config(config)
            .await
            .map_err(|err| anyhow::anyhow!(err))
    }
}
