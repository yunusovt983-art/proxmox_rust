//! EVPN zone driver
//!
//! EVPN (Ethernet VPN) zones provide advanced Layer 2 VPN services using BGP.
//! This enables scalable multi-tenant networks with advanced features like
//! MAC mobility, ARP suppression, and integrated routing.

use anyhow::{Context, Result};
use async_trait::async_trait;
use log::{debug, info, warn};
use pve_sdn_core::{Zone, ZoneConfig, ZoneType};
use std::collections::HashMap;
use std::net::IpAddr;

/// EVPN zone implementation
///
/// EVPN zones provide Layer 2 VPN services using BGP EVPN control plane.
/// Key features:
/// - BGP EVPN control plane for MAC/IP advertisement
/// - VXLAN data plane for traffic forwarding
/// - Route Distinguisher (RD) and Route Target (RT) for VPN isolation
/// - Support for Type-2 (MAC/IP) and Type-3 (IMET) routes
/// - ARP suppression and MAC mobility
/// - Integrated Layer 3 routing (IRB - Integrated Routing and Bridging)
pub struct EvpnZone {
    name: String,
}

impl EvpnZone {
    /// Create new EVPN zone
    pub fn new(name: String) -> Self {
        Self { name }
    }

    /// Validate EVPN-specific configuration parameters
    fn validate_evpn_config(&self, config: &ZoneConfig) -> Result<()> {
        // EVPN requires a VNI
        if let Some(vni) = config.options.get("vni") {
            if let Some(vni_num) = vni.as_u64() {
                if vni_num == 0 || vni_num > 16777215 {
                    anyhow::bail!(
                        "EVPN zone '{}' VNI must be between 1 and 16777215",
                        self.name
                    );
                }
            } else {
                anyhow::bail!("EVPN zone '{}' VNI must be a number", self.name);
            }
        } else {
            anyhow::bail!("EVPN zone '{}' requires a VNI", self.name);
        }

        // Validate Route Distinguisher (RD)
        if let Some(rd) = config.options.get("rd") {
            if let Some(rd_str) = rd.as_str() {
                self.validate_route_distinguisher(rd_str).with_context(|| {
                    format!("Invalid Route Distinguisher for EVPN zone '{}'", self.name)
                })?;
            }
        } else {
            anyhow::bail!(
                "EVPN zone '{}' requires a Route Distinguisher (rd)",
                self.name
            );
        }

        // Validate Route Target (RT)
        if let Some(rt_import) = config.options.get("rt-import") {
            if let Some(rt_str) = rt_import.as_str() {
                self.validate_route_target(rt_str)
                    .with_context(|| format!("Invalid RT import for EVPN zone '{}'", self.name))?;
            }
        }

        if let Some(rt_export) = config.options.get("rt-export") {
            if let Some(rt_str) = rt_export.as_str() {
                self.validate_route_target(rt_str)
                    .with_context(|| format!("Invalid RT export for EVPN zone '{}'", self.name))?;
            }
        }

        // Validate controller reference
        if config.options.get("controller").is_none() {
            anyhow::bail!("EVPN zone '{}' requires a controller reference", self.name);
        }

        // Validate VTEP IP
        if let Some(vtep_ip) = config.options.get("vtep-ip") {
            if let Some(vtep_str) = vtep_ip.as_str() {
                let _addr: IpAddr = vtep_str.parse().with_context(|| {
                    format!(
                        "Invalid VTEP IP '{}' for EVPN zone '{}'",
                        vtep_str, self.name
                    )
                })?;
            }
        } else {
            anyhow::bail!("EVPN zone '{}' requires a VTEP IP address", self.name);
        }

        // Validate MAC-VRF settings
        if let Some(mac_vrf) = config.options.get("mac-vrf") {
            if let Some(mac_vrf_str) = mac_vrf.as_str() {
                if mac_vrf_str.is_empty() {
                    anyhow::bail!("EVPN zone '{}' MAC-VRF name cannot be empty", self.name);
                }
            }
        }

        // Validate advertise settings
        if let Some(advertise_pip) = config.options.get("advertise-pip") {
            if advertise_pip.as_bool().is_none() {
                anyhow::bail!("EVPN zone '{}' advertise-pip must be a boolean", self.name);
            }
        }

        if let Some(advertise_svi_ip) = config.options.get("advertise-svi-ip") {
            if advertise_svi_ip.as_bool().is_none() {
                anyhow::bail!(
                    "EVPN zone '{}' advertise-svi-ip must be a boolean",
                    self.name
                );
            }
        }

        Ok(())
    }

    /// Validate Route Distinguisher format (ASN:nn or IP:nn)
    fn validate_route_distinguisher(&self, rd: &str) -> Result<()> {
        let parts: Vec<&str> = rd.split(':').collect();
        if parts.len() != 2 {
            anyhow::bail!("Route Distinguisher must be in format 'ASN:nn' or 'IP:nn'");
        }

        // Try to parse as ASN:nn
        if let Ok(asn) = parts[0].parse::<u32>() {
            if asn > 65535 {
                // 4-byte ASN
                if let Err(_) = parts[1].parse::<u16>() {
                    anyhow::bail!("Invalid Route Distinguisher format for 4-byte ASN");
                }
            } else {
                // 2-byte ASN
                if let Err(_) = parts[1].parse::<u32>() {
                    anyhow::bail!("Invalid Route Distinguisher format for 2-byte ASN");
                }
            }
        } else {
            // Try to parse as IP:nn
            if let Err(_) = parts[0].parse::<IpAddr>() {
                anyhow::bail!("Route Distinguisher first part must be ASN or IP address");
            }
            if let Err(_) = parts[1].parse::<u16>() {
                anyhow::bail!("Route Distinguisher second part must be a number");
            }
        }

        Ok(())
    }

    /// Validate Route Target format
    fn validate_route_target(&self, rt: &str) -> Result<()> {
        // Route Target can be a list separated by spaces or commas
        let targets: Vec<&str> = rt
            .split(|c| c == ' ' || c == ',')
            .filter(|s| !s.is_empty())
            .collect();

        for target in targets {
            let parts: Vec<&str> = target.split(':').collect();
            if parts.len() != 2 {
                anyhow::bail!(
                    "Route Target '{}' must be in format 'ASN:nn' or 'IP:nn'",
                    target
                );
            }

            // Similar validation as RD
            if let Ok(asn) = parts[0].parse::<u32>() {
                if asn > 65535 {
                    if let Err(_) = parts[1].parse::<u16>() {
                        anyhow::bail!("Invalid Route Target format '{}' for 4-byte ASN", target);
                    }
                } else {
                    if let Err(_) = parts[1].parse::<u32>() {
                        anyhow::bail!("Invalid Route Target format '{}' for 2-byte ASN", target);
                    }
                }
            } else {
                if let Err(_) = parts[0].parse::<IpAddr>() {
                    anyhow::bail!(
                        "Route Target '{}' first part must be ASN or IP address",
                        target
                    );
                }
                if let Err(_) = parts[1].parse::<u16>() {
                    anyhow::bail!("Route Target '{}' second part must be a number", target);
                }
            }
        }

        Ok(())
    }

    /// Get VXLAN interface name for EVPN
    fn get_vxlan_interface_name(&self, vni: u32) -> String {
        format!("vxlan{}", vni)
    }

    /// Generate VXLAN interface configuration for EVPN
    fn generate_vxlan_interface_config(&self, config: &ZoneConfig) -> Result<String> {
        let vni = config.options.get("vni").unwrap().as_u64().unwrap() as u32;
        let vxlan_interface = self.get_vxlan_interface_name(vni);
        let vxlan_port = config.vxlan_port.unwrap_or(4789);
        let vtep_ip = config.options.get("vtep-ip").unwrap().as_str().unwrap();

        let mut vxlan_config = format!(
            "auto {vxlan_interface}\n\
             iface {vxlan_interface} inet manual\n\
             \tvxlan-id {vni}\n\
             \tvxlan-port {vxlan_port}\n\
             \tvxlan-local-tunnelip {vtep_ip}\n\
             \tvxlan-learning off\n\
             \tvxlan-proxy on\n",
            vxlan_interface = vxlan_interface,
            vni = vni,
            vxlan_port = vxlan_port,
            vtep_ip = vtep_ip
        );

        // Add MTU if specified
        if let Some(mtu) = config.mtu {
            vxlan_config.push_str(&format!("\tmtu {}\n", mtu));
        }

        // EVPN-specific settings
        vxlan_config.push_str("\t# EVPN settings\n");
        vxlan_config.push_str("\tvxlan-ageing 0\n"); // Disable MAC aging for EVPN

        Ok(vxlan_config)
    }

    /// Generate bridge configuration for EVPN
    fn generate_bridge_config(&self, config: &ZoneConfig) -> Result<String> {
        let vni = config.options.get("vni").unwrap().as_u64().unwrap() as u32;
        let vxlan_interface = self.get_vxlan_interface_name(vni);
        let default_bridge = format!("br{}", vni);
        let bridge = config.bridge.as_ref().unwrap_or(&default_bridge);

        let mut bridge_config = format!(
            "auto {bridge}\n\
             iface {bridge} inet manual\n\
             \tbridge_ports {vxlan_interface}\n\
             \tbridge_stp off\n\
             \tbridge_fd 0\n\
             \tbridge_vlan_aware yes\n",
            bridge = bridge,
            vxlan_interface = vxlan_interface
        );

        // Add MTU if specified
        if let Some(mtu) = config.mtu {
            bridge_config.push_str(&format!("\tmtu {}\n", mtu));
        }

        // EVPN-specific bridge settings
        bridge_config.push_str("\t# EVPN bridge settings\n");
        bridge_config.push_str("\tbridge_ageing 0\n"); // Disable MAC aging

        Ok(bridge_config)
    }

    /// Generate FRR BGP EVPN configuration
    fn generate_frr_config(&self, config: &ZoneConfig) -> Result<String> {
        let vni = config.options.get("vni").unwrap().as_u64().unwrap() as u32;
        let rd = config.options.get("rd").unwrap().as_str().unwrap();
        let vtep_ip = config.options.get("vtep-ip").unwrap().as_str().unwrap();

        let mut frr_config = format!(
            "!\n\
             ! EVPN configuration for zone {}\n\
             !\n\
             router bgp\n\
             !\n\
             vni {vni}\n\
             \trd {rd}\n",
            self.name,
            vni = vni,
            rd = rd
        );

        // Add Route Targets
        if let Some(rt_import) = config.options.get("rt-import") {
            if let Some(rt_str) = rt_import.as_str() {
                for rt in rt_str
                    .split(|c| c == ' ' || c == ',')
                    .filter(|s| !s.is_empty())
                {
                    frr_config.push_str(&format!("\troute-target import {}\n", rt));
                }
            }
        }

        if let Some(rt_export) = config.options.get("rt-export") {
            if let Some(rt_str) = rt_export.as_str() {
                for rt in rt_str
                    .split(|c| c == ' ' || c == ',')
                    .filter(|s| !s.is_empty())
                {
                    frr_config.push_str(&format!("\troute-target export {}\n", rt));
                }
            }
        }

        // Add advertise settings
        if let Some(advertise_pip) = config.options.get("advertise-pip") {
            if let Some(true) = advertise_pip.as_bool() {
                frr_config.push_str("\tadvertise-pip\n");
            }
        }

        if let Some(advertise_svi_ip) = config.options.get("advertise-svi-ip") {
            if let Some(true) = advertise_svi_ip.as_bool() {
                frr_config.push_str("\tadvertise-svi-ip\n");
            }
        }

        frr_config.push_str("\texit-vni\n!\n");

        // Add VTEP configuration
        frr_config.push_str(&format!(
            "interface {}\n\
             \tno shutdown\n\
             \tip address {}/32\n\
             !\n",
            self.get_vxlan_interface_name(vni),
            vtep_ip
        ));

        Ok(frr_config)
    }

    /// Generate systemd network configuration for EVPN
    fn generate_systemd_config(&self, config: &ZoneConfig) -> Result<String> {
        let vni = config.options.get("vni").unwrap().as_u64().unwrap() as u32;
        let vxlan_interface = self.get_vxlan_interface_name(vni);
        let vxlan_port = config.vxlan_port.unwrap_or(4789);
        let vtep_ip = config.options.get("vtep-ip").unwrap().as_str().unwrap();

        let systemd_config = format!(
            "[NetDev]\n\
             Name={vxlan_interface}\n\
             Kind=vxlan\n\
             \n\
             [VXLAN]\n\
             VNI={vni}\n\
             DestinationPort={vxlan_port}\n\
             Local={vtep_ip}\n\
             MacLearning=no\n\
             L2MissNotification=yes\n\
             L3MissNotification=yes\n",
            vxlan_interface = vxlan_interface,
            vni = vni,
            vxlan_port = vxlan_port,
            vtep_ip = vtep_ip
        );

        Ok(systemd_config)
    }
}

#[async_trait]
impl Zone for EvpnZone {
    fn zone_type(&self) -> ZoneType {
        ZoneType::Evpn
    }

    fn name(&self) -> &str {
        &self.name
    }

    async fn validate_config(&self, config: &ZoneConfig) -> Result<()> {
        debug!("Validating EVPN zone '{}' configuration", self.name);

        // Basic validation
        config
            .validate()
            .with_context(|| format!("Basic validation failed for EVPN zone '{}'", self.name))?;

        // EVPN-specific validation
        self.validate_evpn_config(config)
            .with_context(|| format!("EVPN-specific validation failed for zone '{}'", self.name))?;

        info!(
            "EVPN zone '{}' configuration validation successful",
            self.name
        );
        Ok(())
    }

    async fn apply_config(&self, config: &ZoneConfig) -> Result<()> {
        debug!("Applying EVPN zone '{}' configuration", self.name);

        // Validate configuration first
        self.validate_config(config).await.with_context(|| {
            format!(
                "Configuration validation failed for EVPN zone '{}'",
                self.name
            )
        })?;

        let vni = config.options.get("vni").unwrap().as_u64().unwrap() as u32;
        let vxlan_interface = self.get_vxlan_interface_name(vni);
        let vxlan_port = config.vxlan_port.unwrap_or(4789);
        let vtep_ip = config.options.get("vtep-ip").unwrap().as_str().unwrap();

        // Check if VXLAN interface already exists
        let interface_exists = tokio::process::Command::new("ip")
            .args(&["link", "show", &vxlan_interface])
            .output()
            .await
            .map(|output| output.status.success())
            .unwrap_or(false);

        if !interface_exists {
            info!(
                "Creating EVPN VXLAN interface '{}' for zone '{}'",
                vxlan_interface, self.name
            );

            let vni_str = vni.to_string();
            let vxlan_port_str = vxlan_port.to_string();
            let cmd_args = vec![
                "link",
                "add",
                &vxlan_interface,
                "type",
                "vxlan",
                "id",
                &vni_str,
                "dstport",
                &vxlan_port_str,
                "local",
                vtep_ip,
                "nolearning", // Disable learning for EVPN
                "proxy",      // Enable ARP/ND proxy
            ];

            let output = tokio::process::Command::new("ip")
                .args(&cmd_args)
                .output()
                .await
                .with_context(|| {
                    format!(
                        "Failed to create EVPN VXLAN interface '{}' for zone '{}'",
                        vxlan_interface, self.name
                    )
                })?;

            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                anyhow::bail!(
                    "Failed to create EVPN VXLAN interface '{}': {}",
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
                        format!(
                            "Failed to set MTU on EVPN VXLAN interface '{}'",
                            vxlan_interface
                        )
                    })?;

                if !output.status.success() {
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    warn!(
                        "Failed to set MTU {} on EVPN VXLAN interface '{}': {}",
                        mtu, vxlan_interface, stderr
                    );
                }
            }

            // Disable MAC aging for EVPN
            let output = tokio::process::Command::new("ip")
                .args(&[
                    "link",
                    "set",
                    &vxlan_interface,
                    "type",
                    "vxlan",
                    "ageing",
                    "0",
                ])
                .output()
                .await;

            if let Ok(output) = output {
                if !output.status.success() {
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    warn!(
                        "Failed to disable MAC aging on EVPN VXLAN interface '{}': {}",
                        vxlan_interface, stderr
                    );
                }
            }

            // Bring interface up
            let output = tokio::process::Command::new("ip")
                .args(&["link", "set", &vxlan_interface, "up"])
                .output()
                .await
                .with_context(|| {
                    format!(
                        "Failed to bring up EVPN VXLAN interface '{}'",
                        vxlan_interface
                    )
                })?;

            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                anyhow::bail!(
                    "Failed to bring up EVPN VXLAN interface '{}': {}",
                    vxlan_interface,
                    stderr
                );
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
                info!("Creating bridge '{}' for EVPN zone '{}'", bridge, self.name);

                let output = tokio::process::Command::new("ip")
                    .args(&["link", "add", "name", bridge, "type", "bridge"])
                    .output()
                    .await
                    .with_context(|| {
                        format!(
                            "Failed to create bridge '{}' for EVPN zone '{}'",
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
                            "Failed to add EVPN VXLAN interface '{}' to bridge '{}'",
                            vxlan_interface, bridge
                        )
                    })?;

                if !output.status.success() {
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    anyhow::bail!(
                        "Failed to add EVPN VXLAN interface '{}' to bridge '{}': {}",
                        vxlan_interface,
                        bridge,
                        stderr
                    );
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
                    warn!(
                        "Failed to enable VLAN filtering on bridge '{}': {}",
                        bridge, stderr
                    );
                }

                // Disable MAC aging on bridge for EVPN
                let output = tokio::process::Command::new("ip")
                    .args(&["link", "set", bridge, "type", "bridge", "ageing_time", "0"])
                    .output()
                    .await;

                if let Ok(output) = output {
                    if !output.status.success() {
                        let stderr = String::from_utf8_lossy(&output.stderr);
                        warn!(
                            "Failed to disable MAC aging on bridge '{}': {}",
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
            "EVPN zone '{}' configuration applied successfully",
            self.name
        );
        Ok(())
    }

    async fn generate_config(&self, config: &ZoneConfig) -> Result<HashMap<String, String>> {
        debug!(
            "Generating configuration files for EVPN zone '{}'",
            self.name
        );

        let mut configs = HashMap::new();

        // Generate VXLAN interface configuration
        let vxlan_config = self
            .generate_vxlan_interface_config(config)
            .with_context(|| {
                format!(
                    "Failed to generate VXLAN interface config for EVPN zone '{}'",
                    self.name
                )
            })?;
        configs.insert("vxlan".to_string(), vxlan_config);

        // Generate bridge configuration if bridge is specified
        if config.bridge.is_some() {
            let bridge_config = self.generate_bridge_config(config).with_context(|| {
                format!(
                    "Failed to generate bridge config for EVPN zone '{}'",
                    self.name
                )
            })?;
            configs.insert("bridge".to_string(), bridge_config);
        }

        // Generate FRR BGP EVPN configuration
        let frr_config = self.generate_frr_config(config).with_context(|| {
            format!(
                "Failed to generate FRR config for EVPN zone '{}'",
                self.name
            )
        })?;
        configs.insert("frr".to_string(), frr_config);

        // Generate systemd network configuration
        let systemd_config = self.generate_systemd_config(config).with_context(|| {
            format!(
                "Failed to generate systemd config for EVPN zone '{}'",
                self.name
            )
        })?;
        configs.insert("systemd".to_string(), systemd_config);

        // Generate zone-specific metadata
        let vni = config.options.get("vni").unwrap().as_u64().unwrap() as u32;
        let rd = config.options.get("rd").unwrap().as_str().unwrap();
        let vtep_ip = config.options.get("vtep-ip").unwrap().as_str().unwrap();

        let metadata = format!(
            "# EVPN Zone Configuration\n\
             # Zone: {}\n\
             # Type: EVPN\n\
             # VNI: {}\n\
             # Route Distinguisher: {}\n\
             # VTEP IP: {}\n\
             # Interface: {}\n\
             # Bridge: {}\n\
             # Controller: {}\n\
             # RT Import: {}\n\
             # RT Export: {}\n",
            self.name,
            vni,
            rd,
            vtep_ip,
            self.get_vxlan_interface_name(vni),
            config.bridge.as_deref().unwrap_or("none"),
            config
                .options
                .get("controller")
                .and_then(|v| v.as_str())
                .unwrap_or("none"),
            config
                .options
                .get("rt-import")
                .and_then(|v| v.as_str())
                .unwrap_or("none"),
            config
                .options
                .get("rt-export")
                .and_then(|v| v.as_str())
                .unwrap_or("none")
        );
        configs.insert("metadata".to_string(), metadata);

        info!(
            "Generated configuration files for EVPN zone '{}'",
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
    async fn test_evpn_zone_validation() {
        let zone = EvpnZone::new("test-evpn".to_string());

        // Test valid configuration
        let mut config = ZoneConfig::new(ZoneType::Evpn, "test-evpn".to_string());
        config.options.insert("vni".to_string(), json!(100));
        config.options.insert("rd".to_string(), json!("65000:100"));
        config
            .options
            .insert("rt-import".to_string(), json!("65000:100"));
        config
            .options
            .insert("rt-export".to_string(), json!("65000:100"));
        config
            .options
            .insert("vtep-ip".to_string(), json!("192.168.1.1"));
        config
            .options
            .insert("controller".to_string(), json!("evpn1"));

        assert!(zone.validate_config(&config).await.is_ok());

        // Test missing VNI
        config.options.remove("vni");
        assert!(zone.validate_config(&config).await.is_err());

        // Test invalid VNI
        config.options.insert("vni".to_string(), json!(0));
        assert!(zone.validate_config(&config).await.is_err());

        // Test missing RD
        config.options.insert("vni".to_string(), json!(100));
        config.options.remove("rd");
        assert!(zone.validate_config(&config).await.is_err());

        // Test invalid RD format
        config.options.insert("rd".to_string(), json!("invalid"));
        assert!(zone.validate_config(&config).await.is_err());

        // Test missing VTEP IP
        config.options.insert("rd".to_string(), json!("65000:100"));
        config.options.remove("vtep-ip");
        assert!(zone.validate_config(&config).await.is_err());
    }

    #[tokio::test]
    async fn test_evpn_config_generation() {
        let zone = EvpnZone::new("test-evpn".to_string());

        let mut config = ZoneConfig::new(ZoneType::Evpn, "test-evpn".to_string());
        config.options.insert("vni".to_string(), json!(100));
        config.options.insert("rd".to_string(), json!("65000:100"));
        config
            .options
            .insert("rt-import".to_string(), json!("65000:100"));
        config
            .options
            .insert("rt-export".to_string(), json!("65000:100"));
        config
            .options
            .insert("vtep-ip".to_string(), json!("192.168.1.1"));
        config
            .options
            .insert("controller".to_string(), json!("evpn1"));
        config.bridge = Some("vmbr0".to_string());
        config.vxlan_port = Some(4789);

        let configs = zone.generate_config(&config).await.unwrap();

        assert!(configs.contains_key("vxlan"));
        assert!(configs.contains_key("bridge"));
        assert!(configs.contains_key("frr"));
        assert!(configs.contains_key("systemd"));
        assert!(configs.contains_key("metadata"));

        let vxlan_config = configs.get("vxlan").unwrap();
        assert!(vxlan_config.contains("vxlan-id 100"));
        assert!(vxlan_config.contains("vxlan-local-tunnelip 192.168.1.1"));
        assert!(vxlan_config.contains("vxlan-learning off"));
        assert!(vxlan_config.contains("vxlan-proxy on"));

        let frr_config = configs.get("frr").unwrap();
        assert!(frr_config.contains("vni 100"));
        assert!(frr_config.contains("rd 65000:100"));
        assert!(frr_config.contains("route-target import 65000:100"));
        assert!(frr_config.contains("route-target export 65000:100"));
    }

    #[test]
    fn test_route_distinguisher_validation() {
        let zone = EvpnZone::new("test".to_string());

        // Valid RD formats
        assert!(zone.validate_route_distinguisher("65000:100").is_ok());
        assert!(zone.validate_route_distinguisher("192.168.1.1:100").is_ok());
        assert!(zone.validate_route_distinguisher("4294967295:100").is_ok()); // 4-byte ASN

        // Invalid RD formats
        assert!(zone.validate_route_distinguisher("invalid").is_err());
        assert!(zone.validate_route_distinguisher("65000").is_err());
        assert!(zone.validate_route_distinguisher("65000:").is_err());
        assert!(zone.validate_route_distinguisher(":100").is_err());
    }

    #[test]
    fn test_route_target_validation() {
        let zone = EvpnZone::new("test".to_string());

        // Valid RT formats
        assert!(zone.validate_route_target("65000:100").is_ok());
        assert!(zone.validate_route_target("65000:100 65000:200").is_ok());
        assert!(zone.validate_route_target("65000:100,65000:200").is_ok());
        assert!(zone.validate_route_target("192.168.1.1:100").is_ok());

        // Invalid RT formats
        assert!(zone.validate_route_target("invalid").is_err());
        assert!(zone.validate_route_target("65000:100 invalid").is_err());
    }
}
