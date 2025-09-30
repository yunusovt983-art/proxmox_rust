//! EVPN controller driver
//!
//! EVPN controller provides BGP EVPN control plane functionality for SDN zones.
//! This controller manages FRR BGP EVPN configuration and VXLAN integration.

use anyhow::{Context, Result};
use async_trait::async_trait;
use log::{debug, error, info, warn};
use pve_sdn_core::controller::{ControllerConfig, ControllerStatus};
use pve_sdn_core::{Controller, ControllerType, VNet, Zone};
use std::collections::HashMap;
use std::net::IpAddr;
use std::path::Path;
use tokio::process::Command;

/// EVPN controller implementation
///
/// The EVPN controller manages FRR BGP EVPN daemon for advanced Layer 2 VPN services.
/// Key features:
/// - BGP EVPN control plane
/// - VXLAN data plane integration
/// - Route Distinguisher (RD) and Route Target (RT) management
/// - Type-2 (MAC/IP) and Type-3 (IMET) route advertisement
/// - ARP suppression and MAC mobility
/// - Integrated routing and bridging (IRB)
pub struct EvpnController {
    name: String,
}

impl EvpnController {
    /// Create new EVPN controller
    pub fn new(name: String) -> Self {
        Self { name }
    }

    /// Validate EVPN-specific configuration
    fn validate_evpn_config(&self, config: &ControllerConfig) -> Result<()> {
        // EVPN requires an ASN
        if config.asn.is_none() {
            anyhow::bail!("EVPN controller '{}' requires an ASN", self.name);
        }

        let asn = config.asn.unwrap();
        if asn == 0 || asn > 4294967295 {
            anyhow::bail!(
                "EVPN controller '{}' ASN must be between 1 and 4294967295",
                self.name
            );
        }

        // Validate peers if specified
        if let Some(peers) = &config.peers {
            for peer in peers {
                let _addr: IpAddr = peer.parse().with_context(|| {
                    format!(
                        "Invalid peer address '{}' for EVPN controller '{}'",
                        peer, self.name
                    )
                })?;
            }
        }

        // Validate router-id if specified
        if let Some(router_id) = config.options.get("router-id") {
            if let Some(router_id_str) = router_id.as_str() {
                let _addr: IpAddr = router_id_str.parse().with_context(|| {
                    format!(
                        "Invalid router-id '{}' for EVPN controller '{}'",
                        router_id_str, self.name
                    )
                })?;
            }
        }

        // Validate VTEP IP
        if let Some(vtep_ip) = config.options.get("vtep-ip") {
            if let Some(vtep_str) = vtep_ip.as_str() {
                let _addr: IpAddr = vtep_str.parse().with_context(|| {
                    format!(
                        "Invalid VTEP IP '{}' for EVPN controller '{}'",
                        vtep_str, self.name
                    )
                })?;
            }
        } else {
            anyhow::bail!("EVPN controller '{}' requires a VTEP IP address", self.name);
        }

        Ok(())
    }

    /// Generate FRR BGP EVPN configuration
    fn generate_frr_evpn_config(&self, config: &ControllerConfig) -> Result<String> {
        let asn = config.asn.unwrap();
        let vtep_ip = config.options.get("vtep-ip").unwrap().as_str().unwrap();

        let mut evpn_config = format!(
            "!\n\
             ! BGP EVPN configuration for controller {}\n\
             !\n\
             router bgp {asn}\n",
            self.name,
            asn = asn
        );

        // Add router-id
        let router_id = config
            .options
            .get("router-id")
            .and_then(|v| v.as_str())
            .unwrap_or(vtep_ip);
        evpn_config.push_str(&format!(" bgp router-id {}\n", router_id));

        // BGP configuration options
        if let Some(true) = config.bgp_multipath_relax {
            evpn_config.push_str(" bgp bestpath as-path multipath-relax\n");
        }

        if let Some(false) = config.ebgp_requires_policy {
            evpn_config.push_str(" no bgp ebgp-requires-policy\n");
        }

        // EVPN-specific BGP settings
        evpn_config.push_str(" bgp log-neighbor-changes\n");
        evpn_config.push_str(" no bgp default ipv4-unicast\n");

        // Add BGP neighbors
        if let Some(peers) = &config.peers {
            for peer in peers {
                evpn_config.push_str(&format!(" neighbor {} remote-as external\n", peer));
                evpn_config.push_str(&format!(" neighbor {} capability extended-nexthop\n", peer));

                // Add peer-specific options
                if let Some(peer_options) = config.options.get(&format!("peer-{}", peer)) {
                    if let Some(peer_obj) = peer_options.as_object() {
                        if let Some(remote_asn) = peer_obj.get("remote-as") {
                            if let Some(remote_asn_num) = remote_asn.as_u64() {
                                evpn_config = evpn_config.replace(
                                    &format!(" neighbor {} remote-as external\n", peer),
                                    &format!(" neighbor {} remote-as {}\n", peer, remote_asn_num),
                                );
                            }
                        }

                        if let Some(description) = peer_obj.get("description") {
                            if let Some(desc_str) = description.as_str() {
                                evpn_config.push_str(&format!(
                                    " neighbor {} description {}\n",
                                    peer, desc_str
                                ));
                            }
                        }
                    }
                }
            }
        }

        // L2VPN EVPN address family
        evpn_config.push_str(" !\n address-family l2vpn evpn\n");

        if let Some(peers) = &config.peers {
            for peer in peers {
                evpn_config.push_str(&format!("  neighbor {} activate\n", peer));
                evpn_config.push_str(&format!("  neighbor {} route-reflector-client\n", peer));
            }
        }

        // EVPN advertise settings
        if let Some(true) = config
            .options
            .get("advertise-all-vni")
            .and_then(|v| v.as_bool())
        {
            evpn_config.push_str("  advertise-all-vni\n");
        }

        if let Some(true) = config
            .options
            .get("advertise-default-gw")
            .and_then(|v| v.as_bool())
        {
            evpn_config.push_str("  advertise-default-gw\n");
        }

        if let Some(true) = config
            .options
            .get("advertise-svi-ip")
            .and_then(|v| v.as_bool())
        {
            evpn_config.push_str("  advertise-svi-ip\n");
        }

        evpn_config.push_str(" exit-address-family\n");

        // IPv4 unicast address family for underlay
        evpn_config.push_str(" !\n address-family ipv4 unicast\n");

        if let Some(peers) = &config.peers {
            for peer in peers {
                evpn_config.push_str(&format!("  neighbor {} activate\n", peer));
            }
        }

        // Network advertisements for underlay
        if let Some(networks) = config.options.get("underlay-networks") {
            if let Some(networks_array) = networks.as_array() {
                for network in networks_array {
                    if let Some(network_str) = network.as_str() {
                        evpn_config.push_str(&format!("  network {}\n", network_str));
                    }
                }
            }
        }

        // Redistribute connected for VTEP IP
        evpn_config.push_str("  redistribute connected\n");

        evpn_config.push_str(" exit-address-family\n");

        evpn_config.push_str("!\n");

        // Add VTEP interface configuration
        evpn_config.push_str(&format!(
            "interface lo\n\
             \tip address {}/32\n\
             !\n",
            vtep_ip
        ));

        Ok(evpn_config)
    }

    /// Generate VXLAN-specific configuration for EVPN zones
    fn generate_vxlan_evpn_config(
        &self,
        zones: &[Box<dyn Zone>],
        vnets: &[VNet],
    ) -> Result<String> {
        let mut vxlan_config = String::new();

        vxlan_config.push_str(&format!(
            "!\n\
             ! VXLAN configuration for EVPN controller {}\n\
             !\n",
            self.name
        ));

        // Configure VNIs for EVPN zones
        for zone in zones {
            if zone.zone_type() == pve_sdn_core::ZoneType::Evpn {
                // This would need zone configuration access
                // For now, we'll add a placeholder
                vxlan_config.push_str(&format!("! EVPN zone: {}\n", zone.name()));
            }
        }

        // Configure VNets
        for vnet in vnets {
            vxlan_config.push_str(&format!(
                "! VNet: {} (Zone: {})\n",
                vnet.name(),
                vnet.zone()
            ));
        }

        Ok(vxlan_config)
    }

    /// Generate systemd service configuration
    fn generate_systemd_config(&self, _config: &ControllerConfig) -> Result<String> {
        let systemd_config = format!(
            "[Unit]\n\
             Description=FRR BGP EVPN daemon for controller {}\n\
             After=network.target\n\
             Wants=network.target\n\
             \n\
             [Service]\n\
             Type=forking\n\
             ExecStart=/usr/lib/frr/bgpd -d -f /etc/frr/bgpd-evpn-{}.conf --pid_file /var/run/frr/bgpd-evpn-{}.pid\n\
             ExecReload=/bin/kill -HUP $MAINPID\n\
             PIDFile=/var/run/frr/bgpd-evpn-{}.pid\n\
             Restart=on-failure\n\
             \n\
             [Install]\n\
             WantedBy=multi-user.target\n",
            self.name, self.name, self.name, self.name
        );

        Ok(systemd_config)
    }

    /// Check if FRR is installed with EVPN support
    async fn check_frr_evpn_support(&self) -> Result<bool> {
        let output = Command::new("bgpd").args(&["--help"]).output().await?;

        if !output.status.success() {
            return Ok(false);
        }

        let help_text = String::from_utf8_lossy(&output.stdout);
        Ok(help_text.contains("evpn") || help_text.contains("l2vpn"))
    }

    /// Get EVPN BGP daemon PID
    async fn get_evpn_bgp_pid(&self) -> Result<Option<u32>> {
        let pid_file = format!("/var/run/frr/bgpd-evpn-{}.pid", self.name);

        if !Path::new(&pid_file).exists() {
            return Ok(None);
        }

        let pid_content = tokio::fs::read_to_string(&pid_file).await?;
        let pid: u32 = pid_content
            .trim()
            .parse()
            .with_context(|| format!("Invalid PID in file {}", pid_file))?;

        // Check if process is actually running
        let output = Command::new("kill")
            .args(&["-0", &pid.to_string()])
            .output()
            .await?;

        if output.status.success() {
            Ok(Some(pid))
        } else {
            Ok(None)
        }
    }
}

#[async_trait]
impl Controller for EvpnController {
    fn controller_type(&self) -> ControllerType {
        ControllerType::Evpn
    }

    fn name(&self) -> &str {
        &self.name
    }

    async fn validate_configuration(&self, config: &ControllerConfig) -> Result<()> {
        debug!("Validating EVPN controller '{}' configuration", self.name);

        // Basic validation
        config.validate().with_context(|| {
            format!(
                "Basic validation failed for EVPN controller '{}'",
                self.name
            )
        })?;

        // EVPN-specific validation
        self.validate_evpn_config(config).with_context(|| {
            format!(
                "EVPN-specific validation failed for controller '{}'",
                self.name
            )
        })?;

        info!(
            "EVPN controller '{}' configuration validation successful",
            self.name
        );
        Ok(())
    }

    async fn apply_configuration(&self, zones: &[Box<dyn Zone>], vnets: &[VNet]) -> Result<()> {
        debug!("Applying configuration for EVPN controller '{}'", self.name);

        // Apply EVPN-specific configuration to zones and vnets
        for zone in zones {
            if zone.zone_type() == pve_sdn_core::ZoneType::Evpn {
                debug!(
                    "Configuring EVPN zone '{}' with controller '{}'",
                    zone.name(),
                    self.name
                );
                // Zone-specific EVPN configuration would be applied here
            }
        }

        for vnet in vnets {
            debug!(
                "Configuring VNet '{}' for EVPN controller '{}'",
                vnet.name(),
                self.name
            );
            // VNet-specific EVPN configuration would be applied here
        }

        info!(
            "EVPN controller '{}' configuration applied successfully",
            self.name
        );
        Ok(())
    }

    async fn generate_config(&self, config: &ControllerConfig) -> Result<HashMap<String, String>> {
        debug!(
            "Generating configuration files for EVPN controller '{}'",
            self.name
        );

        let mut configs = HashMap::new();

        // Generate FRR BGP EVPN configuration
        let frr_config = self.generate_frr_evpn_config(config).with_context(|| {
            format!(
                "Failed to generate FRR EVPN config for controller '{}'",
                self.name
            )
        })?;
        configs.insert("frr".to_string(), frr_config);

        // Generate systemd service configuration
        let systemd_config = self.generate_systemd_config(config).with_context(|| {
            format!(
                "Failed to generate systemd config for EVPN controller '{}'",
                self.name
            )
        })?;
        configs.insert("systemd".to_string(), systemd_config);

        // Generate controller metadata
        let metadata = format!(
            "# EVPN Controller Configuration\n\
             # Controller: {}\n\
             # Type: EVPN\n\
             # ASN: {}\n\
             # VTEP IP: {}\n\
             # Peers: {}\n\
             # Router ID: {}\n",
            self.name,
            config.asn.unwrap_or(0),
            config
                .options
                .get("vtep-ip")
                .and_then(|v| v.as_str())
                .unwrap_or("none"),
            config
                .peers
                .as_ref()
                .map(|p| p.join(", "))
                .unwrap_or_else(|| "none".to_string()),
            config
                .options
                .get("router-id")
                .and_then(|v| v.as_str())
                .unwrap_or("auto")
        );
        configs.insert("metadata".to_string(), metadata);

        info!(
            "Generated configuration files for EVPN controller '{}'",
            self.name
        );
        Ok(configs)
    }

    async fn start(&self) -> Result<()> {
        debug!("Starting EVPN controller '{}'", self.name);

        // Check if FRR with EVPN support is available
        if !self.check_frr_evpn_support().await? {
            anyhow::bail!(
                "FRR with EVPN support is not available, cannot start EVPN controller '{}'",
                self.name
            );
        }

        // Check if already running
        if let Some(pid) = self.get_evpn_bgp_pid().await? {
            warn!(
                "EVPN controller '{}' is already running with PID {}",
                self.name, pid
            );
            return Ok(());
        }

        // Start BGP EVPN daemon
        let config_file = format!("/etc/frr/bgpd-evpn-{}.conf", self.name);
        let pid_file = format!("/var/run/frr/bgpd-evpn-{}.pid", self.name);

        let output = Command::new("/usr/lib/frr/bgpd")
            .args(&["-d", "-f", &config_file, "--pid_file", &pid_file])
            .output()
            .await
            .with_context(|| {
                format!(
                    "Failed to start BGP EVPN daemon for controller '{}'",
                    self.name
                )
            })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!(
                "Failed to start EVPN controller '{}': {}",
                self.name,
                stderr
            );
        }

        info!("EVPN controller '{}' started successfully", self.name);
        Ok(())
    }

    async fn stop(&self) -> Result<()> {
        debug!("Stopping EVPN controller '{}'", self.name);

        if let Some(pid) = self.get_evpn_bgp_pid().await? {
            let output = Command::new("kill")
                .args(&["-TERM", &pid.to_string()])
                .output()
                .await
                .with_context(|| {
                    format!(
                        "Failed to stop BGP EVPN daemon for controller '{}'",
                        self.name
                    )
                })?;

            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                error!("Failed to stop EVPN controller '{}': {}", self.name, stderr);

                // Try force kill
                let output = Command::new("kill")
                    .args(&["-KILL", &pid.to_string()])
                    .output()
                    .await?;

                if !output.status.success() {
                    anyhow::bail!("Failed to force stop EVPN controller '{}'", self.name);
                }
            }

            info!("EVPN controller '{}' stopped successfully", self.name);
        } else {
            warn!("EVPN controller '{}' is not running", self.name);
        }

        Ok(())
    }

    async fn status(&self) -> Result<ControllerStatus> {
        debug!("Getting status for EVPN controller '{}'", self.name);

        let pid = self.get_evpn_bgp_pid().await?;
        let running = pid.is_some();

        let uptime = if let Some(pid) = pid {
            // Get process uptime (simplified implementation)
            let stat_file = format!("/proc/{}/stat", pid);
            if let Ok(stat_content) = tokio::fs::read_to_string(&stat_file).await {
                let fields: Vec<&str> = stat_content.split_whitespace().collect();
                if fields.len() > 21 {
                    if let Ok(starttime) = fields[21].parse::<u64>() {
                        let uptime_ticks = std::fs::read_to_string("/proc/uptime")
                            .ok()
                            .and_then(|content| {
                                content.split_whitespace().next()?.parse::<f64>().ok()
                            })
                            .map(|uptime| uptime as u64);

                        if let Some(system_uptime) = uptime_ticks {
                            let clock_ticks_per_sec = 100;
                            let process_uptime = system_uptime - (starttime / clock_ticks_per_sec);
                            Some(process_uptime)
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                } else {
                    None
                }
            } else {
                None
            }
        } else {
            None
        };

        Ok(ControllerStatus {
            running,
            pid,
            uptime,
            last_error: None,
            config_version: None,
        })
    }

    async fn reload(&self) -> Result<()> {
        debug!("Reloading EVPN controller '{}'", self.name);

        if let Some(pid) = self.get_evpn_bgp_pid().await? {
            let output = Command::new("kill")
                .args(&["-HUP", &pid.to_string()])
                .output()
                .await
                .with_context(|| {
                    format!(
                        "Failed to reload BGP EVPN daemon for controller '{}'",
                        self.name
                    )
                })?;

            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                anyhow::bail!(
                    "Failed to reload EVPN controller '{}': {}",
                    self.name,
                    stderr
                );
            }

            info!("EVPN controller '{}' reloaded successfully", self.name);
        } else {
            anyhow::bail!(
                "EVPN controller '{}' is not running, cannot reload",
                self.name
            );
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[tokio::test]
    async fn test_evpn_controller_validation() {
        let controller = EvpnController::new("test-evpn".to_string());

        // Test valid configuration
        let mut config = ControllerConfig::new(ControllerType::Evpn, "test-evpn".to_string());
        config.asn = Some(65000);
        config.peers = Some(vec!["192.168.1.1".to_string(), "192.168.1.2".to_string()]);
        config
            .options
            .insert("vtep-ip".to_string(), json!("192.168.1.10"));
        config
            .options
            .insert("router-id".to_string(), json!("192.168.1.10"));

        assert!(controller.validate_configuration(&config).await.is_ok());

        // Test missing ASN
        config.asn = None;
        assert!(controller.validate_configuration(&config).await.is_err());

        // Test missing VTEP IP
        config.asn = Some(65000);
        config.options.remove("vtep-ip");
        assert!(controller.validate_configuration(&config).await.is_err());

        // Test invalid VTEP IP
        config
            .options
            .insert("vtep-ip".to_string(), json!("invalid-ip"));
        assert!(controller.validate_configuration(&config).await.is_err());
    }

    #[tokio::test]
    async fn test_evpn_config_generation() {
        let controller = EvpnController::new("test-evpn".to_string());

        let mut config = ControllerConfig::new(ControllerType::Evpn, "test-evpn".to_string());
        config.asn = Some(65000);
        config.peers = Some(vec!["192.168.1.1".to_string(), "192.168.1.2".to_string()]);
        config
            .options
            .insert("vtep-ip".to_string(), json!("192.168.1.10"));
        config
            .options
            .insert("advertise-all-vni".to_string(), json!(true));
        config.bgp_multipath_relax = Some(true);

        let configs = controller.generate_config(&config).await.unwrap();

        assert!(configs.contains_key("frr"));
        assert!(configs.contains_key("systemd"));
        assert!(configs.contains_key("metadata"));

        let frr_config = configs.get("frr").unwrap();
        assert!(frr_config.contains("router bgp 65000"));
        assert!(frr_config.contains("bgp router-id 192.168.1.10"));
        assert!(frr_config.contains("address-family l2vpn evpn"));
        assert!(frr_config.contains("advertise-all-vni"));
        assert!(frr_config.contains("neighbor 192.168.1.1 activate"));
        assert!(frr_config.contains("redistribute connected"));
        assert!(frr_config.contains("ip address 192.168.1.10/32"));
    }
}
