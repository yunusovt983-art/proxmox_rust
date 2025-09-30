//! Semantic validation for network configurations

use pve_network_core::error::ValidationError;
use pve_network_core::{Interface, InterfaceType, IpAddress, NetworkConfiguration, NetworkError};
use std::collections::{HashMap, HashSet};

/// Semantic validator for network configurations
pub struct SemanticValidator {}

impl SemanticValidator {
    /// Create new semantic validator
    pub fn new() -> Self {
        Self {}
    }

    /// Validate network configuration semantics
    pub fn validate_configuration(
        &self,
        config: &NetworkConfiguration,
    ) -> Result<(), NetworkError> {
        let mut errors = Vec::new();

        // Check for IP conflicts
        if let Err(e) = self.validate_ip_conflicts(config) {
            errors.push(format!("IP conflicts: {}", e));
        }

        // Check for interface dependencies
        if let Err(e) = self.validate_interface_dependencies(config) {
            errors.push(format!("Interface dependencies: {}", e));
        }

        // Check for duplicate interface usage
        if let Err(e) = self.validate_interface_usage(config) {
            errors.push(format!("Interface usage: {}", e));
        }

        // Check for circular dependencies
        if let Err(e) = self.validate_circular_dependencies(config) {
            errors.push(format!("Circular dependencies: {}", e));
        }

        // Check for bridge/bond configuration conflicts
        if let Err(e) = self.validate_bridge_bond_conflicts(config) {
            errors.push(format!("Bridge/Bond conflicts: {}", e));
        }

        if !errors.is_empty() {
            return Err(NetworkError::Validation(ValidationError::NetworkConflict {
                message: errors.join("; "),
            }));
        }

        Ok(())
    }

    /// Validate IP address conflicts
    fn validate_ip_conflicts(&self, config: &NetworkConfiguration) -> Result<(), NetworkError> {
        let mut used_networks: Vec<(String, IpAddress)> = Vec::new();

        for (name, interface) in &config.interfaces {
            for addr in &interface.addresses {
                // Check for exact IP duplicates
                for (existing_iface, existing_addr) in &used_networks {
                    if addr.addr == existing_addr.addr {
                        return Err(NetworkError::Validation(ValidationError::NetworkConflict {
                            message: format!(
                                "Duplicate IP address {} on interfaces '{}' and '{}'",
                                addr.addr, existing_iface, name
                            ),
                        }));
                    }

                    // Check for network overlaps
                    if addr.same_network(existing_addr) {
                        return Err(NetworkError::Validation(ValidationError::NetworkConflict {
                            message: format!(
                                "Overlapping networks: {} on '{}' conflicts with {} on '{}'",
                                addr, name, existing_addr, existing_iface
                            ),
                        }));
                    }
                }
                used_networks.push((name.clone(), addr.clone()));
            }
        }

        Ok(())
    }

    /// Validate interface dependencies exist
    fn validate_interface_dependencies(
        &self,
        config: &NetworkConfiguration,
    ) -> Result<(), NetworkError> {
        for (name, interface) in &config.interfaces {
            match &interface.iface_type {
                InterfaceType::Bridge { ports, .. } => {
                    for port in ports {
                        if !config.interfaces.contains_key(port) {
                            return Err(NetworkError::Validation(ValidationError::Interface {
                                name: name.clone(),
                                reason: format!("Bridge port '{}' not defined", port),
                            }));
                        }
                    }
                }
                InterfaceType::Bond { slaves, .. } => {
                    for slave in slaves {
                        if !config.interfaces.contains_key(slave) {
                            return Err(NetworkError::Validation(ValidationError::Interface {
                                name: name.clone(),
                                reason: format!("Bond slave '{}' not defined", slave),
                            }));
                        }
                    }
                }
                InterfaceType::Vlan { parent, .. } => {
                    if !config.interfaces.contains_key(parent) {
                        return Err(NetworkError::Validation(ValidationError::Interface {
                            name: name.clone(),
                            reason: format!("VLAN parent '{}' not defined", parent),
                        }));
                    }
                }
                _ => {}
            }
        }
        Ok(())
    }

    /// Validate interface usage (no interface used in multiple places)
    fn validate_interface_usage(&self, config: &NetworkConfiguration) -> Result<(), NetworkError> {
        let mut used_interfaces: HashMap<String, Vec<String>> = HashMap::new();

        for (name, interface) in &config.interfaces {
            match &interface.iface_type {
                InterfaceType::Bridge { ports, .. } => {
                    for port in ports {
                        used_interfaces
                            .entry(port.clone())
                            .or_default()
                            .push(name.clone());
                    }
                }
                InterfaceType::Bond { slaves, .. } => {
                    for slave in slaves {
                        used_interfaces
                            .entry(slave.clone())
                            .or_default()
                            .push(name.clone());
                    }
                }
                _ => {}
            }
        }

        // Check for interfaces used in multiple places
        for (iface, users) in used_interfaces {
            if users.len() > 1 {
                return Err(NetworkError::Validation(ValidationError::NetworkConflict {
                    message: format!(
                        "Interface '{}' used by multiple interfaces: {}",
                        iface,
                        users.join(", ")
                    ),
                }));
            }
        }

        Ok(())
    }

    /// Validate circular dependencies
    fn validate_circular_dependencies(
        &self,
        config: &NetworkConfiguration,
    ) -> Result<(), NetworkError> {
        let mut visited = HashSet::new();
        let mut rec_stack = HashSet::new();

        for interface_name in config.interfaces.keys() {
            if !visited.contains(interface_name) {
                if self.has_circular_dependency(
                    config,
                    interface_name,
                    &mut visited,
                    &mut rec_stack,
                )? {
                    return Err(NetworkError::Validation(ValidationError::NetworkConflict {
                        message: format!(
                            "Circular dependency detected involving interface '{}'",
                            interface_name
                        ),
                    }));
                }
            }
        }

        Ok(())
    }

    /// Check for circular dependency using DFS
    fn has_circular_dependency(
        &self,
        config: &NetworkConfiguration,
        interface_name: &str,
        visited: &mut HashSet<String>,
        rec_stack: &mut HashSet<String>,
    ) -> Result<bool, NetworkError> {
        visited.insert(interface_name.to_string());
        rec_stack.insert(interface_name.to_string());

        if let Some(interface) = config.interfaces.get(interface_name) {
            let dependencies = self.get_interface_dependencies(interface);

            for dep in dependencies {
                if !visited.contains(&dep) {
                    if self.has_circular_dependency(config, &dep, visited, rec_stack)? {
                        return Ok(true);
                    }
                } else if rec_stack.contains(&dep) {
                    return Ok(true);
                }
            }
        }

        rec_stack.remove(interface_name);
        Ok(false)
    }

    /// Get interface dependencies
    fn get_interface_dependencies(&self, interface: &Interface) -> Vec<String> {
        match &interface.iface_type {
            InterfaceType::Bridge { ports, .. } => ports.clone(),
            InterfaceType::Bond { slaves, .. } => slaves.clone(),
            InterfaceType::Vlan { parent, .. } => vec![parent.clone()],
            _ => vec![],
        }
    }

    /// Validate bridge and bond configuration conflicts
    fn validate_bridge_bond_conflicts(
        &self,
        config: &NetworkConfiguration,
    ) -> Result<(), NetworkError> {
        for (name, interface) in &config.interfaces {
            match &interface.iface_type {
                InterfaceType::Bridge { ports, .. } => {
                    // Check if bridge ports are physical interfaces or other valid types
                    for port in ports {
                        if let Some(port_interface) = config.interfaces.get(port) {
                            match &port_interface.iface_type {
                                InterfaceType::Bridge { .. } => {
                                    return Err(NetworkError::Validation(
                                        ValidationError::Interface {
                                            name: name.clone(),
                                            reason: format!(
                                                "Cannot add bridge '{}' as port to bridge",
                                                port
                                            ),
                                        },
                                    ));
                                }
                                InterfaceType::Loopback => {
                                    return Err(NetworkError::Validation(
                                        ValidationError::Interface {
                                            name: name.clone(),
                                            reason: format!(
                                                "Cannot add loopback interface '{}' as bridge port",
                                                port
                                            ),
                                        },
                                    ));
                                }
                                _ => {} // Other types are generally OK
                            }
                        }
                    }
                }
                InterfaceType::Bond { slaves, .. } => {
                    // Check if bond slaves are physical interfaces
                    for slave in slaves {
                        if let Some(slave_interface) = config.interfaces.get(slave) {
                            match &slave_interface.iface_type {
                                InterfaceType::Physical => {} // OK
                                _ => {
                                    return Err(NetworkError::Validation(
                                        ValidationError::Interface {
                                            name: name.clone(),
                                            reason: format!(
                                                "Bond slave '{}' must be a physical interface",
                                                slave
                                            ),
                                        },
                                    ));
                                }
                            }
                        }
                    }
                }
                _ => {}
            }
        }
        Ok(())
    }
}

impl Default for SemanticValidator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pve_network_core::AddressMethod;
    use std::collections::HashMap;
    use std::net::IpAddr;

    #[test]
    fn test_ip_conflict_detection() {
        let validator = SemanticValidator::new();
        let mut config = NetworkConfiguration::default();

        // Add two interfaces with same IP
        let mut iface1 = Interface {
            name: "eth0".to_string(),
            iface_type: InterfaceType::Physical,
            method: AddressMethod::Static,
            addresses: vec![IpAddress::new("192.168.1.10".parse().unwrap(), Some(24))],
            gateway: None,
            mtu: None,
            options: HashMap::new(),
            enabled: true,
            comments: vec![],
        };

        let mut iface2 = Interface {
            name: "eth1".to_string(),
            iface_type: InterfaceType::Physical,
            method: AddressMethod::Static,
            addresses: vec![IpAddress::new("192.168.1.10".parse().unwrap(), Some(24))],
            gateway: None,
            mtu: None,
            options: HashMap::new(),
            enabled: true,
            comments: vec![],
        };

        config.interfaces.insert("eth0".to_string(), iface1);
        config.interfaces.insert("eth1".to_string(), iface2);

        assert!(validator.validate_ip_conflicts(&config).is_err());
    }

    #[test]
    fn test_interface_dependency_validation() {
        let validator = SemanticValidator::new();
        let mut config = NetworkConfiguration::default();

        // Add bridge with non-existent port
        let bridge = Interface {
            name: "br0".to_string(),
            iface_type: InterfaceType::Bridge {
                ports: vec!["nonexistent".to_string()],
                vlan_aware: false,
            },
            method: AddressMethod::Manual,
            addresses: vec![],
            gateway: None,
            mtu: None,
            options: HashMap::new(),
            enabled: true,
            comments: vec![],
        };

        config.interfaces.insert("br0".to_string(), bridge);

        assert!(validator.validate_interface_dependencies(&config).is_err());
    }
}
