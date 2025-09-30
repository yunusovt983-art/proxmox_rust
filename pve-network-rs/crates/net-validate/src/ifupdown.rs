//! ifupdown2 validation integration

use pve_network_core::error::{SystemError, ValidationError};
use pve_network_core::{NetworkConfiguration, NetworkError};
use std::io::Write;
use std::path::Path;
use std::process::Command;
use tempfile::NamedTempFile;

/// ifupdown2 validator for dry-run validation
pub struct IfUpDownValidator {
    ifupdown2_path: String,
}

impl IfUpDownValidator {
    /// Create new ifupdown2 validator
    pub fn new() -> Self {
        Self {
            ifupdown2_path: "/sbin/ifup".to_string(),
        }
    }

    /// Create validator with custom ifupdown2 path
    pub fn with_path<P: AsRef<Path>>(path: P) -> Self {
        Self {
            ifupdown2_path: path.as_ref().to_string_lossy().to_string(),
        }
    }

    /// Validate configuration using ifupdown2 dry-run
    pub fn validate_configuration(
        &self,
        config: &NetworkConfiguration,
    ) -> Result<(), NetworkError> {
        // Check if ifupdown2 is available
        if !self.is_ifupdown2_available() {
            log::warn!("ifupdown2 not available, skipping dry-run validation");
            return Ok(());
        }

        // Generate interfaces file content
        let interfaces_content = self.generate_interfaces_content(config)?;

        // Create temporary file
        let mut temp_file = NamedTempFile::new().map_err(|e| {
            NetworkError::System(SystemError::FileOperation {
                path: "temp_interfaces".to_string(),
            })
        })?;

        // Write content to temporary file
        temp_file
            .write_all(interfaces_content.as_bytes())
            .map_err(|e| {
                NetworkError::System(SystemError::FileOperation {
                    path: temp_file.path().to_string_lossy().to_string(),
                })
            })?;

        // Run ifupdown2 dry-run validation
        self.run_dry_run_validation(temp_file.path())?;

        Ok(())
    }

    /// Check if ifupdown2 is available on the system
    fn is_ifupdown2_available(&self) -> bool {
        Path::new(&self.ifupdown2_path).exists()
            || Command::new("which")
                .arg("ifup")
                .output()
                .map(|output| output.status.success())
                .unwrap_or(false)
    }

    /// Generate interfaces file content from configuration
    fn generate_interfaces_content(
        &self,
        config: &NetworkConfiguration,
    ) -> Result<String, NetworkError> {
        let mut content = String::new();

        // Add header comment
        content.push_str("# Generated interfaces file for validation\n");
        content.push_str("# This file is used for ifupdown2 dry-run validation only\n\n");

        // Add auto interfaces
        if !config.auto_interfaces.is_empty() {
            content.push_str("auto ");
            content.push_str(&config.auto_interfaces.join(" "));
            content.push('\n');
        }

        // Add hotplug interfaces
        if !config.hotplug_interfaces.is_empty() {
            content.push_str("allow-hotplug ");
            content.push_str(&config.hotplug_interfaces.join(" "));
            content.push('\n');
        }

        content.push('\n');

        // Add interface definitions in order
        let mut processed = std::collections::HashSet::new();

        // Process interfaces in dependency order
        for interface_name in &config.ordering {
            if let Some(interface) = config.interfaces.get(interface_name) {
                if !processed.contains(interface_name) {
                    content.push_str(&self.generate_interface_config(interface)?);
                    content.push('\n');
                    processed.insert(interface_name.clone());
                }
            }
        }

        // Process any remaining interfaces not in ordering
        for (name, interface) in &config.interfaces {
            if !processed.contains(name) {
                content.push_str(&self.generate_interface_config(interface)?);
                content.push('\n');
            }
        }

        Ok(content)
    }

    /// Generate configuration for a single interface
    fn generate_interface_config(
        &self,
        interface: &pve_network_core::Interface,
    ) -> Result<String, NetworkError> {
        let mut config = String::new();

        // Add comments
        for comment in &interface.comments {
            config.push_str(&format!("# {}\n", comment));
        }

        // Interface declaration
        let method_str = match interface.method {
            pve_network_core::AddressMethod::Static => "static",
            pve_network_core::AddressMethod::Dhcp => "dhcp",
            pve_network_core::AddressMethod::Manual => "manual",
            pve_network_core::AddressMethod::None => "none",
        };

        config.push_str(&format!("iface {} inet {}\n", interface.name, method_str));

        // Add addresses for static method
        if interface.method == pve_network_core::AddressMethod::Static {
            if let Some(first_addr) = interface.addresses.first() {
                config.push_str(&format!("    address {}\n", first_addr));
            }

            // Add additional addresses
            for addr in interface.addresses.iter().skip(1) {
                config.push_str(&format!(
                    "    up ip addr add {} dev {}\n",
                    addr, interface.name
                ));
                config.push_str(&format!(
                    "    down ip addr del {} dev {}\n",
                    addr, interface.name
                ));
            }
        }

        // Add gateway
        if let Some(gateway) = &interface.gateway {
            config.push_str(&format!("    gateway {}\n", gateway.addr));
        }

        // Add MTU
        if let Some(mtu) = interface.mtu {
            config.push_str(&format!("    mtu {}\n", mtu));
        }

        // Add interface type specific configuration
        match &interface.iface_type {
            pve_network_core::InterfaceType::Bridge { ports, vlan_aware } => {
                if !ports.is_empty() {
                    config.push_str(&format!("    bridge-ports {}\n", ports.join(" ")));
                }
                if *vlan_aware {
                    config.push_str("    bridge-vlan-aware yes\n");
                }
                config.push_str("    bridge-stp off\n");
                config.push_str("    bridge-fd 0\n");
            }
            pve_network_core::InterfaceType::Bond {
                slaves,
                mode,
                options,
            } => {
                if !slaves.is_empty() {
                    config.push_str(&format!("    bond-slaves {}\n", slaves.join(" ")));
                }
                config.push_str(&format!(
                    "    bond-mode {}\n",
                    self.bond_mode_to_string(mode)
                ));

                for (key, value) in options {
                    config.push_str(&format!("    bond-{} {}\n", key, value));
                }
            }
            pve_network_core::InterfaceType::Vlan { parent, tag } => {
                config.push_str(&format!("    vlan-raw-device {}\n", parent));
                config.push_str(&format!("    vlan-id {}\n", tag));
            }
            pve_network_core::InterfaceType::Vxlan {
                id,
                local,
                remote,
                dstport,
            } => {
                config.push_str(&format!("    vxlan-id {}\n", id));
                config.push_str(&format!("    vxlan-local-tunnelip {}\n", local.addr));
                if let Some(remote_ip) = remote {
                    config.push_str(&format!("    vxlan-remoteip {}\n", remote_ip.addr));
                }
                if let Some(port) = dstport {
                    config.push_str(&format!("    vxlan-port {}\n", port));
                }
            }
            _ => {} // Physical and Loopback don't need special config
        }

        // Add custom options
        for (key, value) in &interface.options {
            // Skip options that are already handled above
            if !matches!(
                key.as_str(),
                "bridge-ports"
                    | "bridge-vlan-aware"
                    | "bond-slaves"
                    | "bond-mode"
                    | "vlan-raw-device"
                    | "vlan-id"
            ) {
                config.push_str(&format!("    {} {}\n", key, value));
            }
        }

        Ok(config)
    }

    /// Convert bond mode to string representation
    fn bond_mode_to_string(&self, mode: &pve_network_core::BondMode) -> &'static str {
        match mode {
            pve_network_core::BondMode::RoundRobin => "balance-rr",
            pve_network_core::BondMode::ActiveBackup => "active-backup",
            pve_network_core::BondMode::Xor => "balance-xor",
            pve_network_core::BondMode::Broadcast => "broadcast",
            pve_network_core::BondMode::Ieee8023ad => "802.3ad",
            pve_network_core::BondMode::BalanceTlb => "balance-tlb",
            pve_network_core::BondMode::BalanceAlb => "balance-alb",
        }
    }

    /// Run ifupdown2 dry-run validation
    fn run_dry_run_validation(&self, interfaces_file: &Path) -> Result<(), NetworkError> {
        // Try different ifupdown2 commands for dry-run validation
        let commands = vec![
            vec!["ifquery", "--syntax-check", "--interfaces"],
            vec!["ifup", "--no-act", "--all", "--interfaces"],
            vec!["ifreload", "--dry-run", "--interfaces"],
        ];

        let mut last_error = None;

        for cmd_args in commands {
            let mut cmd = Command::new(&cmd_args[0]);

            // Add arguments
            for arg in &cmd_args[1..] {
                cmd.arg(arg);
            }

            // Add interfaces file path
            cmd.arg(interfaces_file.to_string_lossy().as_ref());

            match cmd.output() {
                Ok(output) => {
                    if output.status.success() {
                        // Validation passed
                        return Ok(());
                    } else {
                        // Parse error output to provide meaningful messages
                        let stderr = String::from_utf8_lossy(&output.stderr);
                        let stdout = String::from_utf8_lossy(&output.stdout);

                        let error_msg = if !stderr.is_empty() {
                            stderr.to_string()
                        } else {
                            stdout.to_string()
                        };

                        // Try to make error messages compatible with Perl version
                        let formatted_error = self.format_error_message(&error_msg);

                        last_error = Some(NetworkError::Validation(ValidationError::IfUpDown {
                            output: formatted_error,
                        }));
                    }
                }
                Err(e) => {
                    // Command not found or execution failed, try next command
                    continue;
                }
            }
        }

        // If we get here, all commands failed
        match last_error {
            Some(error) => Err(error),
            None => {
                log::warn!("No suitable ifupdown2 command found for validation");
                Ok(()) // Don't fail if ifupdown2 is not available
            }
        }
    }

    /// Format error messages to be compatible with Perl version
    fn format_error_message(&self, error: &str) -> String {
        let mut formatted = error.to_string();

        // Common error message transformations to match Perl version
        formatted = formatted.replace("error:", "ERROR:");
        formatted = formatted.replace("warning:", "WARNING:");

        // Remove temporary file paths from error messages
        if let Some(pos) = formatted.find("/tmp/") {
            if let Some(end) = formatted[pos..].find(' ') {
                formatted.replace_range(pos..pos + end, "<interfaces>");
            }
        }

        // Clean up common ifupdown2 specific messages
        formatted = formatted.replace("ifupdown2:", "");
        formatted = formatted.trim().to_string();

        formatted
    }

    /// Validate specific interface using ifupdown2
    pub fn validate_interface(
        &self,
        interface: &pve_network_core::Interface,
    ) -> Result<(), NetworkError> {
        // Create minimal configuration with just this interface
        let mut config = NetworkConfiguration::default();
        config
            .interfaces
            .insert(interface.name.clone(), interface.clone());

        // Add to auto if it has addresses
        if !interface.addresses.is_empty() {
            config.auto_interfaces.push(interface.name.clone());
        }

        self.validate_configuration(&config)
    }
}

impl Default for IfUpDownValidator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pve_network_core::{AddressMethod, Interface, InterfaceType, IpAddress};
    use std::collections::HashMap;

    #[test]
    fn test_generate_interfaces_content() {
        let validator = IfUpDownValidator::new();
        let mut config = NetworkConfiguration::default();

        let interface = Interface {
            name: "eth0".to_string(),
            iface_type: InterfaceType::Physical,
            method: AddressMethod::Static,
            addresses: vec![IpAddress::new("192.168.1.10".parse().unwrap(), Some(24))],
            gateway: Some(IpAddress::new("192.168.1.1".parse().unwrap(), None)),
            mtu: Some(1500),
            options: HashMap::new(),
            enabled: true,
            comments: vec!["Test interface".to_string()],
        };

        config.interfaces.insert("eth0".to_string(), interface);
        config.auto_interfaces.push("eth0".to_string());
        config.ordering.push("eth0".to_string());

        let content = validator.generate_interfaces_content(&config).unwrap();

        assert!(content.contains("auto eth0"));
        assert!(content.contains("iface eth0 inet static"));
        assert!(content.contains("address 192.168.1.10/24"));
        assert!(content.contains("gateway 192.168.1.1"));
        assert!(content.contains("mtu 1500"));
        assert!(content.contains("# Test interface"));
    }

    #[test]
    fn test_bridge_config_generation() {
        let validator = IfUpDownValidator::new();

        let bridge = Interface {
            name: "br0".to_string(),
            iface_type: InterfaceType::Bridge {
                ports: vec!["eth0".to_string(), "eth1".to_string()],
                vlan_aware: true,
            },
            method: AddressMethod::Manual,
            addresses: vec![],
            gateway: None,
            mtu: None,
            options: HashMap::new(),
            enabled: true,
            comments: vec![],
        };

        let config = validator.generate_interface_config(&bridge).unwrap();

        assert!(config.contains("iface br0 inet manual"));
        assert!(config.contains("bridge-ports eth0 eth1"));
        assert!(config.contains("bridge-vlan-aware yes"));
        assert!(config.contains("bridge-stp off"));
    }

    #[test]
    fn test_bond_config_generation() {
        let validator = IfUpDownValidator::new();

        let bond = Interface {
            name: "bond0".to_string(),
            iface_type: InterfaceType::Bond {
                slaves: vec!["eth0".to_string(), "eth1".to_string()],
                mode: pve_network_core::BondMode::ActiveBackup,
                options: HashMap::from([("miimon".to_string(), "100".to_string())]),
            },
            method: AddressMethod::Manual,
            addresses: vec![],
            gateway: None,
            mtu: None,
            options: HashMap::new(),
            enabled: true,
            comments: vec![],
        };

        let config = validator.generate_interface_config(&bond).unwrap();

        assert!(config.contains("iface bond0 inet manual"));
        assert!(config.contains("bond-slaves eth0 eth1"));
        assert!(config.contains("bond-mode active-backup"));
        assert!(config.contains("bond-miimon 100"));
    }

    #[test]
    fn test_error_message_formatting() {
        let validator = IfUpDownValidator::new();

        let error = "ifupdown2: error: invalid configuration in /tmp/tmpfile123";
        let formatted = validator.format_error_message(error);

        assert!(formatted.contains("ERROR:"));
        assert!(!formatted.contains("ifupdown2:"));
    }
}
