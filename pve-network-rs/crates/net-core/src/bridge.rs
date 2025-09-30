//! Bridge interface management

use crate::error::NetworkError;
use crate::interface::InterfaceConfig;
use crate::types::{Interface, InterfaceType};
use crate::Result;
use std::collections::HashMap;

/// Bridge-specific configuration
#[derive(Debug, Clone)]
pub struct BridgeConfig {
    /// Bridge ports
    pub ports: Vec<String>,
    /// VLAN aware bridge
    pub vlan_aware: bool,
    /// Forward delay
    pub forward_delay: Option<u8>,
    /// Hello time
    pub hello_time: Option<u8>,
    /// Max age
    pub max_age: Option<u8>,
    /// Spanning tree protocol
    pub stp: Option<bool>,
    /// Bridge priority
    pub priority: Option<u16>,
    /// VLAN filtering
    pub vlan_filtering: Option<bool>,
    /// Default PVID for VLAN-aware bridge
    pub vlan_default_pvid: Option<u16>,
    /// VLAN protocol (802.1Q or 802.1ad)
    pub vlan_protocol: Option<VlanProtocol>,
    /// Multicast snooping
    pub multicast_snooping: Option<bool>,
    /// Multicast querier
    pub multicast_querier: Option<bool>,
    /// Additional bridge options
    pub options: HashMap<String, String>,
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

/// VLAN configuration for bridge ports
#[derive(Debug, Clone)]
pub struct BridgeVlanConfig {
    /// Port name
    pub port: String,
    /// VLAN IDs allowed on this port
    pub vids: Vec<u16>,
    /// VLAN ID ranges allowed on this port
    pub vid_ranges: Vec<(u16, u16)>,
    /// Port VLAN ID (PVID) - untagged VLAN
    pub pvid: Option<u16>,
    /// Whether port accepts untagged frames
    pub untagged: bool,
}

impl BridgeVlanConfig {
    /// Create new VLAN configuration for bridge port
    pub fn new(port: String) -> Self {
        Self {
            port,
            vids: Vec::new(),
            vid_ranges: Vec::new(),
            pvid: None,
            untagged: true,
        }
    }

    /// Add VLAN ID
    pub fn with_vid(mut self, vid: u16) -> Self {
        if !self.vids.contains(&vid) {
            self.vids.push(vid);
        }
        self
    }

    /// Add VLAN ID range
    pub fn with_vid_range(mut self, start: u16, end: u16) -> Self {
        if start <= end && start > 0 && end <= 4094 {
            self.vid_ranges.push((start, end));
        }
        self
    }

    /// Set PVID (Port VLAN ID)
    pub fn with_pvid(mut self, pvid: u16) -> Self {
        self.pvid = Some(pvid);
        self
    }

    /// Set untagged mode
    pub fn with_untagged(mut self, untagged: bool) -> Self {
        self.untagged = untagged;
        self
    }

    /// Get all VLAN IDs (individual + ranges)
    pub fn get_all_vids(&self) -> Vec<u16> {
        let mut all_vids = self.vids.clone();

        for (start, end) in &self.vid_ranges {
            for vid in *start..=*end {
                if !all_vids.contains(&vid) {
                    all_vids.push(vid);
                }
            }
        }

        all_vids.sort();
        all_vids
    }

    /// Check if VLAN ID is allowed
    pub fn is_vid_allowed(&self, vid: u16) -> bool {
        if self.vids.contains(&vid) {
            return true;
        }

        for (start, end) in &self.vid_ranges {
            if vid >= *start && vid <= *end {
                return true;
            }
        }

        false
    }
}

impl Default for BridgeConfig {
    fn default() -> Self {
        Self {
            ports: Vec::new(),
            vlan_aware: false,
            forward_delay: None,
            hello_time: None,
            max_age: None,
            stp: None,
            priority: None,
            vlan_filtering: None,
            vlan_default_pvid: None,
            vlan_protocol: None,
            multicast_snooping: None,
            multicast_querier: None,
            options: HashMap::new(),
        }
    }
}

impl BridgeConfig {
    /// Create new bridge configuration
    pub fn new() -> Self {
        Self::default()
    }

    /// Add bridge port
    pub fn with_port(mut self, port: String) -> Self {
        self.ports.push(port);
        self
    }

    /// Set VLAN aware
    pub fn with_vlan_aware(mut self, vlan_aware: bool) -> Self {
        self.vlan_aware = vlan_aware;
        self
    }

    /// Set forward delay
    pub fn with_forward_delay(mut self, delay: u8) -> Self {
        self.forward_delay = Some(delay);
        self
    }

    /// Set hello time
    pub fn with_hello_time(mut self, hello: u8) -> Self {
        self.hello_time = Some(hello);
        self
    }

    /// Set STP
    pub fn with_stp(mut self, stp: bool) -> Self {
        self.stp = Some(stp);
        self
    }

    /// Set bridge priority
    pub fn with_priority(mut self, priority: u16) -> Self {
        self.priority = Some(priority);
        self
    }

    /// Set VLAN filtering
    pub fn with_vlan_filtering(mut self, filtering: bool) -> Self {
        self.vlan_filtering = Some(filtering);
        self
    }

    /// Set default PVID
    pub fn with_vlan_default_pvid(mut self, pvid: u16) -> Self {
        self.vlan_default_pvid = Some(pvid);
        self
    }

    /// Set VLAN protocol
    pub fn with_vlan_protocol(mut self, protocol: VlanProtocol) -> Self {
        self.vlan_protocol = Some(protocol);
        self
    }

    /// Set multicast snooping
    pub fn with_multicast_snooping(mut self, snooping: bool) -> Self {
        self.multicast_snooping = Some(snooping);
        self
    }

    /// Set multicast querier
    pub fn with_multicast_querier(mut self, querier: bool) -> Self {
        self.multicast_querier = Some(querier);
        self
    }

    /// Add bridge option
    pub fn with_option(mut self, key: String, value: String) -> Self {
        self.options.insert(key, value);
        self
    }

    /// Convert to InterfaceConfig
    pub fn to_interface_config(self, name: String) -> InterfaceConfig {
        let mut config = InterfaceConfig::new(
            name,
            InterfaceType::Bridge {
                ports: self.ports,
                vlan_aware: self.vlan_aware,
            },
        );

        // Add bridge-specific options
        if let Some(fd) = self.forward_delay {
            config = config.with_option("bridge_fd".to_string(), fd.to_string());
        }
        if let Some(hello) = self.hello_time {
            config = config.with_option("bridge_hello".to_string(), hello.to_string());
        }
        if let Some(maxage) = self.max_age {
            config = config.with_option("bridge_maxage".to_string(), maxage.to_string());
        }
        if let Some(stp) = self.stp {
            config = config.with_option(
                "bridge_stp".to_string(),
                if stp { "on" } else { "off" }.to_string(),
            );
        }
        if let Some(priority) = self.priority {
            config = config.with_option("bridge_priority".to_string(), priority.to_string());
        }
        if let Some(filtering) = self.vlan_filtering {
            config = config.with_option(
                "bridge_vlan_filtering".to_string(),
                if filtering { "1" } else { "0" }.to_string(),
            );
        }
        if let Some(pvid) = self.vlan_default_pvid {
            config = config.with_option("bridge_vlan_default_pvid".to_string(), pvid.to_string());
        }
        if let Some(ref protocol) = self.vlan_protocol {
            config = config.with_option("bridge_vlan_protocol".to_string(), protocol.to_string());
        }
        if let Some(snooping) = self.multicast_snooping {
            config = config.with_option(
                "bridge_multicast_snooping".to_string(),
                if snooping { "1" } else { "0" }.to_string(),
            );
        }
        if let Some(querier) = self.multicast_querier {
            config = config.with_option(
                "bridge_multicast_querier".to_string(),
                if querier { "1" } else { "0" }.to_string(),
            );
        }

        // Add custom options
        for (key, value) in self.options {
            config = config.with_option(key, value);
        }

        config
    }
}

/// Bridge management operations
pub struct BridgeManager;

/// Advanced bridge operations for VLAN-aware bridges
pub struct VlanAwareBridgeManager;

impl BridgeManager {
    /// Validate bridge configuration
    pub fn validate_config(config: &BridgeConfig) -> Result<()> {
        // Validate forward delay (u8 max is 255, so no need to check upper bound)
        if let Some(_fd) = config.forward_delay {
            // Forward delay is already constrained by u8 type
        }

        // Validate hello time
        if let Some(hello) = config.hello_time {
            if hello < 1 || hello > 10 {
                return Err(NetworkError::Configuration(
                    crate::error::ConfigError::InvalidValue {
                        field: "bridge_hello".to_string(),
                        value: hello.to_string(),
                    },
                ));
            }
        }

        // Validate max age
        if let Some(maxage) = config.max_age {
            if maxage < 6 || maxage > 40 {
                return Err(NetworkError::Configuration(
                    crate::error::ConfigError::InvalidValue {
                        field: "bridge_maxage".to_string(),
                        value: maxage.to_string(),
                    },
                ));
            }
        }

        // Validate bridge priority
        if let Some(priority) = config.priority {
            if priority > 65535 {
                return Err(NetworkError::Configuration(
                    crate::error::ConfigError::InvalidValue {
                        field: "bridge_priority".to_string(),
                        value: priority.to_string(),
                    },
                ));
            }
        }

        // Validate VLAN default PVID
        if let Some(pvid) = config.vlan_default_pvid {
            if pvid == 0 || pvid > 4094 {
                return Err(NetworkError::Configuration(
                    crate::error::ConfigError::InvalidValue {
                        field: "bridge_vlan_default_pvid".to_string(),
                        value: pvid.to_string(),
                    },
                ));
            }
        }

        // Validate VLAN-aware specific settings
        if config.vlan_aware {
            // VLAN filtering should be enabled for VLAN-aware bridges
            if config.vlan_filtering == Some(false) {
                return Err(NetworkError::Configuration(
                    crate::error::ConfigError::InvalidValue {
                        field: "bridge_vlan_filtering".to_string(),
                        value: "VLAN-aware bridge requires VLAN filtering".to_string(),
                    },
                ));
            }
        }

        // Validate port names
        for port in &config.ports {
            crate::interface::InterfaceValidator::validate_name(port)?;
        }

        Ok(())
    }

    /// Check if interface is a bridge
    pub fn is_bridge(interface: &Interface) -> bool {
        matches!(interface.iface_type, InterfaceType::Bridge { .. })
    }

    /// Get bridge ports from interface
    pub fn get_ports(interface: &Interface) -> Option<&Vec<String>> {
        match &interface.iface_type {
            InterfaceType::Bridge { ports, .. } => Some(ports),
            _ => None,
        }
    }

    /// Check if bridge is VLAN aware
    pub fn is_vlan_aware(interface: &Interface) -> bool {
        match &interface.iface_type {
            InterfaceType::Bridge { vlan_aware, .. } => *vlan_aware,
            _ => false,
        }
    }

    /// Add port to bridge
    pub fn add_port(interface: &mut Interface, port: String) -> Result<()> {
        match &mut interface.iface_type {
            InterfaceType::Bridge { ports, .. } => {
                if !ports.contains(&port) {
                    ports.push(port);
                }
                Ok(())
            }
            _ => Err(NetworkError::Configuration(
                crate::error::ConfigError::InvalidValue {
                    field: "interface_type".to_string(),
                    value: "not a bridge".to_string(),
                },
            )),
        }
    }

    /// Remove port from bridge
    pub fn remove_port(interface: &mut Interface, port: &str) -> Result<()> {
        match &mut interface.iface_type {
            InterfaceType::Bridge { ports, .. } => {
                ports.retain(|p| p != port);
                Ok(())
            }
            _ => Err(NetworkError::Configuration(
                crate::error::ConfigError::InvalidValue {
                    field: "interface_type".to_string(),
                    value: "not a bridge".to_string(),
                },
            )),
        }
    }

    /// Enable VLAN awareness on bridge
    pub fn enable_vlan_aware(interface: &mut Interface) -> Result<()> {
        match &mut interface.iface_type {
            InterfaceType::Bridge { vlan_aware, .. } => {
                *vlan_aware = true;
                // Add default VLAN filtering option
                interface
                    .options
                    .insert("bridge_vlan_filtering".to_string(), "1".to_string());
                Ok(())
            }
            _ => Err(NetworkError::Configuration(
                crate::error::ConfigError::InvalidValue {
                    field: "interface_type".to_string(),
                    value: "not a bridge".to_string(),
                },
            )),
        }
    }

    /// Disable VLAN awareness on bridge
    pub fn disable_vlan_aware(interface: &mut Interface) -> Result<()> {
        match &mut interface.iface_type {
            InterfaceType::Bridge { vlan_aware, .. } => {
                *vlan_aware = false;
                // Remove VLAN-related options
                interface.options.remove("bridge_vlan_filtering");
                interface.options.remove("bridge_vlan_default_pvid");
                interface.options.remove("bridge_vlan_protocol");
                Ok(())
            }
            _ => Err(NetworkError::Configuration(
                crate::error::ConfigError::InvalidValue {
                    field: "interface_type".to_string(),
                    value: "not a bridge".to_string(),
                },
            )),
        }
    }

    /// Set bridge priority
    pub fn set_priority(interface: &mut Interface, priority: u16) -> Result<()> {
        if Self::is_bridge(interface) {
            interface
                .options
                .insert("bridge_priority".to_string(), priority.to_string());
            Ok(())
        } else {
            Err(NetworkError::Configuration(
                crate::error::ConfigError::InvalidValue {
                    field: "interface_type".to_string(),
                    value: "not a bridge".to_string(),
                },
            ))
        }
    }

    /// Set STP state
    pub fn set_stp(interface: &mut Interface, enabled: bool) -> Result<()> {
        if Self::is_bridge(interface) {
            interface.options.insert(
                "bridge_stp".to_string(),
                if enabled { "on" } else { "off" }.to_string(),
            );
            Ok(())
        } else {
            Err(NetworkError::Configuration(
                crate::error::ConfigError::InvalidValue {
                    field: "interface_type".to_string(),
                    value: "not a bridge".to_string(),
                },
            ))
        }
    }

    /// Get bridge configuration from interface
    pub fn get_bridge_config(interface: &Interface) -> Option<BridgeConfig> {
        if let InterfaceType::Bridge { ports, vlan_aware } = &interface.iface_type {
            let mut config = BridgeConfig {
                ports: ports.clone(),
                vlan_aware: *vlan_aware,
                ..Default::default()
            };

            // Parse options
            for (key, value) in &interface.options {
                match key.as_str() {
                    "bridge_fd" => config.forward_delay = value.parse().ok(),
                    "bridge_hello" => config.hello_time = value.parse().ok(),
                    "bridge_maxage" => config.max_age = value.parse().ok(),
                    "bridge_stp" => config.stp = Some(value == "on" || value == "1"),
                    "bridge_priority" => config.priority = value.parse().ok(),
                    "bridge_vlan_filtering" => config.vlan_filtering = Some(value == "1"),
                    "bridge_vlan_default_pvid" => config.vlan_default_pvid = value.parse().ok(),
                    "bridge_vlan_protocol" => {
                        config.vlan_protocol = match value.as_str() {
                            "802.1Q" => Some(VlanProtocol::Ieee8021Q),
                            "802.1ad" => Some(VlanProtocol::Ieee8021Ad),
                            _ => None,
                        };
                    }
                    "bridge_multicast_snooping" => config.multicast_snooping = Some(value == "1"),
                    "bridge_multicast_querier" => config.multicast_querier = Some(value == "1"),
                    _ => {
                        config.options.insert(key.clone(), value.clone());
                    }
                }
            }

            Some(config)
        } else {
            None
        }
    }
}

impl VlanAwareBridgeManager {
    /// Configure VLAN on bridge port
    pub fn configure_port_vlan(
        interface: &mut Interface,
        port: &str,
        vlan_config: BridgeVlanConfig,
    ) -> Result<()> {
        if !BridgeManager::is_bridge(interface) {
            return Err(NetworkError::Configuration(
                crate::error::ConfigError::InvalidValue {
                    field: "interface_type".to_string(),
                    value: "not a bridge".to_string(),
                },
            ));
        }

        if !BridgeManager::is_vlan_aware(interface) {
            return Err(NetworkError::Configuration(
                crate::error::ConfigError::InvalidValue {
                    field: "bridge_vlan_aware".to_string(),
                    value: "bridge is not VLAN-aware".to_string(),
                },
            ));
        }

        // Validate VLAN configuration
        Self::validate_vlan_config(&vlan_config)?;

        // Build VLAN configuration string
        let vlan_str = Self::build_vlan_string(&vlan_config);

        // Set bridge VLAN configuration for port
        let option_key = format!("bridge_vids_{}", port);
        interface.options.insert(option_key, vlan_str);

        // Set PVID if specified
        if let Some(pvid) = vlan_config.pvid {
            let pvid_key = format!("bridge_pvid_{}", port);
            interface.options.insert(pvid_key, pvid.to_string());
        }

        Ok(())
    }

    /// Remove VLAN configuration from bridge port
    pub fn remove_port_vlan(interface: &mut Interface, port: &str) -> Result<()> {
        if !BridgeManager::is_bridge(interface) {
            return Err(NetworkError::Configuration(
                crate::error::ConfigError::InvalidValue {
                    field: "interface_type".to_string(),
                    value: "not a bridge".to_string(),
                },
            ));
        }

        let vids_key = format!("bridge_vids_{}", port);
        let pvid_key = format!("bridge_pvid_{}", port);

        interface.options.remove(&vids_key);
        interface.options.remove(&pvid_key);

        Ok(())
    }

    /// Get VLAN configuration for bridge port
    pub fn get_port_vlan_config(interface: &Interface, port: &str) -> Option<BridgeVlanConfig> {
        if !BridgeManager::is_bridge(interface) {
            return None;
        }

        let vids_key = format!("bridge_vids_{}", port);
        let pvid_key = format!("bridge_pvid_{}", port);

        let mut config = BridgeVlanConfig::new(port.to_string());

        // Parse VLAN IDs
        if let Some(vids_str) = interface.options.get(&vids_key) {
            config = Self::parse_vlan_string(config, vids_str);
        }

        // Parse PVID
        if let Some(pvid_str) = interface.options.get(&pvid_key) {
            if let Ok(pvid) = pvid_str.parse::<u16>() {
                config.pvid = Some(pvid);
            }
        }

        Some(config)
    }

    /// Add VLAN to bridge port
    pub fn add_vlan_to_port(
        interface: &mut Interface,
        port: &str,
        vid: u16,
        untagged: bool,
    ) -> Result<()> {
        let mut vlan_config = Self::get_port_vlan_config(interface, port)
            .unwrap_or_else(|| BridgeVlanConfig::new(port.to_string()));

        vlan_config = vlan_config.with_vid(vid);

        if untagged {
            vlan_config = vlan_config.with_pvid(vid);
        }

        Self::configure_port_vlan(interface, port, vlan_config)
    }

    /// Remove VLAN from bridge port
    pub fn remove_vlan_from_port(interface: &mut Interface, port: &str, vid: u16) -> Result<()> {
        let mut vlan_config = Self::get_port_vlan_config(interface, port).ok_or_else(|| {
            NetworkError::Configuration(crate::error::ConfigError::InvalidValue {
                field: "bridge_vlan_config".to_string(),
                value: "no VLAN configuration found".to_string(),
            })
        })?;

        // Remove VID from allowed list
        vlan_config.vids.retain(|&v| v != vid);

        // Remove from ranges
        vlan_config
            .vid_ranges
            .retain(|(start, end)| !(vid >= *start && vid <= *end));

        // Clear PVID if it matches
        if vlan_config.pvid == Some(vid) {
            vlan_config.pvid = None;
        }

        Self::configure_port_vlan(interface, port, vlan_config)
    }

    /// Validate VLAN configuration
    fn validate_vlan_config(config: &BridgeVlanConfig) -> Result<()> {
        // Validate port name
        crate::interface::InterfaceValidator::validate_name(&config.port)?;

        // Validate VLAN IDs
        for &vid in &config.vids {
            if vid == 0 || vid > 4094 {
                return Err(NetworkError::Configuration(
                    crate::error::ConfigError::InvalidValue {
                        field: "vlan_id".to_string(),
                        value: vid.to_string(),
                    },
                ));
            }
        }

        // Validate VLAN ranges
        for (start, end) in &config.vid_ranges {
            if *start == 0 || *end > 4094 || start > end {
                return Err(NetworkError::Configuration(
                    crate::error::ConfigError::InvalidValue {
                        field: "vlan_range".to_string(),
                        value: format!("{}-{}", start, end),
                    },
                ));
            }
        }

        // Validate PVID
        if let Some(pvid) = config.pvid {
            if pvid == 0 || pvid > 4094 {
                return Err(NetworkError::Configuration(
                    crate::error::ConfigError::InvalidValue {
                        field: "pvid".to_string(),
                        value: pvid.to_string(),
                    },
                ));
            }
        }

        Ok(())
    }

    /// Build VLAN string for bridge configuration
    fn build_vlan_string(config: &BridgeVlanConfig) -> String {
        let mut parts = Vec::new();

        // Add individual VIDs
        for &vid in &config.vids {
            parts.push(vid.to_string());
        }

        // Add ranges
        for (start, end) in &config.vid_ranges {
            if start == end {
                parts.push(start.to_string());
            } else {
                parts.push(format!("{}-{}", start, end));
            }
        }

        parts.join(",")
    }

    /// Parse VLAN string into configuration
    fn parse_vlan_string(mut config: BridgeVlanConfig, vids_str: &str) -> BridgeVlanConfig {
        for part in vids_str.split(',') {
            let part = part.trim();
            if part.is_empty() {
                continue;
            }

            if let Some((start_str, end_str)) = part.split_once('-') {
                // Range
                if let (Ok(start), Ok(end)) = (start_str.parse::<u16>(), end_str.parse::<u16>()) {
                    config = config.with_vid_range(start, end);
                }
            } else {
                // Single VID
                if let Ok(vid) = part.parse::<u16>() {
                    config = config.with_vid(vid);
                }
            }
        }

        config
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::AddressMethod;

    #[test]
    fn test_bridge_config_creation() {
        let config = BridgeConfig::new()
            .with_port("eth0".to_string())
            .with_port("eth1".to_string())
            .with_vlan_aware(true)
            .with_stp(true);

        assert_eq!(config.ports.len(), 2);
        assert!(config.vlan_aware);
        assert_eq!(config.stp, Some(true));
    }

    #[test]
    fn test_bridge_validation() {
        let config = BridgeConfig::new()
            .with_forward_delay(15)
            .with_hello_time(2)
            .with_port("eth0".to_string());

        assert!(BridgeManager::validate_config(&config).is_ok());

        let invalid_config = BridgeConfig::new().with_hello_time(15); // Invalid hello time

        assert!(BridgeManager::validate_config(&invalid_config).is_err());
    }

    #[test]
    fn test_bridge_port_management() {
        let mut interface = Interface {
            name: "br0".to_string(),
            iface_type: InterfaceType::Bridge {
                ports: vec!["eth0".to_string()],
                vlan_aware: false,
            },
            method: AddressMethod::Manual,
            addresses: Vec::new(),
            gateway: None,
            mtu: None,
            options: HashMap::new(),
            enabled: true,
            comments: Vec::new(),
        };

        assert!(BridgeManager::is_bridge(&interface));
        assert_eq!(BridgeManager::get_ports(&interface).unwrap().len(), 1);

        BridgeManager::add_port(&mut interface, "eth1".to_string()).unwrap();
        assert_eq!(BridgeManager::get_ports(&interface).unwrap().len(), 2);

        BridgeManager::remove_port(&mut interface, "eth0").unwrap();
        assert_eq!(BridgeManager::get_ports(&interface).unwrap().len(), 1);
        assert_eq!(BridgeManager::get_ports(&interface).unwrap()[0], "eth1");
    }
}
