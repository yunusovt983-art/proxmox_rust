//! Syntax validation for network configuration
//!
//! Validates network interface configuration syntax according to Debian interfaces(5) format

use regex::Regex;
use std::collections::HashSet;

use pve_network_core::error::{ConfigError, ValidationError};
use pve_network_core::{
    AddressMethod, Interface, InterfaceType, NetworkConfiguration, NetworkError,
};

/// Syntax validator for network configurations
pub struct SyntaxValidator {
    /// Valid interface name pattern
    interface_name_regex: Regex,
    /// Valid IP address pattern (IPv4 and IPv6)
    ip_address_regex: Regex,
    /// Valid MAC address pattern
    mac_address_regex: Regex,
    /// Valid VLAN tag range
    vlan_tag_range: std::ops::RangeInclusive<u16>,
    /// Valid MTU range
    mtu_range: std::ops::RangeInclusive<u16>,
}

impl SyntaxValidator {
    /// Create new syntax validator
    pub fn new() -> Self {
        Self {
            interface_name_regex: Regex::new(r"^[a-zA-Z][a-zA-Z0-9_.-]*$").unwrap(),
            ip_address_regex: Regex::new(
                r"^(?:(?:25[0-5]|2[0-4][0-9]|[01]?[0-9][0-9]?)\.){3}(?:25[0-5]|2[0-4][0-9]|[01]?[0-9][0-9]?)(?:/(?:[0-9]|[1-2][0-9]|3[0-2]))?$|^(?:[0-9a-fA-F]{1,4}:){7}[0-9a-fA-F]{1,4}(?:/(?:[0-9]|[1-9][0-9]|1[0-1][0-9]|12[0-8]))?$"
            ).unwrap(),
            mac_address_regex: Regex::new(r"^[0-9a-fA-F]{2}:[0-9a-fA-F]{2}:[0-9a-fA-F]{2}:[0-9a-fA-F]{2}:[0-9a-fA-F]{2}:[0-9a-fA-F]{2}$").unwrap(),
            vlan_tag_range: 1..=4094,
            mtu_range: 68..=65535,
        }
    }

    /// Validate complete network configuration syntax
    pub fn validate_configuration(
        &self,
        config: &NetworkConfiguration,
    ) -> Result<(), NetworkError> {
        let mut errors = Vec::new();
        let mut interface_names = HashSet::new();

        // Validate each interface
        for (name, interface) in &config.interfaces {
            // Check for duplicate interface names
            if !interface_names.insert(name.clone()) {
                errors.push(format!("Duplicate interface name: {}", name));
                continue;
            }

            // Validate individual interface
            if let Err(e) = self.validate_interface(interface) {
                errors.push(format!("Interface '{}': {}", name, e));
            }

            // Validate interface name matches key
            if interface.name != *name {
                errors.push(format!(
                    "Interface name mismatch: key '{}' vs name '{}'",
                    name, interface.name
                ));
            }
        }

        // Validate auto interfaces exist
        for auto_iface in &config.auto_interfaces {
            if !config.interfaces.contains_key(auto_iface) {
                errors.push(format!("Auto interface '{}' not defined", auto_iface));
            }
        }

        // Validate hotplug interfaces exist
        for hotplug_iface in &config.hotplug_interfaces {
            if !config.interfaces.contains_key(hotplug_iface) {
                errors.push(format!("Hotplug interface '{}' not defined", hotplug_iface));
            }
        }

        // Validate interface dependencies
        self.validate_interface_dependencies(config, &mut errors);

        if !errors.is_empty() {
            return Err(NetworkError::Validation(ValidationError::Schema {
                message: errors.join("; "),
            }));
        }

        Ok(())
    }

    /// Validate individual interface syntax
    pub fn validate_interface(&self, interface: &Interface) -> Result<(), NetworkError> {
        let mut errors = Vec::new();

        // Validate interface name
        if !self.interface_name_regex.is_match(&interface.name) {
            errors.push("Invalid interface name format".to_string());
        }

        // Validate interface name length
        if interface.name.len() > 15 {
            errors.push("Interface name too long (max 15 characters)".to_string());
        }

        // Validate addresses
        for address in &interface.addresses {
            if !self.ip_address_regex.is_match(&address.to_string()) {
                errors.push(format!("Invalid IP address format: {}", address));
            }
        }

        // Validate gateway
        if let Some(gateway) = &interface.gateway {
            if !self.ip_address_regex.is_match(&gateway.to_string()) {
                errors.push(format!("Invalid gateway address format: {}", gateway));
            }
        }

        // Validate MTU
        if let Some(mtu) = interface.mtu {
            if !self.mtu_range.contains(&mtu) {
                errors.push(format!(
                    "Invalid MTU value: {} (must be between {} and {})",
                    mtu,
                    self.mtu_range.start(),
                    self.mtu_range.end()
                ));
            }
        }

        // Validate interface type specific syntax
        self.validate_interface_type_syntax(&interface.iface_type, &mut errors);

        // Validate address method compatibility
        self.validate_address_method_compatibility(interface, &mut errors);

        // Validate interface options
        self.validate_interface_options(interface, &mut errors);

        if !errors.is_empty() {
            return Err(NetworkError::Configuration(ConfigError::InvalidValue {
                field: "interface".to_string(),
                value: errors.join("; "),
            }));
        }

        Ok(())
    }

    /// Validate interface type specific syntax
    fn validate_interface_type_syntax(&self, iface_type: &InterfaceType, errors: &mut Vec<String>) {
        match iface_type {
            InterfaceType::Bridge {
                ports,
                vlan_aware: _,
            } => {
                for port in ports {
                    if !self.interface_name_regex.is_match(port) {
                        errors.push(format!("Invalid bridge port name: {}", port));
                    }
                }
            }
            InterfaceType::Bond {
                slaves,
                mode: _,
                options: _,
            } => {
                if slaves.is_empty() {
                    errors.push("Bond interface must have at least one slave".to_string());
                }
                for slave in slaves {
                    if !self.interface_name_regex.is_match(slave) {
                        errors.push(format!("Invalid bond slave name: {}", slave));
                    }
                }
            }
            InterfaceType::Vlan { parent, tag } => {
                if !self.interface_name_regex.is_match(parent) {
                    errors.push(format!("Invalid VLAN parent interface name: {}", parent));
                }
                if !self.vlan_tag_range.contains(tag) {
                    errors.push(format!(
                        "Invalid VLAN tag: {} (must be between {} and {})",
                        tag,
                        self.vlan_tag_range.start(),
                        self.vlan_tag_range.end()
                    ));
                }
            }
            InterfaceType::Vxlan {
                id,
                local: _,
                remote: _,
                dstport,
            } => {
                if *id > 16777215 {
                    errors.push(format!("Invalid VXLAN ID: {} (must be <= 16777215)", id));
                }
                if let Some(port) = dstport {
                    if *port == 0 {
                        errors.push("Invalid VXLAN destination port: 0".to_string());
                    }
                }
            }
            InterfaceType::Physical | InterfaceType::Loopback => {
                // No specific validation needed for physical and loopback interfaces
            }
        }
    }

    /// Validate address method compatibility with interface configuration
    fn validate_address_method_compatibility(
        &self,
        interface: &Interface,
        errors: &mut Vec<String>,
    ) {
        match interface.method {
            AddressMethod::Static => {
                if interface.addresses.is_empty() {
                    errors.push("Static interface must have at least one address".to_string());
                }
            }
            AddressMethod::Dhcp => {
                if !interface.addresses.is_empty() {
                    errors.push("DHCP interface should not have static addresses".to_string());
                }
                if interface.gateway.is_some() {
                    errors.push("DHCP interface should not have static gateway".to_string());
                }
            }
            AddressMethod::Manual => {
                // Manual interfaces can have addresses or not
            }
            AddressMethod::None => {
                if !interface.addresses.is_empty() {
                    errors
                        .push("Interface with method 'none' should not have addresses".to_string());
                }
                if interface.gateway.is_some() {
                    errors.push("Interface with method 'none' should not have gateway".to_string());
                }
            }
        }
    }

    /// Validate interface options syntax
    fn validate_interface_options(&self, interface: &Interface, errors: &mut Vec<String>) {
        for (key, value) in &interface.options {
            // Validate option key format
            if !key
                .chars()
                .all(|c| c.is_alphanumeric() || c == '-' || c == '_')
            {
                errors.push(format!("Invalid option key format: {}", key));
            }

            // Validate specific known options
            match key.as_str() {
                "bridge-ports" => {
                    for port in value.split_whitespace() {
                        if port != "none" && !self.interface_name_regex.is_match(port) {
                            errors.push(format!("Invalid bridge port in options: {}", port));
                        }
                    }
                }
                "bond-slaves" => {
                    for slave in value.split_whitespace() {
                        if slave != "none" && !self.interface_name_regex.is_match(slave) {
                            errors.push(format!("Invalid bond slave in options: {}", slave));
                        }
                    }
                }
                "bond-mode" => {
                    let valid_modes = [
                        "balance-rr",
                        "active-backup",
                        "balance-xor",
                        "broadcast",
                        "802.3ad",
                        "balance-tlb",
                        "balance-alb",
                        "0",
                        "1",
                        "2",
                        "3",
                        "4",
                        "5",
                        "6",
                    ];
                    if !valid_modes.contains(&value.as_str()) {
                        errors.push(format!("Invalid bond mode: {}", value));
                    }
                }
                "bond-miimon" => {
                    if value.parse::<u32>().is_err() {
                        errors.push(format!("Invalid bond-miimon value: {}", value));
                    }
                }
                "vlan-raw-device" => {
                    if !self.interface_name_regex.is_match(value) {
                        errors.push(format!("Invalid VLAN raw device: {}", value));
                    }
                }
                "hwaddress" => {
                    if !self.mac_address_regex.is_match(value) {
                        errors.push(format!("Invalid MAC address: {}", value));
                    }
                }
                _ => {
                    // Unknown options are allowed but should be validated for basic syntax
                    if value.is_empty() {
                        errors.push(format!("Empty value for option: {}", key));
                    }
                }
            }
        }
    }

    /// Validate interface dependencies and references
    fn validate_interface_dependencies(
        &self,
        config: &NetworkConfiguration,
        errors: &mut Vec<String>,
    ) {
        for (name, interface) in &config.interfaces {
            match &interface.iface_type {
                InterfaceType::Bridge {
                    ports,
                    vlan_aware: _,
                } => {
                    for port in ports {
                        if !config.interfaces.contains_key(port) {
                            errors.push(format!(
                                "Bridge '{}' references undefined port: {}",
                                name, port
                            ));
                        }
                    }
                }
                InterfaceType::Bond {
                    slaves,
                    mode: _,
                    options: _,
                } => {
                    for slave in slaves {
                        if !config.interfaces.contains_key(slave) {
                            errors.push(format!(
                                "Bond '{}' references undefined slave: {}",
                                name, slave
                            ));
                        }
                    }
                }
                InterfaceType::Vlan { parent, tag: _ } => {
                    if !config.interfaces.contains_key(parent) {
                        errors.push(format!(
                            "VLAN '{}' references undefined parent: {}",
                            name, parent
                        ));
                    }
                }
                _ => {}
            }
        }

        // Check for circular dependencies
        self.validate_no_circular_dependencies(config, errors);
    }

    /// Validate that there are no circular dependencies between interfaces
    fn validate_no_circular_dependencies(
        &self,
        config: &NetworkConfiguration,
        errors: &mut Vec<String>,
    ) {
        for (name, _) in &config.interfaces {
            let mut visited = HashSet::new();
            let mut path = Vec::new();

            if self.has_circular_dependency(config, name, &mut visited, &mut path) {
                errors.push(format!(
                    "Circular dependency detected involving interface: {}",
                    name
                ));
            }
        }
    }

    /// Check if an interface has circular dependencies (DFS)
    fn has_circular_dependency(
        &self,
        config: &NetworkConfiguration,
        interface_name: &str,
        visited: &mut HashSet<String>,
        path: &mut Vec<String>,
    ) -> bool {
        if path.contains(&interface_name.to_string()) {
            return true; // Circular dependency found
        }

        if visited.contains(interface_name) {
            return false; // Already processed
        }

        visited.insert(interface_name.to_string());
        path.push(interface_name.to_string());

        if let Some(interface) = config.interfaces.get(interface_name) {
            let dependencies = self.get_interface_dependencies(interface);

            for dep in dependencies {
                if self.has_circular_dependency(config, &dep, visited, path) {
                    return true;
                }
            }
        }

        path.pop();
        false
    }

    /// Get list of interfaces that this interface depends on
    fn get_interface_dependencies(&self, interface: &Interface) -> Vec<String> {
        match &interface.iface_type {
            InterfaceType::Bridge {
                ports,
                vlan_aware: _,
            } => ports.clone(),
            InterfaceType::Bond {
                slaves,
                mode: _,
                options: _,
            } => slaves.clone(),
            InterfaceType::Vlan { parent, tag: _ } => vec![parent.clone()],
            _ => Vec::new(),
        }
    }

    /// Validate interface naming conventions
    pub fn validate_naming_conventions(
        &self,
        config: &NetworkConfiguration,
    ) -> Result<(), NetworkError> {
        let mut errors = Vec::new();

        for (name, interface) in &config.interfaces {
            // Check naming conventions based on interface type
            match &interface.iface_type {
                InterfaceType::Bridge { .. } => {
                    if !name.starts_with("br") && !name.starts_with("vmbr") {
                        errors.push(format!(
                            "Bridge interface '{}' should start with 'br' or 'vmbr'",
                            name
                        ));
                    }
                }
                InterfaceType::Bond { .. } => {
                    if !name.starts_with("bond") {
                        errors.push(format!(
                            "Bond interface '{}' should start with 'bond'",
                            name
                        ));
                    }
                }
                InterfaceType::Vlan { parent, tag } => {
                    let expected_name = format!("{}.{}", parent, tag);
                    if name != &expected_name {
                        errors.push(format!(
                            "VLAN interface '{}' should be named '{}'",
                            name, expected_name
                        ));
                    }
                }
                InterfaceType::Vxlan { .. } => {
                    if !name.starts_with("vxlan") {
                        errors.push(format!(
                            "VXLAN interface '{}' should start with 'vxlan'",
                            name
                        ));
                    }
                }
                _ => {} // No specific naming requirements for physical and loopback
            }
        }

        if !errors.is_empty() {
            return Err(NetworkError::Validation(ValidationError::Schema {
                message: errors.join("; "),
            }));
        }

        Ok(())
    }
}

impl Default for SyntaxValidator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pve_network_core::{BondMode, IpAddress};
    use std::collections::HashMap;

    #[test]
    fn test_syntax_validator_creation() {
        let validator = SyntaxValidator::new();
        assert!(validator.interface_name_regex.is_match("eth0"));
        assert!(!validator.interface_name_regex.is_match("0eth"));
    }

    #[test]
    fn test_valid_interface_name() {
        let validator = SyntaxValidator::new();
        assert!(validator.interface_name_regex.is_match("eth0"));
        assert!(validator.interface_name_regex.is_match("vmbr0"));
        assert!(validator.interface_name_regex.is_match("bond0"));
        assert!(validator.interface_name_regex.is_match("eth0.100"));
    }

    #[test]
    fn test_invalid_interface_name() {
        let validator = SyntaxValidator::new();
        assert!(!validator.interface_name_regex.is_match("0eth"));
        assert!(!validator.interface_name_regex.is_match("eth@0"));
        assert!(!validator.interface_name_regex.is_match(""));
    }

    #[test]
    fn test_valid_ip_addresses() {
        let validator = SyntaxValidator::new();
        assert!(validator.ip_address_regex.is_match("192.168.1.1"));
        assert!(validator.ip_address_regex.is_match("192.168.1.1/24"));
        assert!(validator.ip_address_regex.is_match("10.0.0.1/8"));
    }

    #[test]
    fn test_invalid_ip_addresses() {
        let validator = SyntaxValidator::new();
        assert!(!validator.ip_address_regex.is_match("256.1.1.1"));
        assert!(!validator.ip_address_regex.is_match("192.168.1"));
        assert!(!validator.ip_address_regex.is_match("192.168.1.1/33"));
    }

    #[test]
    fn test_valid_interface_validation() {
        let validator = SyntaxValidator::new();
        let interface = Interface {
            name: "eth0".to_string(),
            iface_type: InterfaceType::Physical,
            method: AddressMethod::Static,
            addresses: vec!["192.168.1.10/24".parse::<IpAddress>().unwrap()],
            gateway: Some("192.168.1.1".parse::<IpAddress>().unwrap()),
            mtu: Some(1500),
            options: HashMap::new(),
            enabled: true,
            comments: Vec::new(),
        };

        assert!(validator.validate_interface(&interface).is_ok());
    }

    #[test]
    fn test_invalid_mtu() {
        let validator = SyntaxValidator::new();
        let interface = Interface {
            name: "eth0".to_string(),
            iface_type: InterfaceType::Physical,
            method: AddressMethod::Static,
            addresses: vec!["192.168.1.10/24".parse::<IpAddress>().unwrap()],
            gateway: None,
            mtu: Some(67), // Invalid MTU - below minimum
            options: HashMap::new(),
            enabled: true,
            comments: Vec::new(),
        };

        assert!(validator.validate_interface(&interface).is_err());
    }

    #[test]
    fn test_dhcp_interface_validation() {
        let validator = SyntaxValidator::new();
        let interface = Interface {
            name: "eth0".to_string(),
            iface_type: InterfaceType::Physical,
            method: AddressMethod::Dhcp,
            addresses: Vec::new(), // DHCP should not have static addresses
            gateway: None,
            mtu: Some(1500),
            options: HashMap::new(),
            enabled: true,
            comments: Vec::new(),
        };

        assert!(validator.validate_interface(&interface).is_ok());
    }

    #[test]
    fn test_bridge_interface_validation() {
        let validator = SyntaxValidator::new();
        let interface = Interface {
            name: "vmbr0".to_string(),
            iface_type: InterfaceType::Bridge {
                ports: vec!["eth0".to_string()],
                vlan_aware: true,
            },
            method: AddressMethod::Static,
            addresses: vec!["192.168.1.1/24".parse::<IpAddress>().unwrap()],
            gateway: None,
            mtu: Some(1500),
            options: HashMap::new(),
            enabled: true,
            comments: Vec::new(),
        };

        assert!(validator.validate_interface(&interface).is_ok());
    }

    #[test]
    fn test_vlan_interface_validation() {
        let validator = SyntaxValidator::new();
        let interface = Interface {
            name: "eth0.100".to_string(),
            iface_type: InterfaceType::Vlan {
                parent: "eth0".to_string(),
                tag: 100,
            },
            method: AddressMethod::Static,
            addresses: vec!["192.168.100.1/24".parse::<IpAddress>().unwrap()],
            gateway: None,
            mtu: Some(1500),
            options: HashMap::new(),
            enabled: true,
            comments: Vec::new(),
        };

        assert!(validator.validate_interface(&interface).is_ok());
    }

    #[test]
    fn test_invalid_vlan_tag() {
        let validator = SyntaxValidator::new();
        let interface = Interface {
            name: "eth0.5000".to_string(),
            iface_type: InterfaceType::Vlan {
                parent: "eth0".to_string(),
                tag: 5000, // Invalid VLAN tag (> 4094)
            },
            method: AddressMethod::Static,
            addresses: vec!["192.168.100.1/24".parse::<IpAddress>().unwrap()],
            gateway: None,
            mtu: Some(1500),
            options: HashMap::new(),
            enabled: true,
            comments: Vec::new(),
        };

        assert!(validator.validate_interface(&interface).is_err());
    }
}
