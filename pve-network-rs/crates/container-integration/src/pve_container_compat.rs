//! pve-container compatibility layer

use async_trait::async_trait;
use log::{error, info, warn};
use serde_json::Value;
use std::collections::HashMap;
use std::path::Path;

use crate::error::{ContainerError, Result};
use crate::types::{
    ContainerId, ContainerNetworkConfig, ContainerNetworkConfigExt, ContainerNetworkInterface,
    ContainerNetworkInterfaceExt,
};

/// pve-container compatibility layer
pub struct PveContainerCompat {
    /// Configuration cache
    config_cache: std::sync::Arc<tokio::sync::RwLock<HashMap<ContainerId, ContainerNetworkConfig>>>,
}

impl PveContainerCompat {
    /// Create new pve-container compatibility layer
    pub fn new() -> Self {
        Self {
            config_cache: std::sync::Arc::new(tokio::sync::RwLock::new(HashMap::new())),
        }
    }

    /// Parse pve-container network configuration
    pub async fn parse_container_config(
        &self,
        container_id: ContainerId,
        config_content: &str,
    ) -> Result<ContainerNetworkConfig> {
        info!("Parsing container {} network configuration", container_id);

        let mut network_config = ContainerNetworkConfig::new(container_id);

        // Parse configuration line by line
        for line in config_content.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }

            if let Some((key, value)) = line.split_once(':') {
                let key = key.trim();
                let value = value.trim();

                match key {
                    "hostname" => {
                        network_config.hostname = Some(value.to_string());
                    }
                    "nameserver" => {
                        network_config.nameservers.push(value.to_string());
                    }
                    "searchdomain" => {
                        network_config.search_domains.push(value.to_string());
                    }
                    key if key.starts_with("net") => {
                        if let Ok(interface) = self.parse_network_interface(key, value).await {
                            network_config.add_interface(interface)?;
                        }
                    }
                    _ => {
                        // Ignore other configuration options
                    }
                }
            }
        }

        // Cache the configuration
        {
            let mut cache = self.config_cache.write().await;
            cache.insert(container_id, network_config.clone());
        }

        info!(
            "Parsed {} network interfaces for container {}",
            network_config.interfaces.len(),
            container_id
        );

        Ok(network_config)
    }

    /// Generate pve-container compatible network configuration
    pub async fn generate_container_config(
        &self,
        config: &ContainerNetworkConfig,
    ) -> Result<String> {
        info!(
            "Generating container {} network configuration",
            config.container_id
        );

        let mut lines = Vec::new();

        // Add hostname
        if let Some(hostname) = &config.hostname {
            lines.push(format!("hostname: {}", hostname));
        }

        // Add nameservers
        for nameserver in &config.nameservers {
            lines.push(format!("nameserver: {}", nameserver));
        }

        // Add search domains
        for domain in &config.search_domains {
            lines.push(format!("searchdomain: {}", domain));
        }

        // Add network interfaces
        for interface in config.interfaces.values() {
            let interface_line = self.generate_interface_config(interface).await?;
            lines.push(interface_line);
        }

        Ok(lines.join("\n"))
    }

    /// Read container configuration from file
    pub async fn read_container_config(
        &self,
        container_id: ContainerId,
    ) -> Result<ContainerNetworkConfig> {
        let config_path = format!("/etc/pve/lxc/{}.conf", container_id);

        match tokio::fs::read_to_string(&config_path).await {
            Ok(content) => self.parse_container_config(container_id, &content).await,
            Err(e) => {
                error!("Failed to read container {} config: {}", container_id, e);
                Err(ContainerError::System { source: e })
            }
        }
    }

    /// Write container configuration to file
    pub async fn write_container_config(&self, config: &ContainerNetworkConfig) -> Result<()> {
        let config_path = format!("/etc/pve/lxc/{}.conf", config.container_id);
        let config_content = self.generate_container_config(config).await?;

        // Create backup
        let backup_path = format!("{}.backup", config_path);
        if Path::new(&config_path).exists() {
            if let Err(e) = tokio::fs::copy(&config_path, &backup_path).await {
                warn!("Failed to create backup of container config: {}", e);
            }
        }

        // Write new configuration
        match tokio::fs::write(&config_path, config_content).await {
            Ok(()) => {
                info!("Updated container {} configuration", config.container_id);
                Ok(())
            }
            Err(e) => {
                error!(
                    "Failed to write container {} config: {}",
                    config.container_id, e
                );
                Err(ContainerError::System { source: e })
            }
        }
    }

    /// Update network interface in container configuration
    pub async fn update_interface(
        &self,
        container_id: ContainerId,
        interface: ContainerNetworkInterface,
    ) -> Result<()> {
        info!(
            "Updating interface '{}' for container {}",
            interface.name, container_id
        );

        // Read current configuration
        let mut config = self.read_container_config(container_id).await?;

        // Update interface
        config.add_interface(interface)?;

        // Write updated configuration
        self.write_container_config(&config).await?;

        Ok(())
    }

    /// Remove network interface from container configuration
    pub async fn remove_interface(
        &self,
        container_id: ContainerId,
        interface_name: &str,
    ) -> Result<()> {
        info!(
            "Removing interface '{}' from container {}",
            interface_name, container_id
        );

        // Read current configuration
        let mut config = self.read_container_config(container_id).await?;

        // Remove interface
        if config.remove_interface(interface_name).is_none() {
            return Err(ContainerError::InterfaceNotFound {
                container_id,
                interface: interface_name.to_string(),
            });
        }

        // Write updated configuration
        self.write_container_config(&config).await?;

        Ok(())
    }

    /// Get cached configuration
    pub async fn get_cached_config(
        &self,
        container_id: ContainerId,
    ) -> Option<ContainerNetworkConfig> {
        let cache = self.config_cache.read().await;
        cache.get(&container_id).cloned()
    }

    /// Clear configuration cache
    pub async fn clear_cache(&self, container_id: Option<ContainerId>) {
        let mut cache = self.config_cache.write().await;

        if let Some(id) = container_id {
            cache.remove(&id);
            info!("Cleared cache for container {}", id);
        } else {
            cache.clear();
            info!("Cleared all container configuration cache");
        }
    }

    /// Parse network interface from pve-container format
    async fn parse_network_interface(
        &self,
        interface_name: &str,
        config_value: &str,
    ) -> Result<ContainerNetworkInterface> {
        let mut interface = ContainerNetworkInterface::new(interface_name.to_string());

        // Parse comma-separated key=value pairs
        for part in config_value.split(',') {
            let part = part.trim();

            if let Some((key, value)) = part.split_once('=') {
                let key = key.trim();
                let value = value.trim();

                match key {
                    "name" => interface.vnet = Some(value.to_string()),
                    "bridge" => interface.bridge = Some(value.to_string()),
                    "hwaddr" => interface.hwaddr = Some(value.to_string()),
                    "ip" => interface.ip = Some(value.to_string()),
                    "ip6" => interface.ip6 = Some(value.to_string()),
                    "gw" => interface.gw = Some(value.to_string()),
                    "gw6" => interface.gw6 = Some(value.to_string()),
                    "tag" => {
                        interface.tag = value.parse().ok();
                    }
                    "trunks" => interface.trunks = Some(value.to_string()),
                    "firewall" => {
                        interface.firewall = match value {
                            "1" | "true" | "yes" | "on" => Some(true),
                            "0" | "false" | "no" | "off" => Some(false),
                            _ => None,
                        };
                    }
                    "link_down" => {
                        interface.link_down = match value {
                            "1" | "true" | "yes" | "on" => Some(true),
                            "0" | "false" | "no" | "off" => Some(false),
                            _ => None,
                        };
                    }
                    "mtu" => {
                        interface.mtu = value.parse().ok();
                    }
                    "rate" => {
                        interface.rate = value.parse().ok();
                    }
                    _ => {
                        interface.options.insert(key.to_string(), value.to_string());
                    }
                }
            } else if !part.is_empty() {
                // Handle boolean flags or single values
                match part {
                    "firewall" => interface.firewall = Some(true),
                    "link_down" => interface.link_down = Some(true),
                    _ => {
                        // Try to parse as bridge name (legacy format)
                        if interface.bridge.is_none() && interface.vnet.is_none() {
                            interface.bridge = Some(part.to_string());
                        }
                    }
                }
            }
        }

        Ok(interface)
    }

    /// Generate interface configuration in pve-container format
    async fn generate_interface_config(
        &self,
        interface: &ContainerNetworkInterface,
    ) -> Result<String> {
        let mut parts = Vec::new();

        // Add network backend (VNet or bridge)
        if let Some(vnet) = &interface.vnet {
            parts.push(format!("name={}", vnet));
        } else if let Some(bridge) = &interface.bridge {
            parts.push(format!("bridge={}", bridge));
        }

        // Add MAC address
        if let Some(hwaddr) = &interface.hwaddr {
            parts.push(format!("hwaddr={}", hwaddr));
        }

        // Add IP configuration
        if let Some(ip) = &interface.ip {
            parts.push(format!("ip={}", ip));
        }

        if let Some(ip6) = &interface.ip6 {
            parts.push(format!("ip6={}", ip6));
        }

        if let Some(gw) = &interface.gw {
            parts.push(format!("gw={}", gw));
        }

        if let Some(gw6) = &interface.gw6 {
            parts.push(format!("gw6={}", gw6));
        }

        // Add VLAN tag
        if let Some(tag) = interface.tag {
            parts.push(format!("tag={}", tag));
        }

        // Add trunks
        if let Some(trunks) = &interface.trunks {
            parts.push(format!("trunks={}", trunks));
        }

        // Add boolean options
        if let Some(true) = interface.firewall {
            parts.push("firewall=1".to_string());
        }

        if let Some(true) = interface.link_down {
            parts.push("link_down=1".to_string());
        }

        // Add MTU
        if let Some(mtu) = interface.mtu {
            parts.push(format!("mtu={}", mtu));
        }

        // Add rate limiting
        if let Some(rate) = interface.rate {
            parts.push(format!("rate={}", rate));
        }

        // Add additional options
        for (key, value) in &interface.options {
            parts.push(format!("{}={}", key, value));
        }

        Ok(format!("{}: {}", interface.name, parts.join(",")))
    }
}

impl Default for PveContainerCompat {
    fn default() -> Self {
        Self::new()
    }
}

/// pve-container compatibility trait
#[async_trait]
pub trait PveContainerCompatTrait {
    /// Parse container configuration
    async fn parse_container_config(
        &self,
        container_id: ContainerId,
        config_content: &str,
    ) -> Result<ContainerNetworkConfig>;

    /// Generate container configuration
    async fn generate_container_config(&self, config: &ContainerNetworkConfig) -> Result<String>;

    /// Update interface in container
    async fn update_interface(
        &self,
        container_id: ContainerId,
        interface: ContainerNetworkInterface,
    ) -> Result<()>;

    /// Remove interface from container
    async fn remove_interface(&self, container_id: ContainerId, interface_name: &str)
        -> Result<()>;
}

#[async_trait]
impl PveContainerCompatTrait for PveContainerCompat {
    async fn parse_container_config(
        &self,
        container_id: ContainerId,
        config_content: &str,
    ) -> Result<ContainerNetworkConfig> {
        self.parse_container_config(container_id, config_content)
            .await
    }

    async fn generate_container_config(&self, config: &ContainerNetworkConfig) -> Result<String> {
        self.generate_container_config(config).await
    }

    async fn update_interface(
        &self,
        container_id: ContainerId,
        interface: ContainerNetworkInterface,
    ) -> Result<()> {
        self.update_interface(container_id, interface).await
    }

    async fn remove_interface(
        &self,
        container_id: ContainerId,
        interface_name: &str,
    ) -> Result<()> {
        self.remove_interface(container_id, interface_name).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_parse_container_config() {
        let compat = PveContainerCompat::new();
        let container_id = 100;

        let config_content = r#"
hostname: test-container
nameserver: 8.8.8.8
net0: bridge=vmbr0,hwaddr=02:00:00:00:00:01,ip=192.168.1.10/24,gw=192.168.1.1
net1: name=vnet1,hwaddr=02:00:00:00:00:02,ip=10.0.0.10/24
"#;

        let config = compat
            .parse_container_config(container_id, config_content)
            .await
            .unwrap();

        assert_eq!(config.container_id, container_id);
        assert_eq!(config.hostname, Some("test-container".to_string()));
        assert_eq!(config.nameservers, vec!["8.8.8.8"]);
        assert_eq!(config.interfaces.len(), 2);

        let net0 = config.get_interface("net0").unwrap();
        assert_eq!(net0.bridge, Some("vmbr0".to_string()));
        assert_eq!(net0.ip, Some("192.168.1.10/24".to_string()));

        let net1 = config.get_interface("net1").unwrap();
        assert_eq!(net1.vnet, Some("vnet1".to_string()));
        assert_eq!(net1.ip, Some("10.0.0.10/24".to_string()));
    }

    #[tokio::test]
    async fn test_generate_container_config() {
        let compat = PveContainerCompat::new();
        let mut config = ContainerNetworkConfig::new(100);
        config.hostname = Some("test-container".to_string());
        config.nameservers.push("8.8.8.8".to_string());

        let mut interface = ContainerNetworkInterface::new("net0".to_string());
        interface.bridge = Some("vmbr0".to_string());
        interface.ip = Some("192.168.1.10/24".to_string());
        interface.hwaddr = Some("02:00:00:00:00:01".to_string());

        config.add_interface(interface).unwrap();

        let generated = compat.generate_container_config(&config).await.unwrap();

        assert!(generated.contains("hostname: test-container"));
        assert!(generated.contains("nameserver: 8.8.8.8"));
        assert!(generated.contains("net0: bridge=vmbr0"));
        assert!(generated.contains("ip=192.168.1.10/24"));
    }

    #[tokio::test]
    async fn test_interface_parsing() {
        let compat = PveContainerCompat::new();

        let interface = compat.parse_network_interface(
            "net0",
            "bridge=vmbr0,hwaddr=02:00:00:00:00:01,ip=192.168.1.10/24,gw=192.168.1.1,firewall=1"
        ).await.unwrap();

        assert_eq!(interface.name, "net0");
        assert_eq!(interface.bridge, Some("vmbr0".to_string()));
        assert_eq!(interface.hwaddr, Some("02:00:00:00:00:01".to_string()));
        assert_eq!(interface.ip, Some("192.168.1.10/24".to_string()));
        assert_eq!(interface.gw, Some("192.168.1.1".to_string()));
        assert_eq!(interface.firewall, Some(true));
    }
}
