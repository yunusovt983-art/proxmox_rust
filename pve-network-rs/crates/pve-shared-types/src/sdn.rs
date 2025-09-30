use std::collections::{HashMap, HashSet};
use std::net::IpAddr;

use anyhow::{bail, Result};
use ipnet::IpNet;
use serde::{Deserialize, Serialize};

use crate::ipam::IpamConfig;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "lowercase")]
pub enum ZoneType {
    Simple,
    Vlan,
    QinQ,
    Vxlan,
    Evpn,
}

impl std::fmt::Display for ZoneType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ZoneType::Simple => write!(f, "simple"),
            ZoneType::Vlan => write!(f, "vlan"),
            ZoneType::QinQ => write!(f, "qinq"),
            ZoneType::Vxlan => write!(f, "vxlan"),
            ZoneType::Evpn => write!(f, "evpn"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ZoneConfig {
    #[serde(rename = "type")]
    pub zone_type: ZoneType,
    pub zone: String,
    pub bridge: Option<String>,
    #[serde(rename = "vlan-aware")]
    pub vlan_aware: Option<bool>,
    pub tag: Option<u16>,
    #[serde(rename = "vxlan-port")]
    pub vxlan_port: Option<u16>,
    pub peers: Option<Vec<String>>,
    pub mtu: Option<u16>,
    pub nodes: Option<Vec<String>>,
    #[serde(flatten)]
    pub options: HashMap<String, serde_json::Value>,
}

impl ZoneConfig {
    pub fn new(zone_type: ZoneType, zone: String) -> Self {
        Self {
            zone_type,
            zone,
            bridge: None,
            vlan_aware: None,
            tag: None,
            vxlan_port: None,
            peers: None,
            mtu: None,
            nodes: None,
            options: HashMap::new(),
        }
    }

    pub fn validate(&self) -> Result<()> {
        if self.zone.is_empty() {
            bail!("Zone name cannot be empty");
        }

        if let Some(tag) = self.tag {
            if tag == 0 || tag > 4094 {
                bail!("VLAN tag must be between 1 and 4094");
            }
        }

        if let Some(mtu) = self.mtu {
            if mtu < 68 {
                bail!("MTU must be at least 68");
            }
        }

        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VNetConfig {
    pub vnet: String,
    pub zone: String,
    pub tag: Option<u16>,
    pub alias: Option<String>,
    pub vlanaware: Option<bool>,
    pub mac: Option<String>,
    #[serde(flatten)]
    pub options: HashMap<String, serde_json::Value>,
}

impl VNetConfig {
    pub fn new(vnet: String, zone: String) -> Self {
        Self {
            vnet,
            zone,
            tag: None,
            alias: None,
            vlanaware: None,
            mac: None,
            options: HashMap::new(),
        }
    }

    pub fn validate(&self) -> Result<()> {
        if self.vnet.is_empty() {
            bail!("VNet name cannot be empty");
        }

        if self.zone.is_empty() {
            bail!("Zone name cannot be empty");
        }

        if let Some(tag) = self.tag {
            if tag == 0 || tag > 4094 {
                bail!("VLAN tag must be between 1 and 4094");
            }
        }

        if let Some(mac) = &self.mac {
            if !is_valid_mac(mac) {
                bail!("Invalid MAC address format");
            }
        }

        Ok(())
    }
}

fn is_valid_mac(mac: &str) -> bool {
    let parts: Vec<&str> = mac.split(':').collect();
    if parts.len() != 6 {
        return false;
    }

    parts
        .iter()
        .all(|part| part.len() == 2 && part.chars().all(|c| c.is_ascii_hexdigit()))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubnetConfig {
    pub subnet: String,
    pub vnet: String,
    #[serde(rename = "type")]
    pub subnet_type: SubnetType,
    pub cidr: IpNet,
    pub gateway: Option<IpAddr>,
    pub snat: Option<bool>,
    pub dhcp: Option<DhcpConfig>,
    #[serde(flatten)]
    pub options: HashMap<String, serde_json::Value>,
}

impl SubnetConfig {
    pub fn new(subnet: String, vnet: String, cidr: IpNet) -> Self {
        Self {
            subnet,
            vnet,
            subnet_type: SubnetType::Subnet,
            cidr,
            gateway: None,
            snat: None,
            dhcp: None,
            options: HashMap::new(),
        }
    }

    pub fn validate(&self) -> Result<()> {
        if self.subnet.is_empty() {
            bail!("Subnet name cannot be empty");
        }

        if self.vnet.is_empty() {
            bail!("VNet name cannot be empty");
        }

        if let Some(gateway) = self.gateway {
            if !self.cidr.contains(&gateway) {
                bail!(
                    "Gateway {gateway} is not within subnet {cidr}",
                    cidr = self.cidr
                );
            }
        }

        if let Some(dhcp) = &self.dhcp {
            if let Some(ranges) = &dhcp.dhcp_range {
                for range in ranges {
                    self.validate_dhcp_range(range)?;
                }
            }
        }

        Ok(())
    }

    fn validate_dhcp_range(&self, range: &str) -> Result<()> {
        let parts: Vec<&str> = range.split(',').collect();
        if parts.len() != 2 {
            bail!("DHCP range must be in format 'start,end'");
        }

        let start: IpAddr = parts[0]
            .parse()
            .map_err(|_| anyhow::anyhow!("Invalid start IP in DHCP range"))?;
        let end: IpAddr = parts[1]
            .parse()
            .map_err(|_| anyhow::anyhow!("Invalid end IP in DHCP range"))?;

        if !self.cidr.contains(&start) || !self.cidr.contains(&end) {
            bail!("DHCP range is not within subnet {}", self.cidr);
        }

        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum SubnetType {
    Subnet,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DhcpConfig {
    #[serde(rename = "dhcp-range")]
    pub dhcp_range: Option<Vec<String>>,
    #[serde(rename = "dns-server")]
    pub dns_server: Option<Vec<IpAddr>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "lowercase")]
pub enum ControllerType {
    Evpn,
    Bgp,
    Faucet,
}

impl std::fmt::Display for ControllerType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ControllerType::Evpn => write!(f, "evpn"),
            ControllerType::Bgp => write!(f, "bgp"),
            ControllerType::Faucet => write!(f, "faucet"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ControllerConfig {
    #[serde(rename = "type")]
    pub controller_type: ControllerType,
    pub controller: String,
    pub asn: Option<u32>,
    pub peers: Option<Vec<String>>,
    pub bgp_multipath_relax: Option<bool>,
    pub ebgp_requires_policy: Option<bool>,
    pub node: Option<String>,
    #[serde(flatten)]
    pub options: HashMap<String, serde_json::Value>,
}

impl ControllerConfig {
    pub fn new(controller_type: ControllerType, controller: String) -> Self {
        Self {
            controller_type,
            controller,
            asn: None,
            peers: None,
            bgp_multipath_relax: None,
            ebgp_requires_policy: None,
            node: None,
            options: HashMap::new(),
        }
    }

    pub fn validate(&self) -> Result<()> {
        if self.controller.is_empty() {
            bail!("Controller name cannot be empty");
        }

        if let Some(asn) = self.asn {
            if asn == 0 {
                bail!("ASN cannot be 0");
            }
        }

        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ControllerStatus {
    pub running: bool,
    pub pid: Option<u32>,
    pub uptime: Option<u64>,
    pub last_error: Option<String>,
    pub config_version: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SdnConfiguration {
    #[serde(default)]
    pub zones: HashMap<String, ZoneConfig>,
    #[serde(default)]
    pub vnets: HashMap<String, VNetConfig>,
    #[serde(default)]
    pub subnets: HashMap<String, SubnetConfig>,
    #[serde(default)]
    pub controllers: HashMap<String, ControllerConfig>,
    #[serde(default)]
    pub ipams: HashMap<String, IpamConfig>,
}

impl SdnConfiguration {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_zone(&mut self, config: ZoneConfig) -> Result<()> {
        config.validate()?;
        self.zones.insert(config.zone.clone(), config);
        Ok(())
    }

    pub fn add_vnet(&mut self, config: VNetConfig) -> Result<()> {
        config.validate()?;

        if !self.zones.contains_key(&config.zone) {
            bail!("Zone '{}' does not exist", config.zone);
        }

        self.vnets.insert(config.vnet.clone(), config);
        Ok(())
    }

    pub fn add_subnet(&mut self, config: SubnetConfig) -> Result<()> {
        config.validate()?;

        if !self.vnets.contains_key(&config.vnet) {
            bail!("VNet '{}' does not exist", config.vnet);
        }

        self.subnets.insert(config.subnet.clone(), config);
        Ok(())
    }

    pub fn add_ipam(&mut self, config: IpamConfig) -> Result<()> {
        config.validate()?;
        self.ipams.insert(config.name.clone(), config);
        Ok(())
    }

    pub fn remove_zone(&mut self, zone_name: &str) -> Result<()> {
        let dependent_vnets: Vec<_> = self
            .vnets
            .values()
            .filter(|vnet| vnet.zone == zone_name)
            .map(|vnet| vnet.vnet.clone())
            .collect();

        if !dependent_vnets.is_empty() {
            bail!(
                "Cannot remove zone '{}': VNets {:?} depend on it",
                zone_name,
                dependent_vnets
            );
        }

        self.zones.remove(zone_name);
        Ok(())
    }

    pub fn remove_vnet(&mut self, vnet_name: &str) -> Result<()> {
        let dependent_subnets: Vec<_> = self
            .subnets
            .values()
            .filter(|subnet| subnet.vnet == vnet_name)
            .map(|subnet| subnet.subnet.clone())
            .collect();

        if !dependent_subnets.is_empty() {
            bail!(
                "Cannot remove VNet '{}': Subnets {:?} depend on it",
                vnet_name,
                dependent_subnets
            );
        }

        self.vnets.remove(vnet_name);
        Ok(())
    }

    pub fn remove_subnet(&mut self, subnet_name: &str) -> Result<()> {
        self.subnets.remove(subnet_name);
        Ok(())
    }

    pub fn remove_ipam(&mut self, ipam_name: &str) -> Result<()> {
        self.ipams.remove(ipam_name);
        Ok(())
    }

    pub fn validate(&self) -> Result<()> {
        for zone in self.zones.values() {
            zone.validate()?;
        }

        for vnet in self.vnets.values() {
            vnet.validate()?;
            if !self.zones.contains_key(&vnet.zone) {
                bail!(
                    "VNet '{}' references non-existent zone '{}'",
                    vnet.vnet,
                    vnet.zone
                );
            }
        }

        for subnet in self.subnets.values() {
            subnet.validate()?;
            if !self.vnets.contains_key(&subnet.vnet) {
                bail!(
                    "Subnet '{}' references non-existent VNet '{}'",
                    subnet.subnet,
                    subnet.vnet
                );
            }
        }

        for ipam in self.ipams.values() {
            ipam.validate()?;
        }

        Ok(())
    }

    pub fn from_json(json: &str) -> Result<Self> {
        let config: SdnConfiguration = serde_json::from_str(json)?;
        config.validate()?;
        Ok(config)
    }

    pub fn to_json(&self) -> Result<String> {
        self.validate()?;
        Ok(serde_json::to_string_pretty(self)?)
    }

    pub fn referenced_zones(&self) -> HashSet<String> {
        self.vnets.values().map(|v| v.zone.clone()).collect()
    }
}
