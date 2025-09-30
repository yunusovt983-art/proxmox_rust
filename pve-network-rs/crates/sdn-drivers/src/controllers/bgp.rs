//! BGP controller driver
//!
//! BGP controller provides basic BGP routing functionality for SDN zones.
//! This controller manages FRR BGP daemon configuration and routing policies.

use anyhow::{Context, Result};
use async_trait::async_trait;
use log::{debug, error, info, warn};
use pve_sdn_core::controller::{ControllerConfig, ControllerStatus};
use pve_sdn_core::{Controller, ControllerType, VNet, Zone};
use std::collections::HashMap;
use std::net::IpAddr;
use std::path::Path;
use tokio::process::Command;

/// BGP controller implementation
///
/// The BGP controller manages FRR BGP daemon for basic routing functionality.
/// Key features:
/// - BGP peering configuration
/// - Route redistribution
/// - BGP communities and route-maps
/// - Integration with Linux routing table
pub struct BgpController {
    name: String,
}

impl BgpController {
    /// Create new BGP controller
    pub fn new(name: String) -> Self {
        Self { name }
    }

    /// Validate BGP-specific configuration
    fn validate_bgp_config(&self, config: &ControllerConfig) -> Result<()> {
        // BGP requires an ASN
        if config.asn.is_none() {
            anyhow::bail!("BGP controller '{}' requires an ASN", self.name);
        }

        let asn = config.asn.unwrap();
        if asn == 0 || asn > 4294967295 {
            anyhow::bail!(
                "BGP controller '{}' ASN must be between 1 and 4294967295",
                self.name
            );
        }

        // Validate peers if specified
        if let Some(peers) = &config.peers {
            for peer in peers {
                let _addr: IpAddr = peer.parse().with_context(|| {
                    format!(
                        "Invalid peer address '{}' for BGP controller '{}'",
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
                        "Invalid router-id '{}' for BGP controller '{}'",
                        router_id_str, self.name
                    )
                })?;
            }
        }

        Ok(())
    }

    /// Generate FRR BGP configuration
    fn generate_frr_bgp_config(&self, config: &ControllerConfig) -> Result<String> {
        let asn = config.asn.unwrap();

        let mut bgp_config = format!(
            "!\n\
             ! BGP configuration for controller {}\n\
             !\n\
             router bgp {asn}\n",
            self.name,
            asn = asn
        );

        // Add router-id if specified
        if let Some(router_id) = config.options.get("router-id") {
            if let Some(router_id_str) = router_id.as_str() {
                bgp_config.push_str(&format!(" bgp router-id {}\n", router_id_str));
            }
        }

        // BGP configuration options
        if let Some(true) = config.bgp_multipath_relax {
            bgp_config.push_str(" bgp bestpath as-path multipath-relax\n");
        }

        if let Some(false) = config.ebgp_requires_policy {
            bgp_config.push_str(" no bgp ebgp-requires-policy\n");
        }

        // Add BGP neighbors
        if let Some(peers) = &config.peers {
            for peer in peers {
                bgp_config.push_str(&format!(" neighbor {} remote-as external\n", peer));
                bgp_config.push_str(&format!(" neighbor {} capability extended-nexthop\n", peer));

                // Add peer-specific options
                if let Some(peer_options) = config.options.get(&format!("peer-{}", peer)) {
                    if let Some(peer_obj) = peer_options.as_object() {
                        if let Some(remote_asn) = peer_obj.get("remote-as") {
                            if let Some(remote_asn_num) = remote_asn.as_u64() {
                                bgp_config = bgp_config.replace(
                                    &format!(" neighbor {} remote-as external\n", peer),
                                    &format!(" neighbor {} remote-as {}\n", peer, remote_asn_num),
                                );
                            }
                        }

                        if let Some(description) = peer_obj.get("description") {
                            if let Some(desc_str) = description.as_str() {
                                bgp_config.push_str(&format!(
                                    " neighbor {} description {}\n",
                                    peer, desc_str
                                ));
                            }
                        }

                        if let Some(password) = peer_obj.get("password") {
                            if let Some(pass_str) = password.as_str() {
                                bgp_config.push_str(&format!(
                                    " neighbor {} password {}\n",
                                    peer, pass_str
                                ));
                            }
                        }
                    }
                }
            }
        }

        // Address family configuration
        bgp_config.push_str(" !\n address-family ipv4 unicast\n");

        if let Some(peers) = &config.peers {
            for peer in peers {
                bgp_config.push_str(&format!("  neighbor {} activate\n", peer));
            }
        }

        // Network advertisements
        if let Some(networks) = config.options.get("networks") {
            if let Some(networks_array) = networks.as_array() {
                for network in networks_array {
                    if let Some(network_str) = network.as_str() {
                        bgp_config.push_str(&format!("  network {}\n", network_str));
                    }
                }
            }
        }

        // Route redistribution
        if let Some(redistribute) = config.options.get("redistribute") {
            if let Some(redistribute_array) = redistribute.as_array() {
                for proto in redistribute_array {
                    if let Some(proto_str) = proto.as_str() {
                        bgp_config.push_str(&format!("  redistribute {}\n", proto_str));
                    }
                }
            }
        }

        bgp_config.push_str(" exit-address-family\n");

        // IPv6 address family if enabled
        if let Some(true) = config.options.get("ipv6").and_then(|v| v.as_bool()) {
            bgp_config.push_str(" !\n address-family ipv6 unicast\n");

            if let Some(peers) = &config.peers {
                for peer in peers {
                    bgp_config.push_str(&format!("  neighbor {} activate\n", peer));
                }
            }

            bgp_config.push_str(" exit-address-family\n");
        }

        bgp_config.push_str("!\n");

        Ok(bgp_config)
    }

    /// Generate systemd service configuration
    fn generate_systemd_config(&self, _config: &ControllerConfig) -> Result<String> {
        let systemd_config = format!(
            "[Unit]\n\
             Description=FRR BGP daemon for controller {}\n\
             After=network.target\n\
             Wants=network.target\n\
             \n\
             [Service]\n\
             Type=forking\n\
             ExecStart=/usr/lib/frr/bgpd -d -f /etc/frr/bgpd-{}.conf --pid_file /var/run/frr/bgpd-{}.pid\n\
             ExecReload=/bin/kill -HUP $MAINPID\n\
             PIDFile=/var/run/frr/bgpd-{}.pid\n\
             Restart=on-failure\n\
             \n\
             [Install]\n\
             WantedBy=multi-user.target\n",
            self.name, self.name, self.name, self.name
        );

        Ok(systemd_config)
    }

    /// Check if FRR is installed
    async fn check_frr_installed(&self) -> Result<bool> {
        let output = Command::new("which").arg("bgpd").output().await?;

        Ok(output.status.success())
    }

    /// Get BGP daemon PID
    async fn get_bgp_pid(&self) -> Result<Option<u32>> {
        let pid_file = format!("/var/run/frr/bgpd-{}.pid", self.name);

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
impl Controller for BgpController {
    fn controller_type(&self) -> ControllerType {
        ControllerType::Bgp
    }

    fn name(&self) -> &str {
        &self.name
    }

    async fn validate_configuration(&self, config: &ControllerConfig) -> Result<()> {
        debug!("Validating BGP controller '{}' configuration", self.name);

        // Basic validation
        config.validate().with_context(|| {
            format!("Basic validation failed for BGP controller '{}'", self.name)
        })?;

        // BGP-specific validation
        self.validate_bgp_config(config).with_context(|| {
            format!(
                "BGP-specific validation failed for controller '{}'",
                self.name
            )
        })?;

        info!(
            "BGP controller '{}' configuration validation successful",
            self.name
        );
        Ok(())
    }

    async fn apply_configuration(&self, _zones: &[Box<dyn Zone>], _vnets: &[VNet]) -> Result<()> {
        debug!("Applying configuration for BGP controller '{}'", self.name);

        // BGP controller doesn't directly configure zones/vnets
        // It provides routing services that zones can use

        info!(
            "BGP controller '{}' configuration applied successfully",
            self.name
        );
        Ok(())
    }

    async fn generate_config(&self, config: &ControllerConfig) -> Result<HashMap<String, String>> {
        debug!(
            "Generating configuration files for BGP controller '{}'",
            self.name
        );

        let mut configs = HashMap::new();

        // Generate FRR BGP configuration
        let frr_config = self.generate_frr_bgp_config(config).with_context(|| {
            format!(
                "Failed to generate FRR config for BGP controller '{}'",
                self.name
            )
        })?;
        configs.insert("frr".to_string(), frr_config);

        // Generate systemd service configuration
        let systemd_config = self.generate_systemd_config(config).with_context(|| {
            format!(
                "Failed to generate systemd config for BGP controller '{}'",
                self.name
            )
        })?;
        configs.insert("systemd".to_string(), systemd_config);

        // Generate controller metadata
        let metadata = format!(
            "# BGP Controller Configuration\n\
             # Controller: {}\n\
             # Type: BGP\n\
             # ASN: {}\n\
             # Peers: {}\n\
             # Router ID: {}\n",
            self.name,
            config.asn.unwrap_or(0),
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
            "Generated configuration files for BGP controller '{}'",
            self.name
        );
        Ok(configs)
    }

    async fn start(&self) -> Result<()> {
        debug!("Starting BGP controller '{}'", self.name);

        // Check if FRR is installed
        if !self.check_frr_installed().await? {
            anyhow::bail!(
                "FRR is not installed, cannot start BGP controller '{}'",
                self.name
            );
        }

        // Check if already running
        if let Some(pid) = self.get_bgp_pid().await? {
            warn!(
                "BGP controller '{}' is already running with PID {}",
                self.name, pid
            );
            return Ok(());
        }

        // Start BGP daemon
        let config_file = format!("/etc/frr/bgpd-{}.conf", self.name);
        let pid_file = format!("/var/run/frr/bgpd-{}.pid", self.name);

        let output = Command::new("/usr/lib/frr/bgpd")
            .args(&["-d", "-f", &config_file, "--pid_file", &pid_file])
            .output()
            .await
            .with_context(|| {
                format!("Failed to start BGP daemon for controller '{}'", self.name)
            })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("Failed to start BGP controller '{}': {}", self.name, stderr);
        }

        info!("BGP controller '{}' started successfully", self.name);
        Ok(())
    }

    async fn stop(&self) -> Result<()> {
        debug!("Stopping BGP controller '{}'", self.name);

        if let Some(pid) = self.get_bgp_pid().await? {
            let output = Command::new("kill")
                .args(&["-TERM", &pid.to_string()])
                .output()
                .await
                .with_context(|| {
                    format!("Failed to stop BGP daemon for controller '{}'", self.name)
                })?;

            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                error!("Failed to stop BGP controller '{}': {}", self.name, stderr);

                // Try force kill
                let output = Command::new("kill")
                    .args(&["-KILL", &pid.to_string()])
                    .output()
                    .await?;

                if !output.status.success() {
                    anyhow::bail!("Failed to force stop BGP controller '{}'", self.name);
                }
            }

            info!("BGP controller '{}' stopped successfully", self.name);
        } else {
            warn!("BGP controller '{}' is not running", self.name);
        }

        Ok(())
    }

    async fn status(&self) -> Result<ControllerStatus> {
        debug!("Getting status for BGP controller '{}'", self.name);

        let pid = self.get_bgp_pid().await?;
        let running = pid.is_some();

        let uptime = if let Some(pid) = pid {
            // Get process uptime
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
                            let clock_ticks_per_sec = 100; // Usually 100 on Linux
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
        debug!("Reloading BGP controller '{}'", self.name);

        if let Some(pid) = self.get_bgp_pid().await? {
            let output = Command::new("kill")
                .args(&["-HUP", &pid.to_string()])
                .output()
                .await
                .with_context(|| {
                    format!("Failed to reload BGP daemon for controller '{}'", self.name)
                })?;

            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                anyhow::bail!(
                    "Failed to reload BGP controller '{}': {}",
                    self.name,
                    stderr
                );
            }

            info!("BGP controller '{}' reloaded successfully", self.name);
        } else {
            anyhow::bail!(
                "BGP controller '{}' is not running, cannot reload",
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
    async fn test_bgp_controller_validation() {
        let controller = BgpController::new("test-bgp".to_string());

        // Test valid configuration
        let mut config = ControllerConfig::new(ControllerType::Bgp, "test-bgp".to_string());
        config.asn = Some(65000);
        config.peers = Some(vec!["192.168.1.1".to_string(), "192.168.1.2".to_string()]);
        config
            .options
            .insert("router-id".to_string(), json!("192.168.1.10"));

        assert!(controller.validate_configuration(&config).await.is_ok());

        // Test missing ASN
        config.asn = None;
        assert!(controller.validate_configuration(&config).await.is_err());

        // Test invalid ASN
        config.asn = Some(0);
        assert!(controller.validate_configuration(&config).await.is_err());

        // Test invalid peer
        config.asn = Some(65000);
        config.peers = Some(vec!["invalid-ip".to_string()]);
        assert!(controller.validate_configuration(&config).await.is_err());

        // Test invalid router-id
        config.peers = Some(vec!["192.168.1.1".to_string()]);
        config
            .options
            .insert("router-id".to_string(), json!("invalid-ip"));
        assert!(controller.validate_configuration(&config).await.is_err());
    }

    #[tokio::test]
    async fn test_bgp_config_generation() {
        let controller = BgpController::new("test-bgp".to_string());

        let mut config = ControllerConfig::new(ControllerType::Bgp, "test-bgp".to_string());
        config.asn = Some(65000);
        config.peers = Some(vec!["192.168.1.1".to_string(), "192.168.1.2".to_string()]);
        config
            .options
            .insert("router-id".to_string(), json!("192.168.1.10"));
        config.options.insert(
            "networks".to_string(),
            json!(["10.0.0.0/24", "10.1.0.0/24"]),
        );
        config.bgp_multipath_relax = Some(true);

        let configs = controller.generate_config(&config).await.unwrap();

        assert!(configs.contains_key("frr"));
        assert!(configs.contains_key("systemd"));
        assert!(configs.contains_key("metadata"));

        let frr_config = configs.get("frr").unwrap();
        assert!(frr_config.contains("router bgp 65000"));
        assert!(frr_config.contains("bgp router-id 192.168.1.10"));
        assert!(frr_config.contains("neighbor 192.168.1.1 remote-as external"));
        assert!(frr_config.contains("neighbor 192.168.1.2 remote-as external"));
        assert!(frr_config.contains("bgp bestpath as-path multipath-relax"));
        assert!(frr_config.contains("network 10.0.0.0/24"));
        assert!(frr_config.contains("network 10.1.0.0/24"));
    }
}
