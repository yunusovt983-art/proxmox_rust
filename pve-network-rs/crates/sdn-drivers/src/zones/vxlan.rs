//! VXLAN zone driver
//!
//! VXLAN zones provide Layer 2 overlay networks over Layer 3 infrastructure.
//! This enables virtual networks to span across multiple physical hosts and data centers.

use anyhow::{Context, Result};
use async_trait::async_trait;
use log::{debug, info, warn};
use pve_sdn_core::{Zone, ZoneConfig, ZoneType};
use std::collections::HashMap;
use std::net::IpAddr;

/// VXLAN zone implementation
///
/// VXLAN zones create Layer 2 overlay networks using VXLAN encapsulation.
/// Key features:
/// - 24-bit VXLAN Network Identifier (VNI) for network isolation
/// - UDP encapsulation (default port 4789)
/// - Multicast or unicast replication for BUM traffic
/// - Support for both kernel and hardware VXLAN offload
pub struct VxlanZone {
    name: String,
}

impl VxlanZone {
    /// Create new VXLAN zone
    pub fn new(name: String) -> Self {
        Self { name }
    }

    /// Validate VXLAN-specific configuration parameters
    fn validate_vxlan_config(&self, config: &ZoneConfig) -> Result<()> {
        // VXLAN requires peers for unicast mode or multicast group
        if config.peers.is_none() || config.peers.as_ref().unwrap().is_empty() {
            if config.options.get("multicast-group").is_none() {
                anyhow::bail!(
                    "VXLAN zone '{}' requires either peers or multicast-group",
                    self.name
                );
            }
        }

        // Validate VXLAN port
        let vxlan_port = config.vxlan_port.unwrap_or(4789);
        if vxlan_port == 0 {
            anyhow::bail!("VXLAN zone '{}' port cannot be 0", self.name);
        }

        // Validate VNI (VXLAN Network Identifier)
        if let Some(vni) = config.options.get("vni") {
            if let Some(vni_num) = vni.as_u64() {
                if vni_num == 0 || vni_num > 16777215 {
                    anyhow::bail!(
                        "VXLAN zone '{}' VNI must be between 1 and 16777215",
                        self.name
                    );
                }
            } else {
                anyhow::bail!("VXLAN zone '{}' VNI must be a number", self.name);
            }
        } else {
            anyhow::bail!(
                "VXLAN zone '{}' requires a VNI (VXLAN Network Identifier)",
                self.name
            );
        }

        // Validate multicast group if specified
        if let Some(mcast_group) = config.options.get("multicast-group") {
            if let Some(group_str) = mcast_group.as_str() {
                let addr: IpAddr = group_str.parse().with_context(|| {
                    format!(
                        "Invalid multicast group '{}' for VXLAN zone '{}'",
                        group_str, self.name
                    )
                })?;

                if let IpAddr::V4(ipv4) = addr {
                    if !ipv4.is_multicast() {
                        anyhow::bail!(
                            "VXLAN zone '{}' multicast group '{}' is not a valid multicast address",
                            self.name,
                            group_str
                        );
                    }
                } else {
                    anyhow::bail!("VXLAN zone '{}' multicast group must be IPv4", self.name);
                }
            }
        }

        // Validate peers if specified
        if let Some(peers) = &config.peers {
            for peer in peers {
                let _addr: IpAddr = peer.parse().with_context(|| {
                    format!(
                        "Invalid peer address '{}' for VXLAN zone '{}'",
                        peer, self.name
                    )
                })?;
            }
        }

        // Validate local IP if specified
        if let Some(local_ip) = config.options.get("local-ip") {
            if let Some(local_str) = local_ip.as_str() {
                let _addr: IpAddr = local_str.parse().with_context(|| {
                    format!(
                        "Invalid local IP '{}' for VXLAN zone '{}'",
                        local_str, self.name
                    )
                })?;
            }
        }

        // Validate MTU considerations for VXLAN overhead
        if let Some(mtu) = config.mtu {
            if mtu > 1450 {
                warn!(
                    "VXLAN zone '{}' MTU {} may be too large considering VXLAN overhead (50 bytes)",
                    self.name, mtu
                );
            }
        }

        Ok(())
    }

    /// Get VXLAN interface name
    fn get_vxlan_interface_name(&self, vni: u32) -> String {
        format!("vxlan{}", vni)
    }

    /// Generate VXLAN interface configuration
    fn generate_vxlan_interface_config(&self, config: &ZoneConfig) -> Result<String> {
        let vni = config.options.get("vni").unwrap().as_u64().unwrap() as u32;
        let vxlan_interface = self.get_vxlan_interface_name(vni);
        let vxlan_port = config.vxlan_port.unwrap_or(4789);

        let mut vxlan_config = format!(
            "auto {vxlan_interface}\n\
             iface {vxlan_interface} inet manual\n\
             \tvxlan-id {vni}\n\
             \tvxlan-port {vxlan_port}\n",
            vxlan_interface = vxlan_interface,
            vni = vni,
            vxlan_port = vxlan_port
        );

        // Add local IP if specified
        if let Some(local_ip) = config.options.get("local-ip") {
            if let Some(local_str) = local_ip.as_str() {
                vxlan_config.push_str(&format!("\tvxlan-local-tunnelip {}\n", local_str));
            }
        }

        // Configure multicast or unicast mode
        if let Some(mcast_group) = config.options.get("multicast-group") {
            if let Some(group_str) = mcast_group.as_str() {
                vxlan_config.push_str(&format!("\tvxlan-svcnodeip {}\n", group_str));

                // Add physical device for multicast
                if let Some(physical_dev) = config.options.get("physical-device") {
                    if let Some(dev_str) = physical_dev.as_str() {
                        vxlan_config.push_str(&format!("\tvxlan-physdev {}\n", dev_str));
                    }
                }
            }
        } else if let Some(peers) = &config.peers {
            // Unicast mode with static peers
            for peer in peers {
                vxlan_config.push_str(&format!("\tvxlan-remoteip {}\n", peer));
            }
        }

        // Add MTU if specified
        if let Some(mtu) = config.mtu {
            vxlan_config.push_str(&format!("\tmtu {}\n", mtu));
        }

        // Additional VXLAN options
        if let Some(learning) = config.options.get("learning") {
            if let Some(learning_bool) = learning.as_bool() {
                vxlan_config.push_str(&format!(
                    "\tvxlan-learning {}\n",
                    if learning_bool { "on" } else { "off" }
                ));
            }
        }

        if let Some(proxy) = config.options.get("arp-proxy") {
            if let Some(proxy_bool) = proxy.as_bool() {
                vxlan_config.push_str(&format!(
                    "\tvxlan-proxy {}\n",
                    if proxy_bool { "on" } else { "off" }
                ));
            }
        }

        Ok(vxlan_config)
    }

    /// Generate bridge configuration for VXLAN
    fn generate_bridge_config(&self, config: &ZoneConfig) -> Result<String> {
        let vni = config.options.get("vni").unwrap().as_u64().unwrap() as u32;
        let vxlan_interface = self.get_vxlan_interface_name(vni);
        let default_bridge = format!("vmbr{}", vni);
        let bridge = config.bridge.as_ref().unwrap_or(&default_bridge);

        let mut bridge_config = format!(
            "auto {bridge}\n\
             iface {bridge} inet manual\n\
             \tbridge_ports {vxlan_interface}\n\
             \tbridge_stp off\n\
             \tbridge_fd 0\n",
            bridge = bridge,
            vxlan_interface = vxlan_interface
        );

        // Add VLAN awareness if specified
        if config.vlan_aware.unwrap_or(false) {
            bridge_config.push_str("\tbridge_vlan_aware yes\n");
        }

        // Add MTU if specified
        if let Some(mtu) = config.mtu {
            bridge_config.push_str(&format!("\tmtu {}\n", mtu));
        }

        Ok(bridge_config)
    }

    /// Generate systemd network configuration for VXLAN
    fn generate_systemd_config(&self, config: &ZoneConfig) -> Result<String> {
        let vni = config.options.get("vni").unwrap().as_u64().unwrap() as u32;
        let vxlan_interface = self.get_vxlan_interface_name(vni);
        let vxlan_port = config.vxlan_port.unwrap_or(4789);

        let mut systemd_config = format!(
            "[NetDev]\n\
             Name={vxlan_interface}\n\
             Kind=vxlan\n\
             \n\
             [VXLAN]\n\
             VNI={vni}\n\
             DestinationPort={vxlan_port}\n",
            vxlan_interface = vxlan_interface,
            vni = vni,
            vxlan_port = vxlan_port
        );

        // Add local IP if specified
        if let Some(local_ip) = config.options.get("local-ip") {
            if let Some(local_str) = local_ip.as_str() {
                systemd_config.push_str(&format!("Local={}\n", local_str));
            }
        }

        // Configure multicast or unicast mode
        if let Some(mcast_group) = config.options.get("multicast-group") {
            if let Some(group_str) = mcast_group.as_str() {
                systemd_config.push_str(&format!("Group={}\n", group_str));
            }
        }

        // Additional VXLAN options
        if let Some(learning) = config.options.get("learning") {
            if let Some(learning_bool) = learning.as_bool() {
                systemd_config.push_str(&format!("MacLearning={}\n", learning_bool));
            }
        }

        Ok(systemd_config)
    }
}

#[async_trait]
impl Zone for VxlanZone {
    fn zone_type(&self) -> ZoneType {
        ZoneType::Vxlan
    }

    fn name(&self) -> &str {
        &self.name
    }

    async fn validate_config(&self, config: &ZoneConfig) -> Result<()> {
        debug!("Validating VXLAN zone '{}' configuration", self.name);

        // Basic validation
        config
            .validate()
            .with_context(|| format!("Basic validation failed for VXLAN zone '{}'", self.name))?;

        // VXLAN-specific validation
        self.validate_vxlan_config(config).with_context(|| {
            format!("VXLAN-specific validation failed for zone '{}'", self.name)
        })?;

        info!(
            "VXLAN zone '{}' configuration validation successful",
            self.name
        );
        Ok(())
    }

    async fn apply_config(&self, config: &ZoneConfig) -> Result<()> {
        debug!("Applying VXLAN zone '{}' configuration", self.name);

        // Validate configuration first
        self.validate_config(config).await.with_context(|| {
            format!(
                "Configuration validation failed for VXLAN zone '{}'",
                self.name
            )
        })?;

        let vni = config.options.get("vni").unwrap().as_u64().unwrap() as u32;
        let vxlan_interface = self.get_vxlan_interface_name(vni);
        let vxlan_port = config.vxlan_port.unwrap_or(4789);

        // Check if VXLAN interface already exists
        let interface_exists = tokio::process::Command::new("ip")
            .args(&["link", "show", &vxlan_interface])
            .output()
            .await
            .map(|output| output.status.success())
            .unwrap_or(false);

        if !interface_exists {
            info!(
                "Creating VXLAN interface '{}' for zone '{}'",
                vxlan_interface, self.name
            );

            let vni_str = vni.to_string();
            let vxlan_port_str = vxlan_port.to_string();
            let mut cmd_args = vec![
                "link",
                "add",
                &vxlan_interface,
                "type",
                "vxlan",
                "id",
                &vni_str,
                "dstport",
                &vxlan_port_str,
            ];

            // Prepare string arguments to avoid lifetime issues
            let local_ip_str = config
                .options
                .get("local-ip")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());

            let group_str = config
                .options
                .get("multicast-group")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());

            let dev_str = config
                .options
                .get("physical-device")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());

            // Add local IP if specified
            if let Some(ref local_str) = local_ip_str {
                cmd_args.extend(&["local", local_str]);
            }

            // Configure multicast or unicast mode
            if let Some(ref group) = group_str {
                cmd_args.extend(&["group", group]);

                // Add physical device for multicast
                if let Some(ref dev) = dev_str {
                    cmd_args.extend(&["dev", dev]);
                }
            }

            // Additional VXLAN options
            if let Some(learning) = config.options.get("learning") {
                if let Some(false) = learning.as_bool() {
                    cmd_args.extend(&["nolearning"]);
                }
            }

            if let Some(proxy) = config.options.get("arp-proxy") {
                if let Some(true) = proxy.as_bool() {
                    cmd_args.extend(&["proxy"]);
                }
            }

            let output = tokio::process::Command::new("ip")
                .args(&cmd_args)
                .output()
                .await
                .with_context(|| {
                    format!(
                        "Failed to create VXLAN interface '{}' for zone '{}'",
                        vxlan_interface, self.name
                    )
                })?;

            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                anyhow::bail!(
                    "Failed to create VXLAN interface '{}': {}",
                    vxlan_interface,
                    stderr
                );
            }

            // Set MTU if specified
            if let Some(mtu) = config.mtu {
                let output = tokio::process::Command::new("ip")
                    .args(&["link", "set", &vxlan_interface, "mtu", &mtu.to_string()])
                    .output()
                    .await
                    .with_context(|| {
                        format!("Failed to set MTU on VXLAN interface '{}'", vxlan_interface)
                    })?;

                if !output.status.success() {
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    warn!(
                        "Failed to set MTU {} on VXLAN interface '{}': {}",
                        mtu, vxlan_interface, stderr
                    );
                }
            }

            // Bring interface up
            let output = tokio::process::Command::new("ip")
                .args(&["link", "set", &vxlan_interface, "up"])
                .output()
                .await
                .with_context(|| {
                    format!("Failed to bring up VXLAN interface '{}'", vxlan_interface)
                })?;

            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                anyhow::bail!(
                    "Failed to bring up VXLAN interface '{}': {}",
                    vxlan_interface,
                    stderr
                );
            }

            // Add static peers for unicast mode
            if config.options.get("multicast-group").is_none() {
                if let Some(peers) = &config.peers {
                    for peer in peers {
                        let output = tokio::process::Command::new("bridge")
                            .args(&[
                                "fdb",
                                "append",
                                "00:00:00:00:00:00",
                                "dev",
                                &vxlan_interface,
                                "dst",
                                peer,
                            ])
                            .output()
                            .await;

                        if let Ok(output) = output {
                            if !output.status.success() {
                                let stderr = String::from_utf8_lossy(&output.stderr);
                                warn!(
                                    "Failed to add peer '{}' to VXLAN interface '{}': {}",
                                    peer, vxlan_interface, stderr
                                );
                            }
                        }
                    }
                }
            }
        }

        // Create bridge if specified
        if let Some(bridge) = &config.bridge {
            let bridge_exists = tokio::process::Command::new("ip")
                .args(&["link", "show", bridge])
                .output()
                .await
                .map(|output| output.status.success())
                .unwrap_or(false);

            if !bridge_exists {
                info!(
                    "Creating bridge '{}' for VXLAN zone '{}'",
                    bridge, self.name
                );

                let output = tokio::process::Command::new("ip")
                    .args(&["link", "add", "name", bridge, "type", "bridge"])
                    .output()
                    .await
                    .with_context(|| {
                        format!(
                            "Failed to create bridge '{}' for VXLAN zone '{}'",
                            bridge, self.name
                        )
                    })?;

                if !output.status.success() {
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    anyhow::bail!("Failed to create bridge '{}': {}", bridge, stderr);
                }

                // Add VXLAN interface to bridge
                let output = tokio::process::Command::new("ip")
                    .args(&["link", "set", &vxlan_interface, "master", bridge])
                    .output()
                    .await
                    .with_context(|| {
                        format!(
                            "Failed to add VXLAN interface '{}' to bridge '{}'",
                            vxlan_interface, bridge
                        )
                    })?;

                if !output.status.success() {
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    anyhow::bail!(
                        "Failed to add VXLAN interface '{}' to bridge '{}': {}",
                        vxlan_interface,
                        bridge,
                        stderr
                    );
                }

                // Configure VLAN awareness if specified
                if config.vlan_aware.unwrap_or(false) {
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
                        warn!(
                            "Failed to enable VLAN filtering on bridge '{}': {}",
                            bridge, stderr
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
        }

        info!(
            "VXLAN zone '{}' configuration applied successfully",
            self.name
        );
        Ok(())
    }

    async fn generate_config(&self, config: &ZoneConfig) -> Result<HashMap<String, String>> {
        debug!(
            "Generating configuration files for VXLAN zone '{}'",
            self.name
        );

        let mut configs = HashMap::new();

        // Generate VXLAN interface configuration
        let vxlan_config = self
            .generate_vxlan_interface_config(config)
            .with_context(|| {
                format!(
                    "Failed to generate VXLAN interface config for zone '{}'",
                    self.name
                )
            })?;
        configs.insert("vxlan".to_string(), vxlan_config);

        // Generate bridge configuration if bridge is specified
        if config.bridge.is_some() {
            let bridge_config = self.generate_bridge_config(config).with_context(|| {
                format!(
                    "Failed to generate bridge config for VXLAN zone '{}'",
                    self.name
                )
            })?;
            configs.insert("bridge".to_string(), bridge_config);
        }

        // Generate systemd network configuration
        let systemd_config = self.generate_systemd_config(config).with_context(|| {
            format!(
                "Failed to generate systemd config for VXLAN zone '{}'",
                self.name
            )
        })?;
        configs.insert("systemd".to_string(), systemd_config);

        // Generate zone-specific metadata
        let vni = config.options.get("vni").unwrap().as_u64().unwrap() as u32;
        let metadata = format!(
            "# VXLAN Zone Configuration\n\
             # Zone: {}\n\
             # Type: VXLAN\n\
             # VNI: {}\n\
             # Port: {}\n\
             # Interface: {}\n\
             # Bridge: {}\n\
             # Multicast Group: {}\n\
             # Peers: {}\n",
            self.name,
            vni,
            config.vxlan_port.unwrap_or(4789),
            self.get_vxlan_interface_name(vni),
            config.bridge.as_deref().unwrap_or("none"),
            config
                .options
                .get("multicast-group")
                .and_then(|v| v.as_str())
                .unwrap_or("none"),
            config
                .peers
                .as_ref()
                .map(|p| p.join(", "))
                .unwrap_or_else(|| "none".to_string())
        );
        configs.insert("metadata".to_string(), metadata);

        info!(
            "Generated configuration files for VXLAN zone '{}'",
            self.name
        );
        Ok(configs)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[tokio::test]
    async fn test_vxlan_zone_validation() {
        let zone = VxlanZone::new("test-vxlan".to_string());

        // Test valid configuration with multicast
        let mut config = ZoneConfig::new(ZoneType::Vxlan, "test-vxlan".to_string());
        config.options.insert("vni".to_string(), json!(100));
        config
            .options
            .insert("multicast-group".to_string(), json!("239.1.1.1"));
        config.vxlan_port = Some(4789);

        assert!(zone.validate_config(&config).await.is_ok());

        // Test valid configuration with peers
        config.options.remove("multicast-group");
        config.peers = Some(vec!["192.168.1.1".to_string(), "192.168.1.2".to_string()]);

        assert!(zone.validate_config(&config).await.is_ok());

        // Test missing VNI
        config.options.remove("vni");
        assert!(zone.validate_config(&config).await.is_err());

        // Test invalid VNI
        config.options.insert("vni".to_string(), json!(0));
        assert!(zone.validate_config(&config).await.is_err());

        config.options.insert("vni".to_string(), json!(16777216));
        assert!(zone.validate_config(&config).await.is_err());

        // Test missing peers and multicast group
        config.options.insert("vni".to_string(), json!(100));
        config.peers = None;
        assert!(zone.validate_config(&config).await.is_err());
    }

    #[tokio::test]
    async fn test_vxlan_config_generation() {
        let zone = VxlanZone::new("test-vxlan".to_string());

        let mut config = ZoneConfig::new(ZoneType::Vxlan, "test-vxlan".to_string());
        config.options.insert("vni".to_string(), json!(100));
        config
            .options
            .insert("multicast-group".to_string(), json!("239.1.1.1"));
        config.bridge = Some("vmbr0".to_string());
        config.vxlan_port = Some(4789);
        config.mtu = Some(1450);

        let configs = zone.generate_config(&config).await.unwrap();

        assert!(configs.contains_key("vxlan"));
        assert!(configs.contains_key("bridge"));
        assert!(configs.contains_key("systemd"));
        assert!(configs.contains_key("metadata"));

        let vxlan_config = configs.get("vxlan").unwrap();
        assert!(vxlan_config.contains("vxlan-id 100"));
        assert!(vxlan_config.contains("vxlan-port 4789"));
        assert!(vxlan_config.contains("vxlan-svcnodeip 239.1.1.1"));
        assert!(vxlan_config.contains("mtu 1450"));

        let bridge_config = configs.get("bridge").unwrap();
        assert!(bridge_config.contains("bridge_ports vxlan100"));
    }
}
