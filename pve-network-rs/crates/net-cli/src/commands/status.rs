//! Status command

use anyhow::{Context, Result};
use pve_network_api::context::AppContext;
use pve_network_config::InterfacesParser;
use pve_network_core::{AddressMethod, InterfaceType};
use serde_json::Value;
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::process::Command;
use std::sync::Arc;

/// Status command implementation
pub struct StatusCommand {
    context: Arc<AppContext>,
    parser: InterfacesParser,
}

impl StatusCommand {
    /// Create new status command
    pub fn new(context: Arc<AppContext>) -> Self {
        Self {
            context,
            parser: InterfacesParser::new(),
        }
    }

    /// Execute status command
    pub async fn execute(&self, verbose: bool) -> Result<()> {
        if verbose {
            println!("Detailed network status:");
            self.show_detailed_status().await
        } else {
            println!("Network status:");
            self.show_basic_status().await
        }
    }

    /// Show basic network status
    async fn show_basic_status(&self) -> Result<()> {
        // Read current configuration
        let config_path = "/etc/network/interfaces";
        let config = self.read_configuration(config_path).await?;

        // Get system interface status
        let system_interfaces = self.get_system_interfaces().await?;

        println!(
            "{:<15} {:<10} {:<15} {:<20} {}",
            "Interface", "Type", "Method", "Address", "Status"
        );
        println!("{}", "-".repeat(80));

        for (name, iface) in &config.interfaces {
            let iface_type = match &iface.iface_type {
                InterfaceType::Physical => "physical",
                InterfaceType::Bridge { .. } => "bridge",
                InterfaceType::Bond { .. } => "bond",
                InterfaceType::Vlan { .. } => "vlan",
                InterfaceType::Vxlan { .. } => "vxlan",
                InterfaceType::Loopback => "loopback",
            };

            let method = match &iface.method {
                AddressMethod::Static => "static",
                AddressMethod::Dhcp => "dhcp",
                AddressMethod::Manual => "manual",
                AddressMethod::None => "none",
            };

            let address = iface
                .addresses
                .first()
                .map(|addr| addr.to_string())
                .unwrap_or_else(|| "-".to_string());

            let status = if system_interfaces.contains_key(name) {
                if system_interfaces[name]
                    .get("operstate")
                    .and_then(|v| v.as_str())
                    == Some("up")
                {
                    "UP"
                } else {
                    "DOWN"
                }
            } else {
                "NOT_FOUND"
            };

            println!(
                "{:<15} {:<10} {:<15} {:<20} {}",
                name, iface_type, method, address, status
            );
        }

        self.show_migration_metrics().await?;
        Ok(())
    }

    /// Show detailed network status
    async fn show_detailed_status(&self) -> Result<()> {
        // Read current configuration
        let config_path = "/etc/network/interfaces";
        let config = self.read_configuration(config_path).await?;

        // Get system interface status
        let system_interfaces = self.get_system_interfaces().await?;

        println!("=== Network Configuration Status ===");
        println!("Configuration file: {}", config_path);

        if Path::new(config_path).exists() {
            let metadata = fs::metadata(config_path)?;
            let modified = metadata.modified()?;
            println!("Last modified: {:?}", modified);
        }

        println!("\n=== Configured Interfaces ===");
        for (name, iface) in &config.interfaces {
            println!("\nInterface: {}", name);
            println!("  Type: {:?}", iface.iface_type);
            println!("  Method: {:?}", iface.method);

            if !iface.addresses.is_empty() {
                println!("  Addresses:");
                for addr in &iface.addresses {
                    println!("    {}", addr);
                }
            }

            if let Some(gateway) = &iface.gateway {
                println!("  Gateway: {}", gateway);
            }

            if let Some(mtu) = iface.mtu {
                println!("  MTU: {}", mtu);
            }

            if !iface.options.is_empty() {
                println!("  Options:");
                for (key, value) in &iface.options {
                    println!("    {}: {}", key, value);
                }
            }

            // Show system status
            if let Some(sys_info) = system_interfaces.get(name) {
                println!("  System Status:");
                if let Some(operstate) = sys_info.get("operstate").and_then(|v| v.as_str()) {
                    println!("    Operational State: {}", operstate);
                }
                if let Some(carrier) = sys_info.get("carrier").and_then(|v| v.as_str()) {
                    println!("    Carrier: {}", carrier);
                }
                if let Some(mtu) = sys_info.get("mtu").and_then(|v| v.as_u64()) {
                    println!("    System MTU: {}", mtu);
                }
                if let Some(mac) = sys_info.get("address").and_then(|v| v.as_str()) {
                    println!("    MAC Address: {}", mac);
                }
            } else {
                println!("  System Status: Interface not found in system");
            }
        }

        // Show system interfaces not in configuration
        println!("\n=== System Interfaces Not in Configuration ===");
        for (name, info) in &system_interfaces {
            if !config.interfaces.contains_key(name) && !name.starts_with("lo") {
                println!("Interface: {}", name);
                if let Some(operstate) = info.get("operstate").and_then(|v| v.as_str()) {
                    println!("  State: {}", operstate);
                }
                if let Some(mac) = info.get("address").and_then(|v| v.as_str()) {
                    println!("  MAC: {}", mac);
                }
            }
        }

        // Show bridge information
        self.show_bridge_status().await?;

        // Show bond information
        self.show_bond_status().await?;

        self.show_migration_metrics().await?;
        Ok(())
    }

    async fn show_migration_metrics(&self) -> Result<()> {
        let phase = self.context.migration_hooks.current_phase().await;
        let applied = self.context.migration_hooks.network_applied_count().await;

        println!("\nMigration status:");
        println!("  Current phase: {:?}", phase);
        println!("  Network apply events observed: {}", applied);
        Ok(())
    }

    /// Read network configuration
    async fn read_configuration(
        &self,
        config_path: &str,
    ) -> Result<pve_network_core::NetworkConfiguration> {
        if !Path::new(config_path).exists() {
            anyhow::bail!("Configuration file not found: {}", config_path);
        }

        let content = fs::read_to_string(config_path)
            .with_context(|| format!("Failed to read configuration file: {}", config_path))?;

        self.parser
            .parse(&content)
            .with_context(|| "Failed to parse network configuration")
    }

    /// Get system interface information
    async fn get_system_interfaces(&self) -> Result<HashMap<String, Value>> {
        let output = Command::new("ip")
            .args(&["-j", "link", "show"])
            .output()
            .with_context(|| "Failed to execute 'ip link show'")?;

        if !output.status.success() {
            anyhow::bail!(
                "ip command failed: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }

        let json_str = String::from_utf8(output.stdout)
            .with_context(|| "Failed to parse ip command output as UTF-8")?;

        let interfaces: Vec<Value> = serde_json::from_str(&json_str)
            .with_context(|| "Failed to parse ip command JSON output")?;

        let mut result = HashMap::new();
        for iface in interfaces {
            if let Some(name) = iface.get("ifname").and_then(|v| v.as_str()) {
                result.insert(name.to_string(), iface);
            }
        }

        Ok(result)
    }

    /// Show bridge status
    async fn show_bridge_status(&self) -> Result<()> {
        let output = Command::new("brctl").args(&["show"]).output();

        match output {
            Ok(output) if output.status.success() => {
                let bridge_info = String::from_utf8_lossy(&output.stdout);
                if !bridge_info.trim().is_empty() {
                    println!("\n=== Bridge Status ===");
                    println!("{}", bridge_info);
                }
            }
            _ => {
                // brctl might not be available, try ip command
                let output = Command::new("ip")
                    .args(&["-d", "link", "show", "type", "bridge"])
                    .output();

                if let Ok(output) = output {
                    if output.status.success() {
                        let bridge_info = String::from_utf8_lossy(&output.stdout);
                        if !bridge_info.trim().is_empty() {
                            println!("\n=== Bridge Status ===");
                            println!("{}", bridge_info);
                        }
                    }
                }
            }
        }

        Ok(())
    }

    /// Show bond status
    async fn show_bond_status(&self) -> Result<()> {
        let bond_dir = "/proc/net/bonding";
        if !Path::new(bond_dir).exists() {
            return Ok(());
        }

        let entries = fs::read_dir(bond_dir);
        if let Ok(entries) = entries {
            let bonds: Vec<_> = entries
                .filter_map(|entry| entry.ok())
                .filter_map(|entry| entry.file_name().to_str().map(|s| s.to_string()))
                .collect();

            if !bonds.is_empty() {
                println!("\n=== Bond Status ===");
                for bond in bonds {
                    let bond_info_path = format!("{}/{}", bond_dir, bond);
                    if let Ok(content) = fs::read_to_string(&bond_info_path) {
                        println!("Bond: {}", bond);
                        // Show first few lines of bond info
                        for line in content.lines().take(10) {
                            if line.trim().starts_with("Bonding Mode:")
                                || line.trim().starts_with("MII Status:")
                                || line.trim().starts_with("Slave Interface:")
                            {
                                println!("  {}", line.trim());
                            }
                        }
                        println!();
                    }
                }
            }
        }

        Ok(())
    }

    /// Show interface statistics
    pub async fn show_statistics(&self, interface: Option<&str>) -> Result<()> {
        match interface {
            Some(iface) => {
                println!("Statistics for interface: {}", iface);
                self.show_interface_stats(iface).await?;
            }
            None => {
                println!("Network interface statistics:");
                let system_interfaces = self.get_system_interfaces().await?;
                for name in system_interfaces.keys() {
                    if !name.starts_with("lo") {
                        self.show_interface_stats(name).await?;
                    }
                }
            }
        }
        Ok(())
    }

    /// Show statistics for specific interface
    async fn show_interface_stats(&self, interface: &str) -> Result<()> {
        let stats_path = format!("/sys/class/net/{}/statistics", interface);
        if !Path::new(&stats_path).exists() {
            println!("  {}: No statistics available", interface);
            return Ok(());
        }

        println!("  {}:", interface);

        // Read common statistics
        let stats = [
            ("rx_bytes", "RX bytes"),
            ("tx_bytes", "TX bytes"),
            ("rx_packets", "RX packets"),
            ("tx_packets", "TX packets"),
            ("rx_errors", "RX errors"),
            ("tx_errors", "TX errors"),
            ("rx_dropped", "RX dropped"),
            ("tx_dropped", "TX dropped"),
        ];

        for (file, label) in &stats {
            let stat_file = format!("{}/{}", stats_path, file);
            if let Ok(content) = fs::read_to_string(&stat_file) {
                if let Ok(value) = content.trim().parse::<u64>() {
                    println!("    {}: {}", label, value);
                }
            }
        }

        Ok(())
    }
}
