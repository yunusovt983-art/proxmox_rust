//! SDN configuration management

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Complete SDN configuration
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SdnConfiguration {
    pub zones: HashMap<String, ZoneConfig>,
    pub vnets: HashMap<String, VNetConfig>,
    pub subnets: HashMap<String, SubnetConfig>,
    pub controllers: HashMap<String, ControllerConfig>,
    pub ipams: HashMap<String, IpamConfig>,
}

/// SDN Zone types
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ZoneType {
    Simple,
    Vlan,
    QinQ,
    Vxlan,
    Evpn,
}

/// Zone configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ZoneConfig {
    #[serde(rename = "type")]
    pub zone_type: ZoneType,
    pub bridge: Option<String>,
    #[serde(rename = "vlan-aware")]
    pub vlan_aware: Option<bool>,
    pub tag: Option<u16>,
    #[serde(rename = "vxlan-port")]
    pub vxlan_port: Option<u16>,
    pub peers: Option<Vec<String>>,
    pub mtu: Option<u16>,
    pub nodes: Option<Vec<String>>,
    #[serde(flatten)]
    pub options: HashMap<String, serde_json::Value>,
}

/// VNet configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VNetConfig {
    pub zone: String,
    pub tag: Option<u16>,
    pub alias: Option<String>,
    pub vlanaware: Option<bool>,
    pub mac: Option<String>,
}

/// Subnet configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubnetConfig {
    pub vnet: String,
    pub gateway: Option<String>,
    pub snat: Option<bool>,
    pub dhcp: Option<bool>,
    #[serde(rename = "dhcp-range")]
    pub dhcp_range: Option<Vec<String>>,
    #[serde(rename = "dns-server")]
    pub dns_server: Option<Vec<String>>,
}

/// Controller types
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ControllerType {
    Evpn,
    Bgp,
    Faucet,
}

/// Controller configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ControllerConfig {
    #[serde(rename = "type")]
    pub controller_type: ControllerType,
    pub asn: Option<u32>,
    pub peers: Option<Vec<String>>,
    #[serde(flatten)]
    pub options: HashMap<String, serde_json::Value>,
}

/// IPAM types
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum IpamType {
    Pve,
    Phpipam,
    Netbox,
}

/// IPAM configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IpamConfig {
    #[serde(rename = "type")]
    pub ipam_type: IpamType,
    pub url: Option<String>,
    pub token: Option<String>,
    pub section: Option<String>,
    #[serde(flatten)]
    pub options: HashMap<String, serde_json::Value>,
}

/// SDN configuration manager with cluster synchronization
pub struct SdnConfigManager {
    pmxcfs: crate::pmxcfs::PmxcfsConfig,
}

impl SdnConfigManager {
    /// Create new SDN config manager
    pub fn new() -> anyhow::Result<Self> {
        Ok(Self {
            pmxcfs: crate::pmxcfs::PmxcfsConfig::new()?,
        })
    }

    /// Create SDN config manager with custom pmxcfs config
    pub fn with_pmxcfs(pmxcfs: crate::pmxcfs::PmxcfsConfig) -> Self {
        Self { pmxcfs }
    }

    /// Read complete SDN configuration
    pub async fn read_config(&self) -> anyhow::Result<SdnConfiguration> {
        self.pmxcfs.read_sdn_config().await
    }

    /// Write complete SDN configuration with cluster lock
    pub async fn write_config(&self, config: &SdnConfiguration) -> anyhow::Result<()> {
        self.pmxcfs
            .with_lock("sdn_config", "write_sdn_config", || Ok(()))
            .await?;

        self.pmxcfs.write_sdn_config(config).await
    }

    /// Add or update a zone configuration
    pub async fn update_zone(
        &self,
        zone_name: &str,
        zone_config: ZoneConfig,
    ) -> anyhow::Result<()> {
        self.pmxcfs
            .with_lock("sdn_config", &format!("update_zone_{}", zone_name), || {
                Ok(())
            })
            .await?;

        let mut config = self.read_config().await?;
        config.zones.insert(zone_name.to_string(), zone_config);
        self.pmxcfs.write_sdn_config(&config).await
    }

    /// Remove a zone configuration
    pub async fn remove_zone(&self, zone_name: &str) -> anyhow::Result<()> {
        self.pmxcfs
            .with_lock("sdn_config", &format!("remove_zone_{}", zone_name), || {
                Ok(())
            })
            .await?;

        let mut config = self.read_config().await?;
        config.zones.remove(zone_name);
        self.pmxcfs.write_sdn_config(&config).await
    }

    /// Add or update a vnet configuration
    pub async fn update_vnet(
        &self,
        vnet_name: &str,
        vnet_config: VNetConfig,
    ) -> anyhow::Result<()> {
        self.pmxcfs
            .with_lock("sdn_config", &format!("update_vnet_{}", vnet_name), || {
                Ok(())
            })
            .await?;

        let mut config = self.read_config().await?;
        config.vnets.insert(vnet_name.to_string(), vnet_config);
        self.pmxcfs.write_sdn_config(&config).await
    }

    /// Remove a vnet configuration
    pub async fn remove_vnet(&self, vnet_name: &str) -> anyhow::Result<()> {
        self.pmxcfs
            .with_lock("sdn_config", &format!("remove_vnet_{}", vnet_name), || {
                Ok(())
            })
            .await?;

        let mut config = self.read_config().await?;
        config.vnets.remove(vnet_name);
        self.pmxcfs.write_sdn_config(&config).await
    }

    /// Verify cluster synchronization
    pub async fn verify_sync(&self) -> anyhow::Result<bool> {
        self.pmxcfs.verify_cluster_sync("sdn").await
    }

    /// Get cluster nodes
    pub async fn get_cluster_nodes(&self) -> anyhow::Result<Vec<String>> {
        self.pmxcfs.get_cluster_nodes().await
    }
}

impl Default for SdnConfigManager {
    fn default() -> Self {
        Self::new().expect("Failed to create default SdnConfigManager")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_sdn_config_serialization() {
        let zone_config = ZoneConfig {
            zone_type: ZoneType::Vlan,
            bridge: Some("vmbr0".to_string()),
            vlan_aware: Some(true),
            tag: Some(100),
            vxlan_port: None,
            peers: None,
            mtu: Some(1500),
            nodes: None,
            options: HashMap::new(),
        };

        let json = serde_json::to_string(&zone_config).unwrap();
        let deserialized: ZoneConfig = serde_json::from_str(&json).unwrap();

        assert!(matches!(deserialized.zone_type, ZoneType::Vlan));
        assert_eq!(deserialized.bridge, Some("vmbr0".to_string()));
        assert_eq!(deserialized.vlan_aware, Some(true));
    }

    #[tokio::test]
    async fn test_vnet_config_serialization() {
        let vnet_config = VNetConfig {
            zone: "test_zone".to_string(),
            tag: Some(200),
            alias: Some("test_vnet".to_string()),
            vlanaware: Some(false),
            mac: None,
        };

        let json = serde_json::to_string(&vnet_config).unwrap();
        let deserialized: VNetConfig = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.zone, "test_zone");
        assert_eq!(deserialized.tag, Some(200));
        assert_eq!(deserialized.alias, Some("test_vnet".to_string()));
    }

    #[tokio::test]
    async fn test_sdn_config_manager() {
        let temp_dir = TempDir::new().unwrap();
        let pmxcfs = crate::pmxcfs::PmxcfsConfig::with_base_path(temp_dir.path()).unwrap();
        let manager = SdnConfigManager::with_pmxcfs(pmxcfs);

        let zone_config = ZoneConfig {
            zone_type: ZoneType::Simple,
            bridge: Some("vmbr0".to_string()),
            vlan_aware: None,
            tag: None,
            vxlan_port: None,
            peers: None,
            mtu: None,
            nodes: None,
            options: HashMap::new(),
        };

        let result = manager.update_zone("test_zone", zone_config).await;
        assert!(result.is_ok());

        let config = manager.read_config().await.unwrap();
        assert!(config.zones.contains_key("test_zone"));
    }
}
