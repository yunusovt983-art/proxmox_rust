//! VLAN interface management

use crate::error::NetworkError;
use crate::interface::InterfaceConfig;
use crate::types::{Interface, InterfaceType};
use crate::Result;

/// VLAN-specific configuration
#[derive(Debug, Clone)]
pub struct VlanConfig {
    /// Parent interface
    pub parent: String,
    /// VLAN tag
    pub tag: u16,
    /// VLAN protocol (802.1Q or 802.1ad)
    pub protocol: Option<VlanProtocol>,
    /// Ingress QoS mapping
    pub ingress_qos_map: Option<String>,
    /// Egress QoS mapping
    pub egress_qos_map: Option<String>,
    /// GVRP (GARP VLAN Registration Protocol)
    pub gvrp: Option<bool>,
    /// MVRP (Multiple VLAN Registration Protocol)
    pub mvrp: Option<bool>,
    /// Loose binding
    pub loose_binding: Option<bool>,
    /// Reorder header
    pub reorder_hdr: Option<bool>,
}

/// VLAN protocol types
#[derive(Debug, Clone, PartialEq)]
pub enum VlanProtocol {
    /// 802.1Q (standard VLAN)
    Ieee8021Q,
    /// 802.1ad (QinQ/Provider Bridge)
    Ieee8021Ad,
}

impl std::fmt::Display for VlanProtocol {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            VlanProtocol::Ieee8021Q => write!(f, "802.1Q"),
            VlanProtocol::Ieee8021Ad => write!(f, "802.1ad"),
        }
    }
}

/// QinQ (802.1ad) VLAN configuration
#[derive(Debug, Clone)]
pub struct QinQConfig {
    /// Outer VLAN tag (S-TAG)
    pub outer_tag: u16,
    /// Inner VLAN tag (C-TAG)
    pub inner_tag: u16,
    /// Parent interface
    pub parent: String,
}

impl QinQConfig {
    /// Create new QinQ configuration
    pub fn new(parent: String, outer_tag: u16, inner_tag: u16) -> Self {
        Self {
            parent,
            outer_tag,
            inner_tag,
        }
    }

    /// Generate QinQ interface name
    pub fn generate_name(&self) -> String {
        format!("{}.{}.{}", self.parent, self.outer_tag, self.inner_tag)
    }

    /// Convert to VLAN configuration
    pub fn to_vlan_config(self, name: String) -> VlanConfig {
        VlanConfig {
            parent: format!("{}.{}", self.parent, self.outer_tag),
            tag: self.inner_tag,
            protocol: Some(VlanProtocol::Ieee8021Q),
            ingress_qos_map: None,
            egress_qos_map: None,
            gvrp: None,
            mvrp: None,
            loose_binding: None,
            reorder_hdr: None,
        }
    }
}

impl VlanConfig {
    /// Create new VLAN configuration
    pub fn new(parent: String, tag: u16) -> Self {
        Self {
            parent,
            tag,
            protocol: None,
            ingress_qos_map: None,
            egress_qos_map: None,
            gvrp: None,
            mvrp: None,
            loose_binding: None,
            reorder_hdr: None,
        }
    }

    /// Set VLAN protocol
    pub fn with_protocol(mut self, protocol: VlanProtocol) -> Self {
        self.protocol = Some(protocol);
        self
    }

    /// Set ingress QoS mapping
    pub fn with_ingress_qos_map(mut self, mapping: String) -> Self {
        self.ingress_qos_map = Some(mapping);
        self
    }

    /// Set egress QoS mapping
    pub fn with_egress_qos_map(mut self, mapping: String) -> Self {
        self.egress_qos_map = Some(mapping);
        self
    }

    /// Enable GVRP
    pub fn with_gvrp(mut self, enabled: bool) -> Self {
        self.gvrp = Some(enabled);
        self
    }

    /// Enable MVRP
    pub fn with_mvrp(mut self, enabled: bool) -> Self {
        self.mvrp = Some(enabled);
        self
    }

    /// Set loose binding
    pub fn with_loose_binding(mut self, enabled: bool) -> Self {
        self.loose_binding = Some(enabled);
        self
    }

    /// Set reorder header
    pub fn with_reorder_hdr(mut self, enabled: bool) -> Self {
        self.reorder_hdr = Some(enabled);
        self
    }

    /// Convert to InterfaceConfig
    pub fn to_interface_config(self, name: String) -> InterfaceConfig {
        let mut config = InterfaceConfig::new(
            name,
            InterfaceType::Vlan {
                parent: self.parent,
                tag: self.tag,
            },
        );

        // Add VLAN-specific options
        if let Some(ref protocol) = self.protocol {
            config = config.with_option("vlan_protocol".to_string(), protocol.to_string());
        }
        if let Some(ref mapping) = self.ingress_qos_map {
            config = config.with_option("vlan_ingress_qos_map".to_string(), mapping.clone());
        }
        if let Some(ref mapping) = self.egress_qos_map {
            config = config.with_option("vlan_egress_qos_map".to_string(), mapping.clone());
        }
        if let Some(gvrp) = self.gvrp {
            config = config.with_option(
                "vlan_gvrp".to_string(),
                if gvrp { "on" } else { "off" }.to_string(),
            );
        }
        if let Some(mvrp) = self.mvrp {
            config = config.with_option(
                "vlan_mvrp".to_string(),
                if mvrp { "on" } else { "off" }.to_string(),
            );
        }
        if let Some(loose) = self.loose_binding {
            config = config.with_option(
                "vlan_loose_binding".to_string(),
                if loose { "on" } else { "off" }.to_string(),
            );
        }
        if let Some(reorder) = self.reorder_hdr {
            config = config.with_option(
                "vlan_reorder_hdr".to_string(),
                if reorder { "on" } else { "off" }.to_string(),
            );
        }

        config
    }

    /// Generate VLAN interface name from parent and tag
    pub fn generate_name(parent: &str, tag: u16) -> String {
        format!("{}.{}", parent, tag)
    }
}

/// VLAN management operations
pub struct VlanManager;

impl VlanManager {
    /// Validate VLAN configuration
    pub fn validate_config(config: &VlanConfig) -> Result<()> {
        // Validate parent interface name
        crate::interface::InterfaceValidator::validate_name(&config.parent)?;

        // Validate VLAN tag
        if config.tag == 0 || config.tag > 4094 {
            return Err(NetworkError::Configuration(
                crate::error::ConfigError::InvalidValue {
                    field: "vlan_tag".to_string(),
                    value: config.tag.to_string(),
                },
            ));
        }

        Ok(())
    }

    /// Check if interface is a VLAN
    pub fn is_vlan(interface: &Interface) -> bool {
        matches!(interface.iface_type, InterfaceType::Vlan { .. })
    }

    /// Get VLAN parent from interface
    pub fn get_parent(interface: &Interface) -> Option<&String> {
        match &interface.iface_type {
            InterfaceType::Vlan { parent, .. } => Some(parent),
            _ => None,
        }
    }

    /// Get VLAN tag from interface
    pub fn get_tag(interface: &Interface) -> Option<u16> {
        match &interface.iface_type {
            InterfaceType::Vlan { tag, .. } => Some(*tag),
            _ => None,
        }
    }

    /// Parse VLAN interface name to extract parent and tag
    pub fn parse_vlan_name(name: &str) -> Option<(String, u16)> {
        if let Some((parent, tag_str)) = name.rsplit_once('.') {
            if let Ok(tag) = tag_str.parse::<u16>() {
                if tag > 0 && tag <= 4094 {
                    return Some((parent.to_string(), tag));
                }
            }
        }
        None
    }

    /// Check if VLAN name is valid format
    pub fn is_valid_vlan_name(name: &str) -> bool {
        Self::parse_vlan_name(name).is_some()
    }

    /// Get all VLAN interfaces for a parent
    pub fn get_vlans_for_parent<'a>(
        interfaces: &'a [Interface],
        parent: &str,
    ) -> Vec<&'a Interface> {
        interfaces
            .iter()
            .filter(|iface| {
                if let Some(vlan_parent) = Self::get_parent(iface) {
                    vlan_parent == parent
                } else {
                    false
                }
            })
            .collect()
    }

    /// Check if VLAN tag is already used on parent
    pub fn is_tag_used(interfaces: &[Interface], parent: &str, tag: u16) -> bool {
        interfaces.iter().any(|iface| {
            if let InterfaceType::Vlan { parent: p, tag: t } = &iface.iface_type {
                p == parent && *t == tag
            } else {
                false
            }
        })
    }

    /// Get next available VLAN tag for parent
    pub fn get_next_available_tag(interfaces: &[Interface], parent: &str) -> Option<u16> {
        let used_tags: std::collections::HashSet<u16> = interfaces
            .iter()
            .filter_map(|iface| {
                if let InterfaceType::Vlan { parent: p, tag } = &iface.iface_type {
                    if p == parent {
                        Some(*tag)
                    } else {
                        None
                    }
                } else {
                    None
                }
            })
            .collect();

        for tag in 1..=4094 {
            if !used_tags.contains(&tag) {
                return Some(tag);
            }
        }
        None
    }

    /// Create QinQ (802.1ad) VLAN interface
    pub fn create_qinq_interface(
        parent: &str,
        outer_tag: u16,
        inner_tag: u16,
    ) -> Result<(String, VlanConfig)> {
        // Validate outer tag
        if outer_tag == 0 || outer_tag > 4094 {
            return Err(NetworkError::Configuration(
                crate::error::ConfigError::InvalidValue {
                    field: "outer_vlan_tag".to_string(),
                    value: outer_tag.to_string(),
                },
            ));
        }

        // Validate inner tag
        if inner_tag == 0 || inner_tag > 4094 {
            return Err(NetworkError::Configuration(
                crate::error::ConfigError::InvalidValue {
                    field: "inner_vlan_tag".to_string(),
                    value: inner_tag.to_string(),
                },
            ));
        }

        let qinq = QinQConfig::new(parent.to_string(), outer_tag, inner_tag);
        let interface_name = qinq.generate_name();
        let vlan_config = qinq.to_vlan_config(interface_name.clone());

        Ok((interface_name, vlan_config))
    }

    /// Parse QinQ interface name
    pub fn parse_qinq_name(name: &str) -> Option<(String, u16, u16)> {
        let parts: Vec<&str> = name.split('.').collect();
        if parts.len() == 3 {
            if let (Ok(outer_tag), Ok(inner_tag)) =
                (parts[1].parse::<u16>(), parts[2].parse::<u16>())
            {
                if outer_tag > 0 && outer_tag <= 4094 && inner_tag > 0 && inner_tag <= 4094 {
                    return Some((parts[0].to_string(), outer_tag, inner_tag));
                }
            }
        }
        None
    }

    /// Check if interface name is QinQ format
    pub fn is_qinq_name(name: &str) -> bool {
        Self::parse_qinq_name(name).is_some()
    }

    /// Set VLAN QoS mapping
    pub fn set_qos_mapping(
        interface: &mut Interface,
        ingress: Option<String>,
        egress: Option<String>,
    ) -> Result<()> {
        if !Self::is_vlan(interface) {
            return Err(NetworkError::Configuration(
                crate::error::ConfigError::InvalidValue {
                    field: "interface_type".to_string(),
                    value: "not a VLAN interface".to_string(),
                },
            ));
        }

        if let Some(ingress_map) = ingress {
            // Validate ingress mapping format (e.g., "1:2,3:4")
            if Self::validate_qos_mapping(&ingress_map)? {
                interface
                    .options
                    .insert("vlan_ingress_qos_map".to_string(), ingress_map);
            }
        }

        if let Some(egress_map) = egress {
            // Validate egress mapping format
            if Self::validate_qos_mapping(&egress_map)? {
                interface
                    .options
                    .insert("vlan_egress_qos_map".to_string(), egress_map);
            }
        }

        Ok(())
    }

    /// Enable/disable GVRP on VLAN interface
    pub fn set_gvrp(interface: &mut Interface, enabled: bool) -> Result<()> {
        if !Self::is_vlan(interface) {
            return Err(NetworkError::Configuration(
                crate::error::ConfigError::InvalidValue {
                    field: "interface_type".to_string(),
                    value: "not a VLAN interface".to_string(),
                },
            ));
        }

        interface.options.insert(
            "vlan_gvrp".to_string(),
            if enabled { "on" } else { "off" }.to_string(),
        );
        Ok(())
    }

    /// Enable/disable MVRP on VLAN interface
    pub fn set_mvrp(interface: &mut Interface, enabled: bool) -> Result<()> {
        if !Self::is_vlan(interface) {
            return Err(NetworkError::Configuration(
                crate::error::ConfigError::InvalidValue {
                    field: "interface_type".to_string(),
                    value: "not a VLAN interface".to_string(),
                },
            ));
        }

        interface.options.insert(
            "vlan_mvrp".to_string(),
            if enabled { "on" } else { "off" }.to_string(),
        );
        Ok(())
    }

    /// Get VLAN configuration from interface
    pub fn get_vlan_config(interface: &Interface) -> Option<VlanConfig> {
        if let InterfaceType::Vlan { parent, tag } = &interface.iface_type {
            let mut config = VlanConfig::new(parent.clone(), *tag);

            // Parse options
            for (key, value) in &interface.options {
                match key.as_str() {
                    "vlan_protocol" => {
                        config.protocol = match value.as_str() {
                            "802.1Q" => Some(VlanProtocol::Ieee8021Q),
                            "802.1ad" => Some(VlanProtocol::Ieee8021Ad),
                            _ => None,
                        };
                    }
                    "vlan_ingress_qos_map" => config.ingress_qos_map = Some(value.clone()),
                    "vlan_egress_qos_map" => config.egress_qos_map = Some(value.clone()),
                    "vlan_gvrp" => config.gvrp = Some(value == "on"),
                    "vlan_mvrp" => config.mvrp = Some(value == "on"),
                    "vlan_loose_binding" => config.loose_binding = Some(value == "on"),
                    "vlan_reorder_hdr" => config.reorder_hdr = Some(value == "on"),
                    _ => {}
                }
            }

            Some(config)
        } else {
            None
        }
    }

    /// Validate QoS mapping format
    fn validate_qos_mapping(mapping: &str) -> Result<bool> {
        for pair in mapping.split(',') {
            let pair = pair.trim();
            if let Some((from_str, to_str)) = pair.split_once(':') {
                let from = from_str.trim().parse::<u8>().map_err(|_| {
                    NetworkError::Configuration(crate::error::ConfigError::InvalidValue {
                        field: "qos_mapping".to_string(),
                        value: pair.to_string(),
                    })
                })?;
                let to = to_str.trim().parse::<u8>().map_err(|_| {
                    NetworkError::Configuration(crate::error::ConfigError::InvalidValue {
                        field: "qos_mapping".to_string(),
                        value: pair.to_string(),
                    })
                })?;

                // Validate priority values (0-7)
                if from > 7 || to > 7 {
                    return Err(NetworkError::Configuration(
                        crate::error::ConfigError::InvalidValue {
                            field: "qos_priority".to_string(),
                            value: format!("priority values must be 0-7, got {}:{}", from, to),
                        },
                    ));
                }
            } else {
                return Err(NetworkError::Configuration(
                    crate::error::ConfigError::InvalidValue {
                        field: "qos_mapping_format".to_string(),
                        value: format!("invalid format: {}, expected 'from:to'", pair),
                    },
                ));
            }
        }
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::AddressMethod;
    use std::collections::HashMap;

    #[test]
    fn test_vlan_config_creation() {
        let config = VlanConfig::new("eth0".to_string(), 100);
        assert_eq!(config.parent, "eth0");
        assert_eq!(config.tag, 100);
    }

    #[test]
    fn test_vlan_validation() {
        let config = VlanConfig::new("eth0".to_string(), 100);
        assert!(VlanManager::validate_config(&config).is_ok());

        let invalid_config = VlanConfig::new("eth0".to_string(), 0);
        assert!(VlanManager::validate_config(&invalid_config).is_err());

        let invalid_config2 = VlanConfig::new("eth0".to_string(), 5000);
        assert!(VlanManager::validate_config(&invalid_config2).is_err());
    }

    #[test]
    fn test_vlan_name_parsing() {
        assert_eq!(
            VlanManager::parse_vlan_name("eth0.100"),
            Some(("eth0".to_string(), 100))
        );
        assert_eq!(
            VlanManager::parse_vlan_name("br-test.200"),
            Some(("br-test".to_string(), 200))
        );

        assert_eq!(VlanManager::parse_vlan_name("eth0"), None);
        assert_eq!(VlanManager::parse_vlan_name("eth0.0"), None);
        assert_eq!(VlanManager::parse_vlan_name("eth0.5000"), None);
        assert_eq!(VlanManager::parse_vlan_name("eth0.abc"), None);
    }

    #[test]
    fn test_vlan_name_generation() {
        assert_eq!(VlanConfig::generate_name("eth0", 100), "eth0.100");
        assert_eq!(VlanConfig::generate_name("br-test", 200), "br-test.200");
    }

    #[test]
    fn test_vlan_interface_operations() {
        let interface = Interface {
            name: "eth0.100".to_string(),
            iface_type: InterfaceType::Vlan {
                parent: "eth0".to_string(),
                tag: 100,
            },
            method: AddressMethod::Manual,
            addresses: Vec::new(),
            gateway: None,
            mtu: None,
            options: HashMap::new(),
            enabled: true,
            comments: Vec::new(),
        };

        assert!(VlanManager::is_vlan(&interface));
        assert_eq!(VlanManager::get_parent(&interface).unwrap(), "eth0");
        assert_eq!(VlanManager::get_tag(&interface).unwrap(), 100);
    }

    #[test]
    fn test_vlan_tag_management() {
        let interfaces = vec![
            Interface {
                name: "eth0.100".to_string(),
                iface_type: InterfaceType::Vlan {
                    parent: "eth0".to_string(),
                    tag: 100,
                },
                method: AddressMethod::Manual,
                addresses: Vec::new(),
                gateway: None,
                mtu: None,
                options: HashMap::new(),
                enabled: true,
                comments: Vec::new(),
            },
            Interface {
                name: "eth0.200".to_string(),
                iface_type: InterfaceType::Vlan {
                    parent: "eth0".to_string(),
                    tag: 200,
                },
                method: AddressMethod::Manual,
                addresses: Vec::new(),
                gateway: None,
                mtu: None,
                options: HashMap::new(),
                enabled: true,
                comments: Vec::new(),
            },
        ];

        assert!(VlanManager::is_tag_used(&interfaces, "eth0", 100));
        assert!(!VlanManager::is_tag_used(&interfaces, "eth0", 300));
        assert!(!VlanManager::is_tag_used(&interfaces, "eth1", 100));

        let vlans = VlanManager::get_vlans_for_parent(&interfaces, "eth0");
        assert_eq!(vlans.len(), 2);

        let next_tag = VlanManager::get_next_available_tag(&interfaces, "eth0");
        assert_eq!(next_tag, Some(1));
    }

    #[test]
    fn test_qinq_configuration() {
        let qinq = QinQConfig::new("eth0".to_string(), 100, 200);
        assert_eq!(qinq.generate_name(), "eth0.100.200");

        let vlan_config = qinq.to_vlan_config("eth0.100.200".to_string());
        assert_eq!(vlan_config.parent, "eth0.100");
        assert_eq!(vlan_config.tag, 200);
    }

    #[test]
    fn test_vlan_config_with_options() {
        let config = VlanConfig::new("eth0".to_string(), 100)
            .with_protocol(VlanProtocol::Ieee8021Q)
            .with_gvrp(true)
            .with_ingress_qos_map("1:2,3:4".to_string());

        assert_eq!(config.protocol, Some(VlanProtocol::Ieee8021Q));
        assert_eq!(config.gvrp, Some(true));
        assert_eq!(config.ingress_qos_map, Some("1:2,3:4".to_string()));
    }
}
