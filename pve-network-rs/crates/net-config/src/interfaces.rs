//! /etc/network/interfaces parser

use std::collections::HashMap;

use indexmap::IndexMap;
use regex::Regex;

use pve_network_core::error::ConfigError;
use pve_network_core::{
    AddressMethod, BondMode, Interface, InterfaceType, IpAddress, NetworkConfiguration,
    NetworkError, Result,
};

/// Parser for /etc/network/interfaces
pub struct InterfacesParser {
    preserve_comments: bool,
    preserve_order: bool,
}

/// Represents a line in the interfaces file
#[derive(Debug, Clone)]
enum InterfaceLine {
    Comment(String),
    Auto(Vec<String>),
    Hotplug(Vec<String>),
    Iface {
        name: String,
        family: String,
        method: String,
    },
    Option {
        key: String,
        value: String,
    },
    Empty,
}

/// Parser state for tracking current interface
#[derive(Debug)]
struct ParseState {
    current_interface: Option<String>,
    interfaces: IndexMap<String, Interface>,
    auto_interfaces: Vec<String>,
    hotplug_interfaces: Vec<String>,
    comments: HashMap<String, String>,
    ordering: Vec<String>,
    line_number: usize,
}

impl InterfacesParser {
    /// Create new parser
    pub fn new() -> Self {
        Self {
            preserve_comments: true,
            preserve_order: true,
        }
    }

    /// Create parser with specific options
    pub fn with_options(preserve_comments: bool, preserve_order: bool) -> Self {
        Self {
            preserve_comments,
            preserve_order,
        }
    }

    /// Parse interfaces file content
    pub fn parse(&self, content: &str) -> Result<NetworkConfiguration> {
        let mut state = ParseState {
            current_interface: None,
            interfaces: IndexMap::new(),
            auto_interfaces: Vec::new(),
            hotplug_interfaces: Vec::new(),
            comments: HashMap::new(),
            ordering: Vec::new(),
            line_number: 0,
        };

        let mut current_comments = Vec::new();

        for line in content.lines() {
            state.line_number += 1;
            let parsed_line = self.parse_line(line, state.line_number)?;

            match parsed_line {
                InterfaceLine::Comment(comment) => {
                    if self.preserve_comments {
                        current_comments.push(comment);
                    }
                }
                InterfaceLine::Auto(interfaces) => {
                    state.auto_interfaces.extend(interfaces);
                }
                InterfaceLine::Hotplug(interfaces) => {
                    state.hotplug_interfaces.extend(interfaces);
                }
                InterfaceLine::Iface {
                    name,
                    family,
                    method,
                } => {
                    // Start new interface
                    let mut interface =
                        self.create_interface(&name, &family, &method, state.line_number)?;

                    // Assign accumulated comments to this interface
                    if !current_comments.is_empty() && self.preserve_comments {
                        interface.comments = current_comments.clone();
                        current_comments.clear();
                    }

                    if self.preserve_order && !state.ordering.contains(&name) {
                        state.ordering.push(name.clone());
                    }

                    state.interfaces.insert(name.clone(), interface);
                    state.current_interface = Some(name);
                }
                InterfaceLine::Option { key, value } => {
                    if let Some(current_name) = &state.current_interface {
                        if let Some(interface) = state.interfaces.get_mut(current_name) {
                            self.apply_option(interface, &key, &value, state.line_number)?;
                        }
                    } else {
                        return Err(NetworkError::Configuration(ConfigError::Parse {
                            line: state.line_number,
                            message: format!("Option '{}' found outside interface definition", key),
                        }));
                    }
                }
                InterfaceLine::Empty => {
                    // Empty lines reset comment accumulation
                    if !current_comments.is_empty() && self.preserve_comments {
                        if let Some(current_name) = &state.current_interface {
                            if let Some(interface) = state.interfaces.get_mut(current_name) {
                                interface.comments.extend(current_comments.clone());
                            }
                        }
                        current_comments.clear();
                    }
                }
            }
        }

        // Handle final comments
        if let Some(current_name) = &state.current_interface {
            if let Some(interface) = state.interfaces.get_mut(current_name) {
                if !current_comments.is_empty() && self.preserve_comments {
                    interface.comments.extend(current_comments);
                }
            }
        }

        Ok(NetworkConfiguration {
            interfaces: state.interfaces.into_iter().collect(),
            auto_interfaces: state.auto_interfaces,
            hotplug_interfaces: state.hotplug_interfaces,
            comments: state.comments,
            ordering: state.ordering,
        })
    }

    /// Generate interfaces file content
    pub fn generate(&self, config: &NetworkConfiguration) -> Result<String> {
        let mut output = String::new();

        // Generate auto interfaces
        if !config.auto_interfaces.is_empty() {
            output.push_str(&format!("auto {}\n", config.auto_interfaces.join(" ")));
        }

        // Generate hotplug interfaces
        if !config.hotplug_interfaces.is_empty() {
            output.push_str(&format!(
                "allow-hotplug {}\n",
                config.hotplug_interfaces.join(" ")
            ));
        }

        if !config.auto_interfaces.is_empty() || !config.hotplug_interfaces.is_empty() {
            output.push('\n');
        }

        // Generate interfaces in preserved order or alphabetical order
        let interface_order = if self.preserve_order && !config.ordering.is_empty() {
            config.ordering.clone()
        } else {
            let mut names: Vec<_> = config.interfaces.keys().cloned().collect();
            names.sort();
            names
        };

        for (i, name) in interface_order.iter().enumerate() {
            if let Some(interface) = config.interfaces.get(name) {
                if i > 0 {
                    output.push('\n');
                }

                // Add comments before interface
                if self.preserve_comments {
                    for comment in &interface.comments {
                        output.push_str(&format!("# {}\n", comment));
                    }
                }

                self.generate_interface(interface, &mut output)?;
            }
        }

        Ok(output)
    }

    /// Parse a single line
    fn parse_line(&self, line: &str, line_number: usize) -> Result<InterfaceLine> {
        // Empty line
        if line.trim().is_empty() {
            return Ok(InterfaceLine::Empty);
        }

        // Option line (indented) - check before trimming
        if line.starts_with(' ') || line.starts_with('\t') {
            let trimmed = line.trim();
            // Split on first whitespace to handle values with spaces
            let mut parts = trimmed.splitn(2, char::is_whitespace);
            let key = parts.next().unwrap_or("").to_string();
            let value = parts.next().unwrap_or("").trim().to_string();

            return Ok(InterfaceLine::Option { key, value });
        }

        let line = line.trim();

        // Comment
        if line.starts_with('#') {
            let comment = line.strip_prefix('#').unwrap_or("").trim().to_string();
            return Ok(InterfaceLine::Comment(comment));
        }

        // Auto interfaces
        if line.starts_with("auto ") {
            let interfaces: Vec<String> = line[5..]
                .split_whitespace()
                .map(|s| s.to_string())
                .collect();
            return Ok(InterfaceLine::Auto(interfaces));
        }

        // Hotplug interfaces
        if line.starts_with("allow-hotplug ") {
            let interfaces: Vec<String> = line[14..]
                .split_whitespace()
                .map(|s| s.to_string())
                .collect();
            return Ok(InterfaceLine::Hotplug(interfaces));
        }

        // Interface definition
        if line.starts_with("iface ") {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() < 4 {
                return Err(NetworkError::Configuration(ConfigError::Parse {
                    line: line_number,
                    message: "Invalid iface line format".to_string(),
                }));
            }

            return Ok(InterfaceLine::Iface {
                name: parts[1].to_string(),
                family: parts[2].to_string(),
                method: parts[3].to_string(),
            });
        }

        // Unknown line format
        Err(NetworkError::Configuration(ConfigError::Parse {
            line: line_number,
            message: format!("Unknown line format: {}", line),
        }))
    }

    /// Create interface from parsed data
    fn create_interface(
        &self,
        name: &str,
        _family: &str,
        method: &str,
        line_number: usize,
    ) -> Result<Interface> {
        // Validate interface name
        let name_regex = Regex::new(r"^[a-zA-Z][a-zA-Z0-9_.-]*$").unwrap();
        if !name_regex.is_match(name) {
            return Err(NetworkError::Configuration(
                ConfigError::InvalidInterfaceName {
                    name: name.to_string(),
                },
            ));
        }

        // Parse address method
        let address_method = match method {
            "static" => AddressMethod::Static,
            "dhcp" => AddressMethod::Dhcp,
            "manual" => AddressMethod::Manual,
            "none" => AddressMethod::None,
            "loopback" => AddressMethod::None, // loopback is treated as none
            _ => {
                return Err(NetworkError::Configuration(ConfigError::Parse {
                    line: line_number,
                    message: format!("Unknown address method: {}", method),
                }));
            }
        };

        // Determine interface type from name
        let interface_type = self.determine_interface_type(name);

        Ok(Interface {
            name: name.to_string(),
            iface_type: interface_type,
            method: address_method,
            addresses: Vec::new(),
            gateway: None,
            mtu: None,
            options: HashMap::new(),
            enabled: true,
            comments: Vec::new(),
        })
    }

    /// Determine interface type from name
    fn determine_interface_type(&self, name: &str) -> InterfaceType {
        if name == "lo" {
            return InterfaceType::Loopback;
        }

        // VLAN interface (e.g., eth0.100, vmbr0.200)
        if let Some((parent, tag_str)) = name.rsplit_once('.') {
            if let Ok(tag) = tag_str.parse::<u16>() {
                return InterfaceType::Vlan {
                    parent: parent.to_string(),
                    tag,
                };
            }
        }

        // Bridge interface (starts with vmbr, br-, or contains "bridge")
        if name.starts_with("vmbr") || name.starts_with("br-") || name.contains("bridge") {
            return InterfaceType::Bridge {
                ports: Vec::new(),
                vlan_aware: false,
            };
        }

        // Bond interface (starts with bond)
        if name.starts_with("bond") {
            return InterfaceType::Bond {
                slaves: Vec::new(),
                mode: BondMode::ActiveBackup,
                options: HashMap::new(),
            };
        }

        // VXLAN interface (starts with vxlan)
        if name.starts_with("vxlan") {
            return InterfaceType::Vxlan {
                id: 0,
                local: "127.0.0.1".parse().unwrap(),
                remote: None,
                dstport: None,
            };
        }

        // Default to physical interface
        InterfaceType::Physical
    }

    /// Apply option to interface
    fn apply_option(
        &self,
        interface: &mut Interface,
        key: &str,
        value: &str,
        line_number: usize,
    ) -> Result<()> {
        match key {
            "address" => {
                let addr = value.parse::<IpAddress>().map_err(|_| {
                    NetworkError::Configuration(ConfigError::Parse {
                        line: line_number,
                        message: format!("Invalid IP address: {}", value),
                    })
                })?;
                interface.addresses.push(addr);
            }
            "netmask" => {
                // Convert netmask to CIDR if we have an address without prefix
                if let Some(last_addr) = interface.addresses.last_mut() {
                    if last_addr.prefix_len.is_none() {
                        let prefix_len = self.netmask_to_cidr(value)?;
                        last_addr.prefix_len = Some(prefix_len);
                    }
                }
            }
            "gateway" => {
                let gateway = value.parse::<IpAddress>().map_err(|_| {
                    NetworkError::Configuration(ConfigError::Parse {
                        line: line_number,
                        message: format!("Invalid gateway address: {}", value),
                    })
                })?;
                interface.gateway = Some(gateway);
            }
            "mtu" => {
                let mtu = value.parse::<u16>().map_err(|_| {
                    NetworkError::Configuration(ConfigError::Parse {
                        line: line_number,
                        message: format!("Invalid MTU value: {}", value),
                    })
                })?;
                interface.mtu = Some(mtu);
            }
            "bridge_ports" | "bridge-ports" => {
                if let InterfaceType::Bridge { ports, .. } = &mut interface.iface_type {
                    if value == "none" {
                        ports.clear();
                    } else {
                        ports.extend(value.split_whitespace().map(|s| s.to_string()));
                    }
                }
            }
            "bridge_vlan_aware" | "bridge-vlan-aware" => {
                if let InterfaceType::Bridge { vlan_aware, .. } = &mut interface.iface_type {
                    *vlan_aware = value == "yes" || value == "1" || value == "true";
                }
            }
            "bond_slaves" | "bond-slaves" => {
                if let InterfaceType::Bond { slaves, .. } = &mut interface.iface_type {
                    if value == "none" {
                        slaves.clear();
                    } else {
                        slaves.extend(value.split_whitespace().map(|s| s.to_string()));
                    }
                }
            }
            "bond_mode" | "bond-mode" => {
                if let InterfaceType::Bond { mode, .. } = &mut interface.iface_type {
                    *mode = value.parse::<BondMode>().map_err(|_| {
                        NetworkError::Configuration(ConfigError::Parse {
                            line: line_number,
                            message: format!("Invalid bond mode: {}", value),
                        })
                    })?;
                }
            }
            "vxlan_id" | "vxlan-id" => {
                if let InterfaceType::Vxlan { id, .. } = &mut interface.iface_type {
                    *id = value.parse::<u32>().map_err(|_| {
                        NetworkError::Configuration(ConfigError::Parse {
                            line: line_number,
                            message: format!("Invalid VXLAN ID: {}", value),
                        })
                    })?;
                }
            }
            "vxlan_local" | "vxlan-local" => {
                if let InterfaceType::Vxlan { local, .. } = &mut interface.iface_type {
                    *local = value.parse::<IpAddress>().map_err(|_| {
                        NetworkError::Configuration(ConfigError::Parse {
                            line: line_number,
                            message: format!("Invalid VXLAN local address: {}", value),
                        })
                    })?;
                }
            }
            "vxlan_remote" | "vxlan-remote" => {
                if let InterfaceType::Vxlan { remote, .. } = &mut interface.iface_type {
                    let addr = value.parse::<IpAddress>().map_err(|_| {
                        NetworkError::Configuration(ConfigError::Parse {
                            line: line_number,
                            message: format!("Invalid VXLAN remote address: {}", value),
                        })
                    })?;
                    *remote = Some(addr);
                }
            }
            "vxlan_dstport" | "vxlan-dstport" => {
                if let InterfaceType::Vxlan { dstport, .. } = &mut interface.iface_type {
                    let port = value.parse::<u16>().map_err(|_| {
                        NetworkError::Configuration(ConfigError::Parse {
                            line: line_number,
                            message: format!("Invalid VXLAN destination port: {}", value),
                        })
                    })?;
                    *dstport = Some(port);
                }
            }
            _ => {
                // Store unknown options for later use
                interface.options.insert(key.to_string(), value.to_string());
            }
        }

        Ok(())
    }

    /// Convert netmask to CIDR prefix length
    fn netmask_to_cidr(&self, netmask: &str) -> Result<u8> {
        match netmask {
            "255.255.255.255" => Ok(32),
            "255.255.255.254" => Ok(31),
            "255.255.255.252" => Ok(30),
            "255.255.255.248" => Ok(29),
            "255.255.255.240" => Ok(28),
            "255.255.255.224" => Ok(27),
            "255.255.255.192" => Ok(26),
            "255.255.255.128" => Ok(25),
            "255.255.255.0" => Ok(24),
            "255.255.254.0" => Ok(23),
            "255.255.252.0" => Ok(22),
            "255.255.248.0" => Ok(21),
            "255.255.240.0" => Ok(20),
            "255.255.224.0" => Ok(19),
            "255.255.192.0" => Ok(18),
            "255.255.128.0" => Ok(17),
            "255.255.0.0" => Ok(16),
            "255.254.0.0" => Ok(15),
            "255.252.0.0" => Ok(14),
            "255.248.0.0" => Ok(13),
            "255.240.0.0" => Ok(12),
            "255.224.0.0" => Ok(11),
            "255.192.0.0" => Ok(10),
            "255.128.0.0" => Ok(9),
            "255.0.0.0" => Ok(8),
            "254.0.0.0" => Ok(7),
            "252.0.0.0" => Ok(6),
            "248.0.0.0" => Ok(5),
            "240.0.0.0" => Ok(4),
            "224.0.0.0" => Ok(3),
            "192.0.0.0" => Ok(2),
            "128.0.0.0" => Ok(1),
            "0.0.0.0" => Ok(0),
            _ => Err(NetworkError::Configuration(ConfigError::InvalidValue {
                field: "netmask".to_string(),
                value: netmask.to_string(),
            })),
        }
    }

    /// Generate interface configuration
    fn generate_interface(&self, interface: &Interface, output: &mut String) -> Result<()> {
        // Determine address family
        let family = if interface.addresses.iter().any(|addr| addr.addr.is_ipv6()) {
            "inet6"
        } else {
            "inet"
        };

        // Generate iface line
        let method = match interface.method {
            AddressMethod::Static => "static",
            AddressMethod::Dhcp => "dhcp",
            AddressMethod::Manual => "manual",
            AddressMethod::None => "none",
        };

        output.push_str(&format!("iface {} {} {}\n", interface.name, family, method));

        // Generate addresses
        for addr in &interface.addresses {
            output.push_str(&format!("    address {}\n", addr));
        }

        // Generate gateway
        if let Some(gateway) = &interface.gateway {
            output.push_str(&format!("    gateway {}\n", gateway.addr));
        }

        // Generate MTU
        if let Some(mtu) = interface.mtu {
            output.push_str(&format!("    mtu {}\n", mtu));
        }

        // Generate interface-specific options
        match &interface.iface_type {
            InterfaceType::Bridge { ports, vlan_aware } => {
                if ports.is_empty() {
                    output.push_str("    bridge-ports none\n");
                } else {
                    output.push_str(&format!("    bridge-ports {}\n", ports.join(" ")));
                }
                if *vlan_aware {
                    output.push_str("    bridge-vlan-aware yes\n");
                }
            }
            InterfaceType::Bond {
                slaves,
                mode,
                options,
            } => {
                if slaves.is_empty() {
                    output.push_str("    bond-slaves none\n");
                } else {
                    output.push_str(&format!("    bond-slaves {}\n", slaves.join(" ")));
                }

                let mode_str = match mode {
                    BondMode::RoundRobin => "balance-rr",
                    BondMode::ActiveBackup => "active-backup",
                    BondMode::Xor => "balance-xor",
                    BondMode::Broadcast => "broadcast",
                    BondMode::Ieee8023ad => "802.3ad",
                    BondMode::BalanceTlb => "balance-tlb",
                    BondMode::BalanceAlb => "balance-alb",
                };
                output.push_str(&format!("    bond-mode {}\n", mode_str));

                for (key, value) in options {
                    output.push_str(&format!("    {} {}\n", key, value));
                }
            }
            InterfaceType::Vxlan {
                id,
                local,
                remote,
                dstport,
            } => {
                output.push_str(&format!("    vxlan-id {}\n", id));
                output.push_str(&format!("    vxlan-local {}\n", local.addr));
                if let Some(remote_addr) = remote {
                    output.push_str(&format!("    vxlan-remote {}\n", remote_addr.addr));
                }
                if let Some(port) = dstport {
                    output.push_str(&format!("    vxlan-dstport {}\n", port));
                }
            }
            _ => {}
        }

        // Generate additional options
        for (key, value) in &interface.options {
            output.push_str(&format!("    {} {}\n", key, value));
        }

        Ok(())
    }
}

impl Default for InterfacesParser {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

    #[test]
    fn test_parse_simple_loopback() {
        let content = r#"
auto lo
iface lo inet loopback
"#;
        let parser = InterfacesParser::new();
        let config = parser.parse(content).unwrap();

        assert_eq!(config.interfaces.len(), 1);
        assert!(config.interfaces.contains_key("lo"));
        assert_eq!(config.auto_interfaces, vec!["lo"]);

        let lo_interface = &config.interfaces["lo"];
        assert_eq!(lo_interface.name, "lo");
        assert_eq!(lo_interface.iface_type, InterfaceType::Loopback);
        assert_eq!(lo_interface.method, AddressMethod::None);
    }

    #[test]
    fn test_parse_static_interface() {
        let content = r#"
auto eth0
iface eth0 inet static
    address 192.168.1.10/24
    gateway 192.168.1.1
    mtu 1500
"#;
        let parser = InterfacesParser::new();
        let config = parser.parse(content).unwrap();

        assert_eq!(config.interfaces.len(), 1);
        let eth0 = &config.interfaces["eth0"];

        assert_eq!(eth0.name, "eth0");
        assert_eq!(eth0.iface_type, InterfaceType::Physical);
        assert_eq!(eth0.method, AddressMethod::Static);
        assert_eq!(eth0.addresses.len(), 1);
        assert_eq!(
            eth0.addresses[0].addr,
            IpAddr::V4(Ipv4Addr::new(192, 168, 1, 10))
        );
        assert_eq!(eth0.addresses[0].prefix_len, Some(24));
        assert_eq!(
            eth0.gateway.as_ref().unwrap().addr,
            IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1))
        );
        assert_eq!(eth0.mtu, Some(1500));
    }

    #[test]
    fn test_parse_static_interface_with_netmask() {
        let content = r#"
iface eth0 inet static
    address 192.168.1.10
    netmask 255.255.255.0
    gateway 192.168.1.1
"#;
        let parser = InterfacesParser::new();
        let config = parser.parse(content).unwrap();

        let eth0 = &config.interfaces["eth0"];
        assert_eq!(
            eth0.addresses[0].addr,
            IpAddr::V4(Ipv4Addr::new(192, 168, 1, 10))
        );
        assert_eq!(eth0.addresses[0].prefix_len, Some(24));
    }

    #[test]
    fn test_parse_dhcp_interface() {
        let content = r#"
auto eth0
iface eth0 inet dhcp
"#;
        let parser = InterfacesParser::new();
        let config = parser.parse(content).unwrap();

        let eth0 = &config.interfaces["eth0"];
        assert_eq!(eth0.method, AddressMethod::Dhcp);
    }

    #[test]
    fn test_parse_bridge_interface() {
        let content = r#"
auto vmbr0
iface vmbr0 inet static
    address 192.168.1.1/24
    bridge-ports eth0 eth1
    bridge-vlan-aware yes
"#;
        let parser = InterfacesParser::new();
        let config = parser.parse(content).unwrap();

        let vmbr0 = &config.interfaces["vmbr0"];
        assert_eq!(vmbr0.name, "vmbr0");

        if let InterfaceType::Bridge { ports, vlan_aware } = &vmbr0.iface_type {
            assert_eq!(ports, &vec!["eth0".to_string(), "eth1".to_string()]);
            assert_eq!(*vlan_aware, true);
        } else {
            panic!("Expected Bridge interface type");
        }
    }

    #[test]
    fn test_parse_bridge_with_no_ports() {
        let content = r#"
iface vmbr0 inet manual
    bridge-ports none
"#;
        let parser = InterfacesParser::new();
        let config = parser.parse(content).unwrap();

        let vmbr0 = &config.interfaces["vmbr0"];
        if let InterfaceType::Bridge { ports, .. } = &vmbr0.iface_type {
            assert!(ports.is_empty());
        } else {
            panic!("Expected Bridge interface type");
        }
    }

    #[test]
    fn test_parse_bond_interface() {
        let content = r#"
auto bond0
iface bond0 inet static
    address 192.168.1.10/24
    bond-slaves eth0 eth1
    bond-mode active-backup
"#;
        let parser = InterfacesParser::new();
        let config = parser.parse(content).unwrap();

        let bond0 = &config.interfaces["bond0"];
        assert_eq!(bond0.name, "bond0");

        if let InterfaceType::Bond { slaves, mode, .. } = &bond0.iface_type {
            assert_eq!(slaves, &vec!["eth0".to_string(), "eth1".to_string()]);
            assert_eq!(*mode, BondMode::ActiveBackup);
        } else {
            panic!("Expected Bond interface type");
        }
    }

    #[test]
    fn test_parse_vlan_interface() {
        let content = r#"
auto eth0.100
iface eth0.100 inet static
    address 192.168.100.1/24
"#;
        let parser = InterfacesParser::new();
        let config = parser.parse(content).unwrap();

        let vlan_if = &config.interfaces["eth0.100"];
        assert_eq!(vlan_if.name, "eth0.100");

        if let InterfaceType::Vlan { parent, tag } = &vlan_if.iface_type {
            assert_eq!(parent, "eth0");
            assert_eq!(*tag, 100);
        } else {
            panic!("Expected VLAN interface type");
        }
    }

    #[test]
    fn test_parse_vxlan_interface() {
        let content = r#"
auto vxlan100
iface vxlan100 inet manual
    vxlan-id 100
    vxlan-local 192.168.1.1
    vxlan-remote 192.168.1.2
    vxlan-dstport 4789
"#;
        let parser = InterfacesParser::new();
        let config = parser.parse(content).unwrap();

        let vxlan_if = &config.interfaces["vxlan100"];
        assert_eq!(vxlan_if.name, "vxlan100");

        if let InterfaceType::Vxlan {
            id,
            local,
            remote,
            dstport,
        } = &vxlan_if.iface_type
        {
            assert_eq!(*id, 100);
            assert_eq!(local.addr, IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1)));
            assert_eq!(
                remote.as_ref().unwrap().addr,
                IpAddr::V4(Ipv4Addr::new(192, 168, 1, 2))
            );
            assert_eq!(*dstport, Some(4789));
        } else {
            panic!("Expected VXLAN interface type");
        }
    }

    #[test]
    fn test_parse_with_comments() {
        let content = r#"
# This is a comment
auto lo
iface lo inet loopback

# Main network interface
auto eth0
iface eth0 inet static
    address 192.168.1.10/24
    # This is the gateway
    gateway 192.168.1.1
"#;
        let parser = InterfacesParser::new();
        let config = parser.parse(content).unwrap();

        assert_eq!(config.interfaces.len(), 2);

        // Comments should be preserved
        let eth0 = &config.interfaces["eth0"];
        assert!(!eth0.comments.is_empty());
    }

    #[test]
    fn test_parse_hotplug_interfaces() {
        let content = r#"
allow-hotplug eth0 eth1
iface eth0 inet dhcp
iface eth1 inet dhcp
"#;
        let parser = InterfacesParser::new();
        let config = parser.parse(content).unwrap();

        assert_eq!(config.hotplug_interfaces, vec!["eth0", "eth1"]);
    }

    #[test]
    fn test_parse_ipv6_interface() {
        let content = r#"
iface eth0 inet6 static
    address 2001:db8::1/64
    gateway 2001:db8::1
"#;
        let parser = InterfacesParser::new();
        let config = parser.parse(content).unwrap();

        let eth0 = &config.interfaces["eth0"];
        assert_eq!(
            eth0.addresses[0].addr,
            IpAddr::V6("2001:db8::1".parse::<Ipv6Addr>().unwrap())
        );
        assert_eq!(eth0.addresses[0].prefix_len, Some(64));
    }

    #[test]
    fn test_parse_multiple_addresses() {
        let content = r#"
iface eth0 inet static
    address 192.168.1.10/24
    address 192.168.1.11/24
    address 192.168.1.12/24
    gateway 192.168.1.1
"#;
        let parser = InterfacesParser::new();
        let config = parser.parse(content).unwrap();

        let eth0 = &config.interfaces["eth0"];
        assert_eq!(eth0.addresses.len(), 3);
        assert_eq!(
            eth0.addresses[0].addr,
            IpAddr::V4(Ipv4Addr::new(192, 168, 1, 10))
        );
        assert_eq!(
            eth0.addresses[1].addr,
            IpAddr::V4(Ipv4Addr::new(192, 168, 1, 11))
        );
        assert_eq!(
            eth0.addresses[2].addr,
            IpAddr::V4(Ipv4Addr::new(192, 168, 1, 12))
        );
    }

    #[test]
    fn test_generate_simple_interface() {
        let mut config = NetworkConfiguration::default();

        let interface = Interface {
            name: "eth0".to_string(),
            iface_type: InterfaceType::Physical,
            method: AddressMethod::Static,
            addresses: vec!["192.168.1.10/24".parse().unwrap()],
            gateway: Some("192.168.1.1".parse().unwrap()),
            mtu: Some(1500),
            options: HashMap::new(),
            enabled: true,
            comments: Vec::new(),
        };

        config.interfaces.insert("eth0".to_string(), interface);
        config.auto_interfaces.push("eth0".to_string());

        let parser = InterfacesParser::new();
        let generated = parser.generate(&config).unwrap();

        assert!(generated.contains("auto eth0"));
        assert!(generated.contains("iface eth0 inet static"));
        assert!(generated.contains("address 192.168.1.10/24"));
        assert!(generated.contains("gateway 192.168.1.1"));
        assert!(generated.contains("mtu 1500"));
    }

    #[test]
    fn test_generate_bridge_interface() {
        let mut config = NetworkConfiguration::default();

        let interface = Interface {
            name: "vmbr0".to_string(),
            iface_type: InterfaceType::Bridge {
                ports: vec!["eth0".to_string(), "eth1".to_string()],
                vlan_aware: true,
            },
            method: AddressMethod::Static,
            addresses: vec!["192.168.1.1/24".parse().unwrap()],
            gateway: None,
            mtu: None,
            options: HashMap::new(),
            enabled: true,
            comments: Vec::new(),
        };

        config.interfaces.insert("vmbr0".to_string(), interface);

        let parser = InterfacesParser::new();
        let generated = parser.generate(&config).unwrap();

        assert!(generated.contains("iface vmbr0 inet static"));
        assert!(generated.contains("bridge-ports eth0 eth1"));
        assert!(generated.contains("bridge-vlan-aware yes"));
    }

    #[test]
    fn test_generate_bond_interface() {
        let mut config = NetworkConfiguration::default();

        let interface = Interface {
            name: "bond0".to_string(),
            iface_type: InterfaceType::Bond {
                slaves: vec!["eth0".to_string(), "eth1".to_string()],
                mode: BondMode::ActiveBackup,
                options: HashMap::new(),
            },
            method: AddressMethod::Static,
            addresses: vec!["192.168.1.10/24".parse().unwrap()],
            gateway: None,
            mtu: None,
            options: HashMap::new(),
            enabled: true,
            comments: Vec::new(),
        };

        config.interfaces.insert("bond0".to_string(), interface);

        let parser = InterfacesParser::new();
        let generated = parser.generate(&config).unwrap();

        assert!(generated.contains("iface bond0 inet static"));
        assert!(generated.contains("bond-slaves eth0 eth1"));
        assert!(generated.contains("bond-mode active-backup"));
    }

    #[test]
    fn test_roundtrip_parsing() {
        let original_content = r#"auto lo
iface lo inet loopback

auto eth0
iface eth0 inet static
    address 192.168.1.10/24
    gateway 192.168.1.1
    mtu 1500

auto vmbr0
iface vmbr0 inet static
    address 192.168.1.1/24
    bridge-ports eth1
    bridge-vlan-aware yes
"#;

        let parser = InterfacesParser::new();
        let config = parser.parse(original_content).unwrap();
        let generated = parser.generate(&config).unwrap();

        // Parse the generated content again
        let config2 = parser.parse(&generated).unwrap();

        // Compare key properties
        assert_eq!(config.interfaces.len(), config2.interfaces.len());
        assert_eq!(config.auto_interfaces, config2.auto_interfaces);

        for (name, interface1) in &config.interfaces {
            let interface2 = &config2.interfaces[name];
            assert_eq!(interface1.name, interface2.name);
            assert_eq!(interface1.method, interface2.method);
            assert_eq!(interface1.addresses, interface2.addresses);
            assert_eq!(interface1.gateway, interface2.gateway);
            assert_eq!(interface1.mtu, interface2.mtu);
        }
    }

    #[test]
    fn test_error_handling_invalid_interface_name() {
        let content = r#"
iface 123invalid inet static
    address 192.168.1.10/24
"#;
        let parser = InterfacesParser::new();
        let result = parser.parse(content);

        assert!(result.is_err());
        if let Err(NetworkError::Configuration(ConfigError::InvalidInterfaceName { name })) = result
        {
            assert_eq!(name, "123invalid");
        } else {
            panic!("Expected InvalidInterfaceName error");
        }
    }

    #[test]
    fn test_error_handling_invalid_ip() {
        let content = r#"
iface eth0 inet static
    address invalid.ip.address
"#;
        let parser = InterfacesParser::new();
        let result = parser.parse(content);

        assert!(result.is_err());
    }

    #[test]
    fn test_error_handling_option_outside_interface() {
        let content = r#"
    address 192.168.1.10/24
iface eth0 inet static
"#;
        let parser = InterfacesParser::new();
        let result = parser.parse(content);

        assert!(result.is_err());
    }

    #[test]
    fn test_netmask_conversion() {
        let parser = InterfacesParser::new();

        assert_eq!(parser.netmask_to_cidr("255.255.255.0").unwrap(), 24);
        assert_eq!(parser.netmask_to_cidr("255.255.0.0").unwrap(), 16);
        assert_eq!(parser.netmask_to_cidr("255.0.0.0").unwrap(), 8);
        assert_eq!(parser.netmask_to_cidr("255.255.255.128").unwrap(), 25);

        assert!(parser.netmask_to_cidr("invalid.netmask").is_err());
    }

    #[test]
    fn test_bond_mode_parsing() {
        let content = r#"
iface bond0 inet static
    bond-slaves eth0 eth1
    bond-mode 802.3ad
"#;
        let parser = InterfacesParser::new();
        let config = parser.parse(content).unwrap();

        let bond0 = &config.interfaces["bond0"];
        if let InterfaceType::Bond { mode, .. } = &bond0.iface_type {
            assert_eq!(*mode, BondMode::Ieee8023ad);
        } else {
            panic!("Expected Bond interface type");
        }
    }

    #[test]
    fn test_preserve_ordering() {
        let content = r#"
auto lo
iface lo inet loopback

auto eth0
iface eth0 inet dhcp

auto vmbr0
iface vmbr0 inet static
    address 192.168.1.1/24
    bridge-ports eth1
"#;
        let parser = InterfacesParser::new();
        let config = parser.parse(content).unwrap();

        assert_eq!(config.ordering, vec!["lo", "eth0", "vmbr0"]);
    }

    #[test]
    fn test_complex_configuration() {
        let content = r#"
# Loopback interface
auto lo
iface lo inet loopback

# Main uplink
auto eth0
iface eth0 inet manual

# Management bridge
auto vmbr0
iface vmbr0 inet static
    address 192.168.1.1/24
    gateway 192.168.1.254
    bridge-ports eth0
    bridge-stp off
    bridge-fd 0
    bridge-vlan-aware yes

# VLAN interface for guests
auto vmbr0.100
iface vmbr0.100 inet static
    address 192.168.100.1/24

# Bond interface
auto bond0
iface bond0 inet manual
    bond-slaves eth1 eth2
    bond-mode 802.3ad
    bond-miimon 100
    bond-lacp-rate 1

# VXLAN overlay
auto vxlan100
iface vxlan100 inet manual
    vxlan-id 100
    vxlan-local 192.168.1.1
    vxlan-dstport 4789
"#;

        let parser = InterfacesParser::new();
        let config = parser.parse(content).unwrap();

        assert_eq!(config.interfaces.len(), 6);
        assert_eq!(config.auto_interfaces.len(), 6);

        // Verify each interface type
        assert_eq!(config.interfaces["lo"].iface_type, InterfaceType::Loopback);
        assert_eq!(
            config.interfaces["eth0"].iface_type,
            InterfaceType::Physical
        );

        if let InterfaceType::Bridge { .. } = config.interfaces["vmbr0"].iface_type {
            // OK
        } else {
            panic!("vmbr0 should be a bridge");
        }

        if let InterfaceType::Vlan { parent, tag } = &config.interfaces["vmbr0.100"].iface_type {
            assert_eq!(parent, "vmbr0");
            assert_eq!(*tag, 100);
        } else {
            panic!("vmbr0.100 should be a VLAN interface");
        }

        if let InterfaceType::Bond { .. } = config.interfaces["bond0"].iface_type {
            // OK
        } else {
            panic!("bond0 should be a bond interface");
        }

        if let InterfaceType::Vxlan { .. } = config.interfaces["vxlan100"].iface_type {
            // OK
        } else {
            panic!("vxlan100 should be a VXLAN interface");
        }
    }
}
