//! QinQ zone driver
//!
//! QinQ (802.1ad) zones provide double VLAN tagging for service provider networks.
//! This allows for customer VLAN tags to be preserved while adding a service provider tag.

use anyhow::{Context, Result};
use async_trait::async_trait;
use log::{debug, info, warn};
use pve_sdn_core::{Zone, ZoneConfig, ZoneType};
use std::collections::HashMap;

/// QinQ zone implementation
///
/// QinQ zones support double VLAN tagging (802.1ad) where:
/// - Service VLAN (S-VLAN) is the outer tag managed by the service provider
/// - Customer VLAN (C-VLAN) is the inner tag managed by the customer
pub struct QinQZone {
    name: String,
}

impl QinQZone {
    /// Create new QinQ zone
    pub fn new(name: String) -> Self {
        Self { name }
    }

    /// Validate QinQ-specific configuration parameters
    fn validate_qinq_config(&self, config: &ZoneConfig) -> Result<()> {
        // QinQ requires a bridge
        if config.bridge.is_none() {
            anyhow::bail!("QinQ zone '{}' requires a bridge configuration", self.name);
        }

        // Service VLAN tag is required for QinQ
        if config.tag.is_none() {
            anyhow::bail!("QinQ zone '{}' requires a service VLAN tag", self.name);
        }

        let tag = config.tag.unwrap();
        if tag == 0 || tag > 4094 {
            anyhow::bail!(
                "QinQ zone '{}' service VLAN tag must be between 1 and 4094",
                self.name
            );
        }

        // Validate bridge name
        let bridge = config.bridge.as_ref().unwrap();
        if bridge.is_empty() {
            anyhow::bail!("QinQ zone '{}' bridge name cannot be empty", self.name);
        }

        // Check for QinQ-specific options
        if let Some(vlan_protocol) = config.options.get("vlan-protocol") {
            if let Some(protocol) = vlan_protocol.as_str() {
                if protocol != "802.1ad" && protocol != "802.1q" {
                    anyhow::bail!(
                        "QinQ zone '{}' vlan-protocol must be '802.1ad' or '802.1q'",
                        self.name
                    );
                }
            }
        }

        // Validate MTU if specified
        if let Some(mtu) = config.mtu {
            if mtu < 1500 {
                warn!(
                    "QinQ zone '{}' MTU {} is less than 1500, may cause issues with double tagging",
                    self.name, mtu
                );
            }
        }

        Ok(())
    }

    /// Generate bridge configuration for QinQ
    fn generate_bridge_config(&self, config: &ZoneConfig) -> Result<String> {
        let bridge = config.bridge.as_ref().unwrap();
        let tag = config.tag.unwrap();
        let vlan_protocol = config
            .options
            .get("vlan-protocol")
            .and_then(|v| v.as_str())
            .unwrap_or("802.1ad");

        let mut bridge_config = format!(
            "auto {bridge}\n\
             iface {bridge} inet manual\n\
             \tbridge_ports none\n\
             \tbridge_stp off\n\
             \tbridge_fd 0\n\
             \tbridge_vlan_aware yes\n\
             \tbridge_vlan_protocol {vlan_protocol}\n",
            bridge = bridge,
            vlan_protocol = vlan_protocol
        );

        // Add MTU if specified
        if let Some(mtu) = config.mtu {
            bridge_config.push_str(&format!("\tmtu {}\n", mtu));
        }

        // Add service VLAN configuration
        bridge_config.push_str(&format!("\t# QinQ Service VLAN {}\n", tag));

        Ok(bridge_config)
    }

    /// Generate VLAN interface configuration for QinQ
    fn generate_vlan_config(&self, config: &ZoneConfig) -> Result<String> {
        let bridge = config.bridge.as_ref().unwrap();
        let tag = config.tag.unwrap();
        let vlan_interface = format!("{}.{}", bridge, tag);

        let mut vlan_config = format!(
            "auto {vlan_interface}\n\
             iface {vlan_interface} inet manual\n\
             \tvlan-raw-device {bridge}\n\
             \tvlan-id {tag}\n\
             \tvlan-protocol 802.1ad\n",
            vlan_interface = vlan_interface,
            bridge = bridge,
            tag = tag
        );

        // Add MTU if specified
        if let Some(mtu) = config.mtu {
            vlan_config.push_str(&format!("\tmtu {}\n", mtu));
        }

        Ok(vlan_config)
    }

    /// Generate systemd network configuration for QinQ
    fn generate_systemd_config(&self, config: &ZoneConfig) -> Result<String> {
        let bridge = config.bridge.as_ref().unwrap();
        let tag = config.tag.unwrap();

        let systemd_config = format!(
            "[NetDev]\n\
             Name={bridge}\n\
             Kind=bridge\n\
             \n\
             [Bridge]\n\
             VLANFiltering=yes\n\
             DefaultPVID={tag}\n\
             VLANProtocol=802.1ad\n",
            bridge = bridge,
            tag = tag
        );

        Ok(systemd_config)
    }
}

#[async_trait]
impl Zone for QinQZone {
    fn zone_type(&self) -> ZoneType {
        ZoneType::QinQ
    }

    fn name(&self) -> &str {
        &self.name
    }

    async fn validate_config(&self, config: &ZoneConfig) -> Result<()> {
        debug!("Validating QinQ zone '{}' configuration", self.name);

        // Basic validation
        config
            .validate()
            .with_context(|| format!("Basic validation failed for QinQ zone '{}'", self.name))?;

        // QinQ-specific validation
        self.validate_qinq_config(config)
            .with_context(|| format!("QinQ-specific validation failed for zone '{}'", self.name))?;

        info!(
            "QinQ zone '{}' configuration validation successful",
            self.name
        );
        Ok(())
    }

    async fn apply_config(&self, config: &ZoneConfig) -> Result<()> {
        debug!("Applying QinQ zone '{}' configuration", self.name);

        // Validate configuration first
        self.validate_config(config).await.with_context(|| {
            format!(
                "Configuration validation failed for QinQ zone '{}'",
                self.name
            )
        })?;

        let bridge = config.bridge.as_ref().unwrap();
        let _tag = config.tag.unwrap();

        // Check if bridge exists
        let bridge_exists = tokio::process::Command::new("ip")
            .args(&["link", "show", bridge])
            .output()
            .await
            .map(|output| output.status.success())
            .unwrap_or(false);

        if !bridge_exists {
            info!("Creating QinQ bridge '{}' for zone '{}'", bridge, self.name);

            // Create bridge
            let output = tokio::process::Command::new("ip")
                .args(&["link", "add", "name", bridge, "type", "bridge"])
                .output()
                .await
                .with_context(|| {
                    format!(
                        "Failed to create bridge '{}' for QinQ zone '{}'",
                        bridge, self.name
                    )
                })?;

            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                anyhow::bail!("Failed to create bridge '{}': {}", bridge, stderr);
            }

            // Enable VLAN filtering
            let output = tokio::process::Command::new("ip")
                .args(&[
                    "link",
                    "set",
                    bridge,
                    "type",
                    "bridge",
                    "vlan_filtering",
                    "1",
                ])
                .output()
                .await
                .with_context(|| {
                    format!("Failed to enable VLAN filtering on bridge '{}'", bridge)
                })?;

            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                anyhow::bail!(
                    "Failed to enable VLAN filtering on bridge '{}': {}",
                    bridge,
                    stderr
                );
            }

            // Set VLAN protocol to 802.1ad
            let vlan_protocol = config
                .options
                .get("vlan-protocol")
                .and_then(|v| v.as_str())
                .unwrap_or("802.1ad");

            if vlan_protocol == "802.1ad" {
                let output = tokio::process::Command::new("ip")
                    .args(&[
                        "link",
                        "set",
                        bridge,
                        "type",
                        "bridge",
                        "vlan_protocol",
                        "802.1ad",
                    ])
                    .output()
                    .await
                    .with_context(|| {
                        format!("Failed to set VLAN protocol on bridge '{}'", bridge)
                    })?;

                if !output.status.success() {
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    warn!(
                        "Failed to set VLAN protocol to 802.1ad on bridge '{}': {}",
                        bridge, stderr
                    );
                }
            }

            // Set MTU if specified
            if let Some(mtu) = config.mtu {
                let output = tokio::process::Command::new("ip")
                    .args(&["link", "set", bridge, "mtu", &mtu.to_string()])
                    .output()
                    .await
                    .with_context(|| format!("Failed to set MTU on bridge '{}'", bridge))?;

                if !output.status.success() {
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    warn!(
                        "Failed to set MTU {} on bridge '{}': {}",
                        mtu, bridge, stderr
                    );
                }
            }

            // Bring bridge up
            let output = tokio::process::Command::new("ip")
                .args(&["link", "set", bridge, "up"])
                .output()
                .await
                .with_context(|| format!("Failed to bring up bridge '{}'", bridge))?;

            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                anyhow::bail!("Failed to bring up bridge '{}': {}", bridge, stderr);
            }
        }

        info!(
            "QinQ zone '{}' configuration applied successfully",
            self.name
        );
        Ok(())
    }

    async fn generate_config(&self, config: &ZoneConfig) -> Result<HashMap<String, String>> {
        debug!(
            "Generating configuration files for QinQ zone '{}'",
            self.name
        );

        let mut configs = HashMap::new();

        // Generate bridge configuration
        let bridge_config = self.generate_bridge_config(config).with_context(|| {
            format!(
                "Failed to generate bridge config for QinQ zone '{}'",
                self.name
            )
        })?;
        configs.insert("bridge".to_string(), bridge_config);

        // Generate VLAN interface configuration
        let vlan_config = self.generate_vlan_config(config).with_context(|| {
            format!(
                "Failed to generate VLAN config for QinQ zone '{}'",
                self.name
            )
        })?;
        configs.insert("vlan".to_string(), vlan_config);

        // Generate systemd network configuration
        let systemd_config = self.generate_systemd_config(config).with_context(|| {
            format!(
                "Failed to generate systemd config for QinQ zone '{}'",
                self.name
            )
        })?;
        configs.insert("systemd".to_string(), systemd_config);

        // Generate zone-specific metadata
        let metadata = format!(
            "# QinQ Zone Configuration\n\
             # Zone: {}\n\
             # Type: QinQ (802.1ad)\n\
             # Bridge: {}\n\
             # Service VLAN: {}\n\
             # VLAN Protocol: {}\n",
            self.name,
            config.bridge.as_ref().unwrap(),
            config.tag.unwrap(),
            config
                .options
                .get("vlan-protocol")
                .and_then(|v| v.as_str())
                .unwrap_or("802.1ad")
        );
        configs.insert("metadata".to_string(), metadata);

        info!(
            "Generated configuration files for QinQ zone '{}'",
            self.name
        );
        Ok(configs)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_qinq_zone_validation() {
        let zone = QinQZone::new("test-qinq".to_string());

        // Test valid configuration
        let mut config = ZoneConfig::new(ZoneType::QinQ, "test-qinq".to_string());
        config.bridge = Some("vmbr0".to_string());
        config.tag = Some(100);

        assert!(zone.validate_config(&config).await.is_ok());

        // Test missing bridge
        config.bridge = None;
        assert!(zone.validate_config(&config).await.is_err());

        // Test missing tag
        config.bridge = Some("vmbr0".to_string());
        config.tag = None;
        assert!(zone.validate_config(&config).await.is_err());

        // Test invalid tag
        config.tag = Some(0);
        assert!(zone.validate_config(&config).await.is_err());

        config.tag = Some(5000);
        assert!(zone.validate_config(&config).await.is_err());
    }

    #[tokio::test]
    async fn test_qinq_config_generation() {
        let zone = QinQZone::new("test-qinq".to_string());

        let mut config = ZoneConfig::new(ZoneType::QinQ, "test-qinq".to_string());
        config.bridge = Some("vmbr0".to_string());
        config.tag = Some(100);
        config.mtu = Some(1500);

        let configs = zone.generate_config(&config).await.unwrap();

        assert!(configs.contains_key("bridge"));
        assert!(configs.contains_key("vlan"));
        assert!(configs.contains_key("systemd"));
        assert!(configs.contains_key("metadata"));

        let bridge_config = configs.get("bridge").unwrap();
        assert!(bridge_config.contains("bridge_vlan_aware yes"));
        assert!(bridge_config.contains("bridge_vlan_protocol 802.1ad"));
        assert!(bridge_config.contains("mtu 1500"));
    }
}
