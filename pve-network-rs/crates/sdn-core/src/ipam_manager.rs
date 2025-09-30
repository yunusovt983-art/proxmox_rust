//! IPAM Manager
//!
//! Manages multiple IPAM plugins and provides a unified interface

use anyhow::Result;
use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::Arc;

use crate::{
    IpAllocation, IpAllocationRequest, IpamConfig, IpamError, IpamPlugin, IpamType, Subnet,
};

/// IPAM Manager
///
/// Manages multiple IPAM plugins and routes requests to the appropriate plugin
pub struct IpamManager {
    plugins: HashMap<String, Arc<dyn IpamPlugin>>,
    default_plugin: Option<String>,
}

impl IpamManager {
    /// Create new IPAM manager
    pub fn new() -> Self {
        Self {
            plugins: HashMap::new(),
            default_plugin: None,
        }
    }

    /// Register an IPAM plugin
    pub fn register_plugin(&mut self, plugin: Arc<dyn IpamPlugin>) {
        let name = plugin.name().to_string();
        log::info!(
            "Registering IPAM plugin: {} (type: {:?})",
            name,
            plugin.plugin_type()
        );
        self.plugins.insert(name, plugin);
    }

    /// Set default IPAM plugin
    pub fn set_default_plugin(&mut self, name: &str) -> Result<()> {
        if !self.plugins.contains_key(name) {
            anyhow::bail!("IPAM plugin '{}' not found", name);
        }
        self.default_plugin = Some(name.to_string());
        Ok(())
    }

    /// Get IPAM plugin by name
    pub fn get_plugin(&self, name: &str) -> Result<Arc<dyn IpamPlugin>> {
        self.plugins.get(name).cloned().ok_or_else(|| {
            IpamError::Configuration {
                message: format!("IPAM plugin '{}' not found", name),
            }
            .into()
        })
    }

    /// Get default IPAM plugin
    pub fn get_default_plugin(&self) -> Result<Arc<dyn IpamPlugin>> {
        let default_name =
            self.default_plugin
                .as_ref()
                .ok_or_else(|| IpamError::Configuration {
                    message: "No default IPAM plugin configured".to_string(),
                })?;

        self.get_plugin(default_name)
    }

    /// List all registered plugins
    pub fn list_plugins(&self) -> Vec<(String, IpamType)> {
        self.plugins
            .iter()
            .map(|(name, plugin)| (name.clone(), plugin.plugin_type()))
            .collect()
    }

    /// Allocate IP using specified or default plugin
    pub async fn allocate_ip(
        &self,
        plugin_name: Option<&str>,
        request: &IpAllocationRequest,
    ) -> Result<IpAllocation> {
        let plugin = if let Some(name) = plugin_name {
            self.get_plugin(name)?
        } else {
            self.get_default_plugin()?
        };

        plugin.allocate_ip(request).await
    }

    /// Release IP using specified or default plugin
    pub async fn release_ip(
        &self,
        plugin_name: Option<&str>,
        subnet: &str,
        ip: &IpAddr,
    ) -> Result<()> {
        let plugin = if let Some(name) = plugin_name {
            self.get_plugin(name)?
        } else {
            self.get_default_plugin()?
        };

        plugin.release_ip(subnet, ip).await
    }

    /// Update IP using specified or default plugin
    pub async fn update_ip(
        &self,
        plugin_name: Option<&str>,
        subnet: &str,
        ip: &IpAddr,
        allocation: &IpAllocation,
    ) -> Result<()> {
        let plugin = if let Some(name) = plugin_name {
            self.get_plugin(name)?
        } else {
            self.get_default_plugin()?
        };

        plugin.update_ip(subnet, ip, allocation).await
    }

    /// Get IP allocation using specified or default plugin
    pub async fn get_ip(
        &self,
        plugin_name: Option<&str>,
        subnet: &str,
        ip: &IpAddr,
    ) -> Result<Option<IpAllocation>> {
        let plugin = if let Some(name) = plugin_name {
            self.get_plugin(name)?
        } else {
            self.get_default_plugin()?
        };

        plugin.get_ip(subnet, ip).await
    }

    /// List subnet IPs using specified or default plugin
    pub async fn list_subnet_ips(
        &self,
        plugin_name: Option<&str>,
        subnet: &str,
    ) -> Result<Vec<IpAllocation>> {
        let plugin = if let Some(name) = plugin_name {
            self.get_plugin(name)?
        } else {
            self.get_default_plugin()?
        };

        plugin.list_subnet_ips(subnet).await
    }

    /// Add subnet to specified or default plugin
    pub async fn add_subnet(&self, plugin_name: Option<&str>, subnet: &Subnet) -> Result<()> {
        let plugin = if let Some(name) = plugin_name {
            self.get_plugin(name)?
        } else {
            self.get_default_plugin()?
        };

        plugin.add_subnet(subnet).await
    }

    /// Remove subnet from specified or default plugin
    pub async fn remove_subnet(&self, plugin_name: Option<&str>, subnet_name: &str) -> Result<()> {
        let plugin = if let Some(name) = plugin_name {
            self.get_plugin(name)?
        } else {
            self.get_default_plugin()?
        };

        plugin.remove_subnet(subnet_name).await
    }

    /// Get next free IP using specified or default plugin
    pub async fn get_next_free_ip(
        &self,
        plugin_name: Option<&str>,
        subnet: &str,
    ) -> Result<Option<IpAddr>> {
        let plugin = if let Some(name) = plugin_name {
            self.get_plugin(name)?
        } else {
            self.get_default_plugin()?
        };

        plugin.get_next_free_ip(subnet).await
    }

    /// Check if IP is available using specified or default plugin
    pub async fn is_ip_available(
        &self,
        plugin_name: Option<&str>,
        subnet: &str,
        ip: &IpAddr,
    ) -> Result<bool> {
        let plugin = if let Some(name) = plugin_name {
            self.get_plugin(name)?
        } else {
            self.get_default_plugin()?
        };

        plugin.is_ip_available(subnet, ip).await
    }

    /// Validate all plugin configurations
    pub async fn validate_all_configs(&self, configs: &HashMap<String, IpamConfig>) -> Result<()> {
        for (name, plugin) in &self.plugins {
            if let Some(config) = configs.get(name) {
                plugin
                    .validate_config(config)
                    .await
                    .map_err(|e| anyhow::anyhow!("Plugin '{}' validation failed: {}", name, e))?;
            }
        }
        Ok(())
    }
}

impl Default for IpamManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{IpamPlugin, IpamType};
    use async_trait::async_trait;
    use std::sync::Arc;

    struct MockIpamPlugin {
        name: String,
        plugin_type: IpamType,
    }

    impl MockIpamPlugin {
        fn new(name: String, plugin_type: IpamType) -> Self {
            Self { name, plugin_type }
        }
    }

    #[async_trait]
    impl IpamPlugin for MockIpamPlugin {
        fn plugin_type(&self) -> IpamType {
            self.plugin_type.clone()
        }

        fn name(&self) -> &str {
            &self.name
        }

        async fn validate_config(&self, _config: &IpamConfig) -> Result<()> {
            Ok(())
        }

        async fn allocate_ip(&self, _request: &IpAllocationRequest) -> Result<IpAllocation> {
            unimplemented!()
        }

        async fn release_ip(&self, _subnet: &str, _ip: &IpAddr) -> Result<()> {
            unimplemented!()
        }

        async fn update_ip(
            &self,
            _subnet: &str,
            _ip: &IpAddr,
            _allocation: &IpAllocation,
        ) -> Result<()> {
            unimplemented!()
        }

        async fn get_ip(&self, _subnet: &str, _ip: &IpAddr) -> Result<Option<IpAllocation>> {
            unimplemented!()
        }

        async fn list_subnet_ips(&self, _subnet: &str) -> Result<Vec<IpAllocation>> {
            unimplemented!()
        }

        async fn validate_subnet(&self, _subnet: &Subnet) -> Result<()> {
            unimplemented!()
        }

        async fn add_subnet(&self, _subnet: &Subnet) -> Result<()> {
            unimplemented!()
        }

        async fn remove_subnet(&self, _subnet_name: &str) -> Result<()> {
            unimplemented!()
        }

        async fn get_next_free_ip(&self, _subnet: &str) -> Result<Option<IpAddr>> {
            unimplemented!()
        }

        async fn is_ip_available(&self, _subnet: &str, _ip: &IpAddr) -> Result<bool> {
            unimplemented!()
        }
    }

    #[tokio::test]
    async fn test_plugin_registration() {
        let mut manager = IpamManager::new();

        let plugin = Arc::new(MockIpamPlugin::new("test-pve".to_string(), IpamType::Pve));
        manager.register_plugin(plugin.clone());

        let retrieved = manager.get_plugin("test-pve").unwrap();
        assert_eq!(retrieved.name(), "test-pve");
        assert_eq!(retrieved.plugin_type(), IpamType::Pve);
    }

    #[tokio::test]
    async fn test_default_plugin() {
        let mut manager = IpamManager::new();

        let plugin = Arc::new(MockIpamPlugin::new("test-pve".to_string(), IpamType::Pve));
        manager.register_plugin(plugin.clone());
        manager.set_default_plugin("test-pve").unwrap();

        let default = manager.get_default_plugin().unwrap();
        assert_eq!(default.name(), "test-pve");
    }

    #[test]
    fn test_list_plugins() {
        let mut manager = IpamManager::new();

        let pve_plugin = Arc::new(MockIpamPlugin::new("pve".to_string(), IpamType::Pve));
        let netbox_plugin = Arc::new(MockIpamPlugin::new("netbox".to_string(), IpamType::NetBox));

        manager.register_plugin(pve_plugin);
        manager.register_plugin(netbox_plugin);

        let plugins = manager.list_plugins();
        assert_eq!(plugins.len(), 2);

        let plugin_names: Vec<String> = plugins.iter().map(|(name, _)| name.clone()).collect();
        assert!(plugin_names.contains(&"pve".to_string()));
        assert!(plugin_names.contains(&"netbox".to_string()));
    }
}
