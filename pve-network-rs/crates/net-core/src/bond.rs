//! Bond interface management

use crate::error::NetworkError;
use crate::interface::InterfaceConfig;
use crate::types::{BondMode, Interface, InterfaceType};
use crate::Result;
use std::collections::HashMap;

/// Bond-specific configuration
#[derive(Debug, Clone)]
pub struct BondConfig {
    /// Bond slaves
    pub slaves: Vec<String>,
    /// Bond mode
    pub mode: BondMode,
    /// MII monitoring interval
    pub miimon: Option<u32>,
    /// Up delay
    pub updelay: Option<u32>,
    /// Down delay
    pub downdelay: Option<u32>,
    /// ARP monitoring interval
    pub arp_interval: Option<u32>,
    /// ARP IP targets for monitoring
    pub arp_ip_target: Vec<String>,
    /// Primary slave interface
    pub primary: Option<String>,
    /// Primary reselect policy
    pub primary_reselect: Option<PrimaryReselect>,
    /// Fail over MAC policy
    pub fail_over_mac: Option<FailOverMac>,
    /// XMIT hash policy (for balance-xor and 802.3ad)
    pub xmit_hash_policy: Option<XmitHashPolicy>,
    /// LACP rate (for 802.3ad)
    pub lacp_rate: Option<LacpRate>,
    /// Ad select policy (for 802.3ad)
    pub ad_select: Option<AdSelect>,
    /// Minimum links (for 802.3ad)
    pub min_links: Option<u32>,
    /// All slaves active (for balance-tlb and balance-alb)
    pub all_slaves_active: Option<bool>,
    /// Resend IGMP after failover
    pub resend_igmp: Option<u32>,
    /// Additional bond options
    pub options: HashMap<String, String>,
}

/// Primary reselect policies
#[derive(Debug, Clone, PartialEq)]
pub enum PrimaryReselect {
    /// Always use primary when available
    Always,
    /// Use primary only when current active slave fails
    Better,
    /// Never reselect primary automatically
    Failure,
}

impl std::fmt::Display for PrimaryReselect {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PrimaryReselect::Always => write!(f, "always"),
            PrimaryReselect::Better => write!(f, "better"),
            PrimaryReselect::Failure => write!(f, "failure"),
        }
    }
}

/// Fail over MAC policies
#[derive(Debug, Clone, PartialEq)]
pub enum FailOverMac {
    /// No MAC address change
    None,
    /// Use active slave's MAC
    Active,
    /// Follow primary slave's MAC
    Follow,
}

impl std::fmt::Display for FailOverMac {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FailOverMac::None => write!(f, "none"),
            FailOverMac::Active => write!(f, "active"),
            FailOverMac::Follow => write!(f, "follow"),
        }
    }
}

/// XMIT hash policies
#[derive(Debug, Clone, PartialEq)]
pub enum XmitHashPolicy {
    /// Layer 2 (MAC addresses)
    Layer2,
    /// Layer 2+3 (MAC + IP addresses)
    Layer2Plus3,
    /// Layer 3+4 (IP + port numbers)
    Layer3Plus4,
    /// Encapsulation layer 2+3
    Encap2Plus3,
    /// Encapsulation layer 3+4
    Encap3Plus4,
}

impl std::fmt::Display for XmitHashPolicy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            XmitHashPolicy::Layer2 => write!(f, "layer2"),
            XmitHashPolicy::Layer2Plus3 => write!(f, "layer2+3"),
            XmitHashPolicy::Layer3Plus4 => write!(f, "layer3+4"),
            XmitHashPolicy::Encap2Plus3 => write!(f, "encap2+3"),
            XmitHashPolicy::Encap3Plus4 => write!(f, "encap3+4"),
        }
    }
}

/// LACP rates
#[derive(Debug, Clone, PartialEq)]
pub enum LacpRate {
    /// Slow LACP (30 seconds)
    Slow,
    /// Fast LACP (1 second)
    Fast,
}

impl std::fmt::Display for LacpRate {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LacpRate::Slow => write!(f, "slow"),
            LacpRate::Fast => write!(f, "fast"),
        }
    }
}

/// Ad select policies
#[derive(Debug, Clone, PartialEq)]
pub enum AdSelect {
    /// Stable aggregator selection
    Stable,
    /// Bandwidth-based selection
    Bandwidth,
    /// Count-based selection
    Count,
}

impl std::fmt::Display for AdSelect {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AdSelect::Stable => write!(f, "stable"),
            AdSelect::Bandwidth => write!(f, "bandwidth"),
            AdSelect::Count => write!(f, "count"),
        }
    }
}

impl BondConfig {
    /// Create new bond configuration
    pub fn new(mode: BondMode) -> Self {
        Self {
            slaves: Vec::new(),
            mode,
            miimon: None,
            updelay: None,
            downdelay: None,
            arp_interval: None,
            arp_ip_target: Vec::new(),
            primary: None,
            primary_reselect: None,
            fail_over_mac: None,
            xmit_hash_policy: None,
            lacp_rate: None,
            ad_select: None,
            min_links: None,
            all_slaves_active: None,
            resend_igmp: None,
            options: HashMap::new(),
        }
    }

    /// Add bond slave
    pub fn with_slave(mut self, slave: String) -> Self {
        self.slaves.push(slave);
        self
    }

    /// Set MII monitoring interval
    pub fn with_miimon(mut self, miimon: u32) -> Self {
        self.miimon = Some(miimon);
        self
    }

    /// Set up delay
    pub fn with_updelay(mut self, updelay: u32) -> Self {
        self.updelay = Some(updelay);
        self
    }

    /// Set down delay
    pub fn with_downdelay(mut self, downdelay: u32) -> Self {
        self.downdelay = Some(downdelay);
        self
    }

    /// Set ARP monitoring interval
    pub fn with_arp_interval(mut self, arp_interval: u32) -> Self {
        self.arp_interval = Some(arp_interval);
        self
    }

    /// Add ARP IP target
    pub fn with_arp_ip_target(mut self, target: String) -> Self {
        if !self.arp_ip_target.contains(&target) {
            self.arp_ip_target.push(target);
        }
        self
    }

    /// Set primary slave
    pub fn with_primary(mut self, primary: String) -> Self {
        self.primary = Some(primary);
        self
    }

    /// Set primary reselect policy
    pub fn with_primary_reselect(mut self, policy: PrimaryReselect) -> Self {
        self.primary_reselect = Some(policy);
        self
    }

    /// Set fail over MAC policy
    pub fn with_fail_over_mac(mut self, policy: FailOverMac) -> Self {
        self.fail_over_mac = Some(policy);
        self
    }

    /// Set XMIT hash policy
    pub fn with_xmit_hash_policy(mut self, policy: XmitHashPolicy) -> Self {
        self.xmit_hash_policy = Some(policy);
        self
    }

    /// Set LACP rate
    pub fn with_lacp_rate(mut self, rate: LacpRate) -> Self {
        self.lacp_rate = Some(rate);
        self
    }

    /// Set ad select policy
    pub fn with_ad_select(mut self, policy: AdSelect) -> Self {
        self.ad_select = Some(policy);
        self
    }

    /// Set minimum links
    pub fn with_min_links(mut self, min_links: u32) -> Self {
        self.min_links = Some(min_links);
        self
    }

    /// Set all slaves active
    pub fn with_all_slaves_active(mut self, active: bool) -> Self {
        self.all_slaves_active = Some(active);
        self
    }

    /// Set resend IGMP
    pub fn with_resend_igmp(mut self, resend: u32) -> Self {
        self.resend_igmp = Some(resend);
        self
    }

    /// Add bond option
    pub fn with_option(mut self, key: String, value: String) -> Self {
        self.options.insert(key, value);
        self
    }

    /// Convert to InterfaceConfig
    pub fn to_interface_config(self, name: String) -> InterfaceConfig {
        let mut config = InterfaceConfig::new(
            name,
            InterfaceType::Bond {
                slaves: self.slaves,
                mode: self.mode,
                options: self.options.clone(),
            },
        );

        // Add bond-specific options
        if let Some(miimon) = self.miimon {
            config = config.with_option("bond_miimon".to_string(), miimon.to_string());
        }
        if let Some(updelay) = self.updelay {
            config = config.with_option("bond_updelay".to_string(), updelay.to_string());
        }
        if let Some(downdelay) = self.downdelay {
            config = config.with_option("bond_downdelay".to_string(), downdelay.to_string());
        }
        if let Some(arp_interval) = self.arp_interval {
            config = config.with_option("bond_arp_interval".to_string(), arp_interval.to_string());
        }
        if !self.arp_ip_target.is_empty() {
            config = config.with_option(
                "bond_arp_ip_target".to_string(),
                self.arp_ip_target.join(","),
            );
        }
        if let Some(ref primary) = self.primary {
            config = config.with_option("bond_primary".to_string(), primary.clone());
        }
        if let Some(ref policy) = self.primary_reselect {
            config = config.with_option("bond_primary_reselect".to_string(), policy.to_string());
        }
        if let Some(ref policy) = self.fail_over_mac {
            config = config.with_option("bond_fail_over_mac".to_string(), policy.to_string());
        }
        if let Some(ref policy) = self.xmit_hash_policy {
            config = config.with_option("bond_xmit_hash_policy".to_string(), policy.to_string());
        }
        if let Some(ref rate) = self.lacp_rate {
            config = config.with_option("bond_lacp_rate".to_string(), rate.to_string());
        }
        if let Some(ref policy) = self.ad_select {
            config = config.with_option("bond_ad_select".to_string(), policy.to_string());
        }
        if let Some(min_links) = self.min_links {
            config = config.with_option("bond_min_links".to_string(), min_links.to_string());
        }
        if let Some(active) = self.all_slaves_active {
            config = config.with_option(
                "bond_all_slaves_active".to_string(),
                if active { "1" } else { "0" }.to_string(),
            );
        }
        if let Some(resend) = self.resend_igmp {
            config = config.with_option("bond_resend_igmp".to_string(), resend.to_string());
        }

        // Add custom options
        for (key, value) in self.options {
            config = config.with_option(key, value);
        }

        config
    }
}

/// Bond management operations
pub struct BondManager;

impl BondManager {
    /// Validate bond configuration
    pub fn validate_config(config: &BondConfig) -> Result<()> {
        // Must have at least one slave
        if config.slaves.is_empty() {
            return Err(NetworkError::Configuration(
                crate::error::ConfigError::MissingField {
                    field: "bond_slaves".to_string(),
                },
            ));
        }

        // Validate slave names
        for slave in &config.slaves {
            crate::interface::InterfaceValidator::validate_name(slave)?;
        }

        // Validate mode-specific requirements
        match config.mode {
            BondMode::Ieee8023ad => {
                // 802.3ad requires miimon or arp_interval
                if config.miimon.is_none() && config.arp_interval.is_none() {
                    return Err(NetworkError::Configuration(
                        crate::error::ConfigError::MissingField {
                            field: "bond_miimon or bond_arp_interval".to_string(),
                        },
                    ));
                }
            }
            BondMode::ActiveBackup => {
                // Active-backup can use primary slave
                if let Some(ref primary) = config.primary {
                    if !config.slaves.contains(primary) {
                        return Err(NetworkError::Configuration(
                            crate::error::ConfigError::InvalidValue {
                                field: "bond_primary".to_string(),
                                value: format!("primary slave {} not in slaves list", primary),
                            },
                        ));
                    }
                }
            }
            _ => {}
        }

        // Validate monitoring configuration
        if config.miimon.is_some() && config.arp_interval.is_some() {
            return Err(NetworkError::Configuration(
                crate::error::ConfigError::InvalidValue {
                    field: "bond_monitoring".to_string(),
                    value: "cannot use both miimon and arp_interval".to_string(),
                },
            ));
        }

        // Validate ARP targets if ARP monitoring is used
        if config.arp_interval.is_some() && config.arp_ip_target.is_empty() {
            return Err(NetworkError::Configuration(
                crate::error::ConfigError::MissingField {
                    field: "bond_arp_ip_target".to_string(),
                },
            ));
        }

        // Validate ARP IP targets format
        for target in &config.arp_ip_target {
            if target.parse::<std::net::IpAddr>().is_err() {
                return Err(NetworkError::Configuration(
                    crate::error::ConfigError::InvalidValue {
                        field: "bond_arp_ip_target".to_string(),
                        value: target.clone(),
                    },
                ));
            }
        }

        // Validate timing parameters
        if let Some(miimon) = config.miimon {
            if miimon == 0 {
                return Err(NetworkError::Configuration(
                    crate::error::ConfigError::InvalidValue {
                        field: "bond_miimon".to_string(),
                        value: miimon.to_string(),
                    },
                ));
            }
        }

        Ok(())
    }

    /// Check if interface is a bond
    pub fn is_bond(interface: &Interface) -> bool {
        matches!(interface.iface_type, InterfaceType::Bond { .. })
    }

    /// Get bond slaves from interface
    pub fn get_slaves(interface: &Interface) -> Option<&Vec<String>> {
        match &interface.iface_type {
            InterfaceType::Bond { slaves, .. } => Some(slaves),
            _ => None,
        }
    }

    /// Get bond mode from interface
    pub fn get_mode(interface: &Interface) -> Option<&BondMode> {
        match &interface.iface_type {
            InterfaceType::Bond { mode, .. } => Some(mode),
            _ => None,
        }
    }

    /// Add slave to bond
    pub fn add_slave(interface: &mut Interface, slave: String) -> Result<()> {
        match &mut interface.iface_type {
            InterfaceType::Bond { slaves, .. } => {
                if !slaves.contains(&slave) {
                    slaves.push(slave);
                }
                Ok(())
            }
            _ => Err(NetworkError::Configuration(
                crate::error::ConfigError::InvalidValue {
                    field: "interface_type".to_string(),
                    value: "not a bond".to_string(),
                },
            )),
        }
    }

    /// Remove slave from bond
    pub fn remove_slave(interface: &mut Interface, slave: &str) -> Result<()> {
        match &mut interface.iface_type {
            InterfaceType::Bond { slaves, .. } => {
                slaves.retain(|s| s != slave);
                if slaves.is_empty() {
                    return Err(NetworkError::Configuration(
                        crate::error::ConfigError::InvalidValue {
                            field: "bond_slaves".to_string(),
                            value: "cannot remove last slave".to_string(),
                        },
                    ));
                }
                Ok(())
            }
            _ => Err(NetworkError::Configuration(
                crate::error::ConfigError::InvalidValue {
                    field: "interface_type".to_string(),
                    value: "not a bond".to_string(),
                },
            )),
        }
    }

    /// Get bond mode string for configuration
    pub fn mode_to_string(mode: &BondMode) -> &'static str {
        match mode {
            BondMode::RoundRobin => "balance-rr",
            BondMode::ActiveBackup => "active-backup",
            BondMode::Xor => "balance-xor",
            BondMode::Broadcast => "broadcast",
            BondMode::Ieee8023ad => "802.3ad",
            BondMode::BalanceTlb => "balance-tlb",
            BondMode::BalanceAlb => "balance-alb",
        }
    }

    /// Set primary slave for active-backup mode
    pub fn set_primary_slave(interface: &mut Interface, primary: String) -> Result<()> {
        match &interface.iface_type {
            InterfaceType::Bond { slaves, mode, .. } => {
                if *mode != BondMode::ActiveBackup {
                    return Err(NetworkError::Configuration(
                        crate::error::ConfigError::InvalidValue {
                            field: "bond_mode".to_string(),
                            value: "primary slave only supported in active-backup mode".to_string(),
                        },
                    ));
                }

                if !slaves.contains(&primary) {
                    return Err(NetworkError::Configuration(
                        crate::error::ConfigError::InvalidValue {
                            field: "bond_primary".to_string(),
                            value: format!("primary slave {} not in slaves list", primary),
                        },
                    ));
                }

                interface
                    .options
                    .insert("bond_primary".to_string(), primary);
                Ok(())
            }
            _ => Err(NetworkError::Configuration(
                crate::error::ConfigError::InvalidValue {
                    field: "interface_type".to_string(),
                    value: "not a bond".to_string(),
                },
            )),
        }
    }

    /// Set XMIT hash policy for load balancing modes
    pub fn set_xmit_hash_policy(interface: &mut Interface, policy: XmitHashPolicy) -> Result<()> {
        match &interface.iface_type {
            InterfaceType::Bond { mode, .. } => match mode {
                BondMode::Xor | BondMode::Ieee8023ad => {
                    interface
                        .options
                        .insert("bond_xmit_hash_policy".to_string(), policy.to_string());
                    Ok(())
                }
                _ => Err(NetworkError::Configuration(
                    crate::error::ConfigError::InvalidValue {
                        field: "bond_mode".to_string(),
                        value: "xmit_hash_policy only supported in balance-xor and 802.3ad modes"
                            .to_string(),
                    },
                )),
            },
            _ => Err(NetworkError::Configuration(
                crate::error::ConfigError::InvalidValue {
                    field: "interface_type".to_string(),
                    value: "not a bond".to_string(),
                },
            )),
        }
    }

    /// Set LACP rate for 802.3ad mode
    pub fn set_lacp_rate(interface: &mut Interface, rate: LacpRate) -> Result<()> {
        match &interface.iface_type {
            InterfaceType::Bond { mode, .. } => {
                if *mode != BondMode::Ieee8023ad {
                    return Err(NetworkError::Configuration(
                        crate::error::ConfigError::InvalidValue {
                            field: "bond_mode".to_string(),
                            value: "lacp_rate only supported in 802.3ad mode".to_string(),
                        },
                    ));
                }

                interface
                    .options
                    .insert("bond_lacp_rate".to_string(), rate.to_string());
                Ok(())
            }
            _ => Err(NetworkError::Configuration(
                crate::error::ConfigError::InvalidValue {
                    field: "interface_type".to_string(),
                    value: "not a bond".to_string(),
                },
            )),
        }
    }

    /// Configure ARP monitoring
    pub fn configure_arp_monitoring(
        interface: &mut Interface,
        interval: u32,
        targets: Vec<String>,
    ) -> Result<()> {
        if !Self::is_bond(interface) {
            return Err(NetworkError::Configuration(
                crate::error::ConfigError::InvalidValue {
                    field: "interface_type".to_string(),
                    value: "not a bond".to_string(),
                },
            ));
        }

        // Validate ARP targets
        for target in &targets {
            if target.parse::<std::net::IpAddr>().is_err() {
                return Err(NetworkError::Configuration(
                    crate::error::ConfigError::InvalidValue {
                        field: "bond_arp_ip_target".to_string(),
                        value: target.clone(),
                    },
                ));
            }
        }

        // Remove MII monitoring if present
        interface.options.remove("bond_miimon");
        interface.options.remove("bond_updelay");
        interface.options.remove("bond_downdelay");

        // Set ARP monitoring
        interface
            .options
            .insert("bond_arp_interval".to_string(), interval.to_string());
        interface
            .options
            .insert("bond_arp_ip_target".to_string(), targets.join(","));

        Ok(())
    }

    /// Configure MII monitoring
    pub fn configure_mii_monitoring(
        interface: &mut Interface,
        miimon: u32,
        updelay: Option<u32>,
        downdelay: Option<u32>,
    ) -> Result<()> {
        if !Self::is_bond(interface) {
            return Err(NetworkError::Configuration(
                crate::error::ConfigError::InvalidValue {
                    field: "interface_type".to_string(),
                    value: "not a bond".to_string(),
                },
            ));
        }

        // Remove ARP monitoring if present
        interface.options.remove("bond_arp_interval");
        interface.options.remove("bond_arp_ip_target");

        // Set MII monitoring
        interface
            .options
            .insert("bond_miimon".to_string(), miimon.to_string());

        if let Some(up) = updelay {
            interface
                .options
                .insert("bond_updelay".to_string(), up.to_string());
        }

        if let Some(down) = downdelay {
            interface
                .options
                .insert("bond_downdelay".to_string(), down.to_string());
        }

        Ok(())
    }

    /// Get bond configuration from interface
    pub fn get_bond_config(interface: &Interface) -> Option<BondConfig> {
        if let InterfaceType::Bond {
            slaves,
            mode,
            options,
        } = &interface.iface_type
        {
            let mut config = BondConfig {
                slaves: slaves.clone(),
                mode: mode.clone(),
                options: options.clone(),
                ..BondConfig::new(mode.clone())
            };

            // Parse interface options
            for (key, value) in &interface.options {
                match key.as_str() {
                    "bond_miimon" => config.miimon = value.parse().ok(),
                    "bond_updelay" => config.updelay = value.parse().ok(),
                    "bond_downdelay" => config.downdelay = value.parse().ok(),
                    "bond_arp_interval" => config.arp_interval = value.parse().ok(),
                    "bond_arp_ip_target" => {
                        config.arp_ip_target =
                            value.split(',').map(|s| s.trim().to_string()).collect();
                    }
                    "bond_primary" => config.primary = Some(value.clone()),
                    "bond_primary_reselect" => {
                        config.primary_reselect = match value.as_str() {
                            "always" => Some(PrimaryReselect::Always),
                            "better" => Some(PrimaryReselect::Better),
                            "failure" => Some(PrimaryReselect::Failure),
                            _ => None,
                        };
                    }
                    "bond_fail_over_mac" => {
                        config.fail_over_mac = match value.as_str() {
                            "none" => Some(FailOverMac::None),
                            "active" => Some(FailOverMac::Active),
                            "follow" => Some(FailOverMac::Follow),
                            _ => None,
                        };
                    }
                    "bond_xmit_hash_policy" => {
                        config.xmit_hash_policy = match value.as_str() {
                            "layer2" => Some(XmitHashPolicy::Layer2),
                            "layer2+3" => Some(XmitHashPolicy::Layer2Plus3),
                            "layer3+4" => Some(XmitHashPolicy::Layer3Plus4),
                            "encap2+3" => Some(XmitHashPolicy::Encap2Plus3),
                            "encap3+4" => Some(XmitHashPolicy::Encap3Plus4),
                            _ => None,
                        };
                    }
                    "bond_lacp_rate" => {
                        config.lacp_rate = match value.as_str() {
                            "slow" => Some(LacpRate::Slow),
                            "fast" => Some(LacpRate::Fast),
                            _ => None,
                        };
                    }
                    "bond_ad_select" => {
                        config.ad_select = match value.as_str() {
                            "stable" => Some(AdSelect::Stable),
                            "bandwidth" => Some(AdSelect::Bandwidth),
                            "count" => Some(AdSelect::Count),
                            _ => None,
                        };
                    }
                    "bond_min_links" => config.min_links = value.parse().ok(),
                    "bond_all_slaves_active" => config.all_slaves_active = Some(value == "1"),
                    "bond_resend_igmp" => config.resend_igmp = value.parse().ok(),
                    _ => {}
                }
            }

            Some(config)
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::AddressMethod;

    #[test]
    fn test_bond_config_creation() {
        let config = BondConfig::new(BondMode::ActiveBackup)
            .with_slave("eth0".to_string())
            .with_slave("eth1".to_string())
            .with_miimon(100);

        assert_eq!(config.slaves.len(), 2);
        assert_eq!(config.mode, BondMode::ActiveBackup);
        assert_eq!(config.miimon, Some(100));
    }

    #[test]
    fn test_bond_validation() {
        let config = BondConfig::new(BondMode::ActiveBackup)
            .with_slave("eth0".to_string())
            .with_miimon(100);

        assert!(BondManager::validate_config(&config).is_ok());

        let invalid_config = BondConfig::new(BondMode::ActiveBackup); // No slaves
        assert!(BondManager::validate_config(&invalid_config).is_err());

        let ieee_config = BondConfig::new(BondMode::Ieee8023ad)
            .with_slave("eth0".to_string())
            .with_miimon(100);
        assert!(BondManager::validate_config(&ieee_config).is_ok());

        let ieee_invalid = BondConfig::new(BondMode::Ieee8023ad).with_slave("eth0".to_string()); // Missing miimon
        assert!(BondManager::validate_config(&ieee_invalid).is_err());
    }

    #[test]
    fn test_bond_slave_management() {
        let mut interface = Interface {
            name: "bond0".to_string(),
            iface_type: InterfaceType::Bond {
                slaves: vec!["eth0".to_string()],
                mode: BondMode::ActiveBackup,
                options: HashMap::new(),
            },
            method: AddressMethod::Manual,
            addresses: Vec::new(),
            gateway: None,
            mtu: None,
            options: HashMap::new(),
            enabled: true,
            comments: Vec::new(),
        };

        assert!(BondManager::is_bond(&interface));
        assert_eq!(BondManager::get_slaves(&interface).unwrap().len(), 1);
        assert_eq!(
            BondManager::get_mode(&interface).unwrap(),
            &BondMode::ActiveBackup
        );

        BondManager::add_slave(&mut interface, "eth1".to_string()).unwrap();
        assert_eq!(BondManager::get_slaves(&interface).unwrap().len(), 2);

        BondManager::remove_slave(&mut interface, "eth0").unwrap();
        assert_eq!(BondManager::get_slaves(&interface).unwrap().len(), 1);
        assert_eq!(BondManager::get_slaves(&interface).unwrap()[0], "eth1");

        // Cannot remove last slave
        assert!(BondManager::remove_slave(&mut interface, "eth1").is_err());
    }

    #[test]
    fn test_bond_mode_strings() {
        assert_eq!(
            BondManager::mode_to_string(&BondMode::ActiveBackup),
            "active-backup"
        );
        assert_eq!(
            BondManager::mode_to_string(&BondMode::Ieee8023ad),
            "802.3ad"
        );
        assert_eq!(
            BondManager::mode_to_string(&BondMode::RoundRobin),
            "balance-rr"
        );
    }
}
