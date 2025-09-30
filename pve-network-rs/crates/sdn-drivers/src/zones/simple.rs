//! Simple zone driver

use anyhow::Result;
use async_trait::async_trait;
use pve_sdn_core::{Zone, ZoneConfig, ZoneType};
use std::collections::HashMap;

/// Simple zone implementation
///
/// Simple zones provide basic SDN functionality without VLAN tagging.
/// They use a single bridge for all VNets in the zone.
pub struct SimpleZone {
    name: String,
}

impl SimpleZone {
    /// Create new simple zone
    pub fn new(name: String) -> Self {
        Self { name }
    }
}

#[async_trait]
impl Zone for SimpleZone {
    fn zone_type(&self) -> ZoneType {
        ZoneType::Simple
    }

    fn name(&self) -> &str {
        &self.name
    }

    async fn validate_config(&self, config: &ZoneConfig) -> Result<()> {
        // Basic validation
        config.validate()?;

        // Simple zone specific validation
        if config.zone_type != ZoneType::Simple {
            anyhow::bail!(
                "Zone type mismatch: expected Simple, got {}",
                config.zone_type
            );
        }

        // Simple zones should have a bridge
        if config.bridge.is_none() {
            anyhow::bail!("Simple zone requires a bridge configuration");
        }

        // Simple zones don't use VLAN tags
        if config.tag.is_some() {
            anyhow::bail!("Simple zones do not support VLAN tags");
        }

        // Simple zones don't use VXLAN
        if config.vxlan_port.is_some() {
            anyhow::bail!("Simple zones do not support VXLAN");
        }

        Ok(())
    }

    async fn apply_config(&self, config: &ZoneConfig) -> Result<()> {
        self.validate_config(config).await?;

        log::info!("Applying Simple zone configuration for '{}'", config.zone);

        // In a real implementation, this would:
        // 1. Ensure the bridge exists
        // 2. Configure bridge parameters
        // 3. Set up any required firewall rules

        Ok(())
    }

    async fn generate_config(&self, config: &ZoneConfig) -> Result<HashMap<String, String>> {
        self.validate_config(config).await?;

        let mut configs = HashMap::new();

        // Generate bridge configuration
        if let Some(bridge) = &config.bridge {
            let mut bridge_config = String::new();
            bridge_config.push_str(&format!("auto {}\n", bridge));
            bridge_config.push_str(&format!("iface {} inet manual\n", bridge));
            bridge_config.push_str(&format!("    bridge_ports none\n"));
            bridge_config.push_str(&format!("    bridge_stp off\n"));
            bridge_config.push_str(&format!("    bridge_fd 0\n"));

            if let Some(mtu) = config.mtu {
                bridge_config.push_str(&format!("    mtu {}\n", mtu));
            }

            configs.insert(format!("bridge_{}", bridge), bridge_config);
        }

        Ok(configs)
    }
}
