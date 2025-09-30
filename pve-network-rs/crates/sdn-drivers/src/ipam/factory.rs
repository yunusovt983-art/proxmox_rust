//! IPAM plugin factory
//!
//! Creates IPAM plugins with Perl compatibility

use anyhow::Result;
use std::sync::Arc;

use super::{NetBoxIpam, PhpIpam, PveIpam};
use pve_sdn_core::{IpamConfig, IpamPlugin, IpamType};

/// IPAM plugin factory
pub struct IpamPluginFactory;

impl IpamPluginFactory {
    /// Create IPAM plugin from configuration
    pub fn create_plugin(config: &IpamConfig) -> Result<Arc<dyn IpamPlugin>> {
        match config.ipam_type {
            IpamType::Pve => {
                let plugin = PveIpam::new(config.name.clone(), config.clone());
                let plugin_arc = Arc::new(plugin);

                // Load existing data from storage for Perl compatibility
                // Skip async loading during tests to avoid race conditions
                if std::env::var("PVE_IPAM_STORAGE_PATH").is_err() {
                    let plugin_clone = plugin_arc.clone();
                    tokio::spawn(async move {
                        if let Err(e) = plugin_clone.load_from_storage().await {
                            log::warn!("Failed to load PVE IPAM data: {}", e);
                        }
                    });
                }

                Ok(plugin_arc)
            }
            IpamType::PhpIpam => {
                let plugin = PhpIpam::new(config.name.clone(), config.clone())?;
                Ok(Arc::new(plugin))
            }
            IpamType::NetBox => {
                let plugin = NetBoxIpam::new(config.name.clone(), config.clone())?;
                Ok(Arc::new(plugin))
            }
        }
    }

    /// Create and validate plugin
    pub async fn create_and_validate_plugin(config: &IpamConfig) -> Result<Arc<dyn IpamPlugin>> {
        let plugin = Self::create_plugin(config)?;

        // Validate plugin configuration
        plugin.validate_config(config).await?;

        Ok(plugin)
    }

    /// Initialize IPAM manager with plugins from configuration
    pub async fn initialize_manager(
        ipam_configs: &std::collections::HashMap<String, IpamConfig>,
        default_plugin: Option<&str>,
    ) -> Result<pve_sdn_core::IpamManager> {
        let mut manager = pve_sdn_core::IpamManager::new();

        // Create and register all plugins
        for (name, config) in ipam_configs {
            match Self::create_and_validate_plugin(config).await {
                Ok(plugin) => {
                    manager.register_plugin(plugin);
                    log::info!(
                        "Registered IPAM plugin: {} (type: {:?})",
                        name,
                        config.ipam_type
                    );
                }
                Err(e) => {
                    log::error!("Failed to create IPAM plugin '{}': {}", name, e);
                    return Err(e);
                }
            }
        }

        // Set default plugin if specified
        if let Some(default_name) = default_plugin {
            manager.set_default_plugin(default_name)?;
            log::info!("Set default IPAM plugin: {}", default_name);
        } else if ipam_configs.len() == 1 {
            // If only one plugin, make it default
            let plugin_name = ipam_configs.keys().next().unwrap();
            manager.set_default_plugin(plugin_name)?;
            log::info!("Set default IPAM plugin (auto): {}", plugin_name);
        }

        Ok(manager)
    }
}

/// Perl compatibility helpers
pub mod perl_compat {
    use super::*;
    use pve_sdn_core::{IpAllocation, IpAllocationRequest};
    use serde_json::Value;
    use std::collections::HashMap;

    /// Convert Perl-style IPAM request to Rust format
    pub fn convert_perl_request(
        perl_data: &HashMap<String, Value>,
        subnet: &str,
    ) -> Result<IpAllocationRequest> {
        let vmid = perl_data
            .get("vmid")
            .and_then(|v| v.as_u64())
            .map(|v| v as u32);

        let hostname = perl_data
            .get("hostname")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        let mac = perl_data
            .get("mac")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        let description = perl_data
            .get("description")
            .or_else(|| perl_data.get("desc"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        let requested_ip = perl_data
            .get("ip")
            .and_then(|v| v.as_str())
            .and_then(|s| s.parse().ok());

        Ok(IpAllocationRequest {
            subnet: subnet.to_string(),
            vmid,
            hostname,
            mac,
            description,
            requested_ip,
        })
    }

    /// Convert Rust allocation to Perl-compatible format
    pub fn convert_to_perl_format(allocation: &IpAllocation) -> HashMap<String, Value> {
        let mut result = HashMap::new();

        result.insert("ip".to_string(), Value::String(allocation.ip.to_string()));
        result.insert(
            "subnet".to_string(),
            Value::String(allocation.subnet.clone()),
        );

        if let Some(vmid) = allocation.vmid {
            result.insert("vmid".to_string(), Value::Number(vmid.into()));
        }

        if let Some(hostname) = &allocation.hostname {
            result.insert("hostname".to_string(), Value::String(hostname.clone()));
        }

        if let Some(mac) = &allocation.mac {
            result.insert("mac".to_string(), Value::String(mac.clone()));
        }

        if let Some(description) = &allocation.description {
            result.insert(
                "description".to_string(),
                Value::String(description.clone()),
            );
        }

        result.insert(
            "allocated_at".to_string(),
            Value::String(allocation.allocated_at.to_rfc3339()),
        );

        result
    }

    /// Validate IPAM configuration for Perl compatibility
    pub fn validate_perl_compatibility(config: &IpamConfig) -> Result<()> {
        match config.ipam_type {
            IpamType::Pve => {
                // PVE IPAM should be compatible by default
                Ok(())
            }
            IpamType::PhpIpam => {
                // Ensure required fields for phpIPAM
                if config.url.is_none() {
                    anyhow::bail!("phpIPAM requires 'url' configuration");
                }

                if config.token.is_none()
                    && (config.username.is_none() || config.password.is_none())
                {
                    anyhow::bail!("phpIPAM requires either 'token' or 'username'/'password'");
                }

                if config.section.is_none() {
                    log::warn!("phpIPAM section not specified, using default");
                }

                Ok(())
            }
            IpamType::NetBox => {
                // Ensure required fields for NetBox
                if config.url.is_none() {
                    anyhow::bail!("NetBox requires 'url' configuration");
                }

                if config.token.is_none() {
                    anyhow::bail!("NetBox requires 'token' configuration");
                }

                Ok(())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pve_sdn_core::{IpamConfig, IpamType};

    #[tokio::test]
    async fn test_create_pve_plugin() {
        let config = IpamConfig::new("test-pve".to_string(), IpamType::Pve);
        let plugin = IpamPluginFactory::create_plugin(&config).unwrap();

        assert_eq!(plugin.name(), "test-pve");
        assert_eq!(plugin.plugin_type(), IpamType::Pve);
    }

    #[test]
    fn test_perl_compat_validation() {
        let pve_config = IpamConfig::new("pve".to_string(), IpamType::Pve);
        assert!(perl_compat::validate_perl_compatibility(&pve_config).is_ok());

        let mut phpipam_config = IpamConfig::new("phpipam".to_string(), IpamType::PhpIpam);
        assert!(perl_compat::validate_perl_compatibility(&phpipam_config).is_err());

        phpipam_config.url = Some("http://phpipam.example.com".to_string());
        phpipam_config.token = Some("test-token".to_string());
        assert!(perl_compat::validate_perl_compatibility(&phpipam_config).is_ok());
    }

    #[test]
    fn test_perl_request_conversion() {
        let mut perl_data = std::collections::HashMap::new();
        perl_data.insert("vmid".to_string(), serde_json::Value::Number(100.into()));
        perl_data.insert(
            "hostname".to_string(),
            serde_json::Value::String("test-host".to_string()),
        );
        perl_data.insert(
            "mac".to_string(),
            serde_json::Value::String("00:11:22:33:44:55".to_string()),
        );
        perl_data.insert(
            "ip".to_string(),
            serde_json::Value::String("192.168.1.100".to_string()),
        );

        let request = perl_compat::convert_perl_request(&perl_data, "test-subnet").unwrap();

        assert_eq!(request.subnet, "test-subnet");
        assert_eq!(request.vmid, Some(100));
        assert_eq!(request.hostname, Some("test-host".to_string()));
        assert_eq!(request.mac, Some("00:11:22:33:44:55".to_string()));
        assert_eq!(request.requested_ip, Some("192.168.1.100".parse().unwrap()));
    }
}
