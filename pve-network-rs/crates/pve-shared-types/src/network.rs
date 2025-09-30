use std::collections::HashMap;
use std::net::IpAddr;
use std::str::FromStr;

use ipnet::{IpNet, Ipv4Net, Ipv6Net};
use mac_address::MacAddress;
use serde::{Deserialize, Serialize};

use crate::error::SharedTypeError;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct NetworkConfiguration {
    #[serde(default)]
    pub interfaces: HashMap<String, Interface>,
    #[serde(default)]
    pub auto_interfaces: Vec<String>,
    #[serde(default)]
    pub hotplug_interfaces: Vec<String>,
    #[serde(default)]
    pub comments: HashMap<String, String>,
    #[serde(default)]
    pub ordering: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Interface {
    pub name: String,
    pub iface_type: InterfaceType,
    pub method: AddressMethod,
    #[serde(default)]
    pub addresses: Vec<IpAddress>,
    pub gateway: Option<IpAddress>,
    pub mtu: Option<u16>,
    #[serde(default)]
    pub options: HashMap<String, String>,
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub comments: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum InterfaceType {
    Physical,
    Bridge {
        #[serde(default)]
        ports: Vec<String>,
        #[serde(default)]
        vlan_aware: bool,
    },
    Bond {
        #[serde(default)]
        slaves: Vec<String>,
        mode: BondMode,
        #[serde(default)]
        options: HashMap<String, String>,
    },
    Vlan {
        parent: String,
        tag: u16,
    },
    Vxlan {
        id: u32,
        local: IpAddress,
        #[serde(default)]
        remote: Option<IpAddress>,
        #[serde(default)]
        dstport: Option<u16>,
    },
    Loopback,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum AddressMethod {
    Static,
    Dhcp,
    Manual,
    None,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum BondMode {
    #[serde(rename = "balance-rr")]
    RoundRobin,
    #[serde(rename = "active-backup")]
    ActiveBackup,
    #[serde(rename = "balance-xor")]
    Xor,
    Broadcast,
    #[serde(rename = "802.3ad")]
    Ieee8023ad,
    #[serde(rename = "balance-tlb")]
    BalanceTlb,
    #[serde(rename = "balance-alb")]
    BalanceAlb,
}

impl FromStr for BondMode {
    type Err = SharedTypeError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "0" | "balance-rr" => Ok(BondMode::RoundRobin),
            "1" | "active-backup" => Ok(BondMode::ActiveBackup),
            "2" | "balance-xor" => Ok(BondMode::Xor),
            "3" | "broadcast" => Ok(BondMode::Broadcast),
            "4" | "802.3ad" => Ok(BondMode::Ieee8023ad),
            "5" | "balance-tlb" => Ok(BondMode::BalanceTlb),
            "6" | "balance-alb" => Ok(BondMode::BalanceAlb),
            other => Err(SharedTypeError::Unsupported(other.to_string())),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct IpAddress {
    pub addr: IpAddr,
    pub prefix_len: Option<u8>,
}

impl IpAddress {
    pub fn new(addr: IpAddr, prefix_len: Option<u8>) -> Self {
        Self { addr, prefix_len }
    }

    pub fn to_ipnet(&self) -> Option<IpNet> {
        self.prefix_len.map(|prefix| match self.addr {
            IpAddr::V4(addr) => IpNet::V4(Ipv4Net::new(addr, prefix).unwrap()),
            IpAddr::V6(addr) => IpNet::V6(Ipv6Net::new(addr, prefix).unwrap()),
        })
    }

    pub fn same_network(&self, other: &IpAddress) -> bool {
        match (self.to_ipnet(), other.to_ipnet()) {
            (Some(net1), Some(net2)) => net1.contains(&other.addr) || net2.contains(&self.addr),
            _ => false,
        }
    }
}

impl FromStr for IpAddress {
    type Err = SharedTypeError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if let Some((addr, prefix)) = s.split_once('/') {
            let addr = addr
                .parse::<IpAddr>()
                .map_err(|_| SharedTypeError::InvalidValue {
                    field: "ip_address",
                    value: s.to_string(),
                })?;
            let prefix_len = prefix
                .parse::<u8>()
                .map_err(|_| SharedTypeError::InvalidValue {
                    field: "prefix_length",
                    value: prefix.to_string(),
                })?;
            Ok(IpAddress::new(addr, Some(prefix_len)))
        } else {
            let addr = s
                .parse::<IpAddr>()
                .map_err(|_| SharedTypeError::InvalidValue {
                    field: "ip_address",
                    value: s.to_string(),
                })?;
            Ok(IpAddress::new(addr, None))
        }
    }
}

impl std::fmt::Display for IpAddress {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(prefix) = self.prefix_len {
            write!(f, "{}/{}", self.addr, prefix)
        } else {
            write!(f, "{}", self.addr)
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MacAddr(pub MacAddress);

struct MacAddrVisitor;

impl<'de> serde::de::Visitor<'de> for MacAddrVisitor {
    type Value = MacAddr;

    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        formatter.write_str("a MAC address string")
    }

    fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        v.parse::<MacAddress>()
            .map(MacAddr)
            .map_err(|_| E::custom(format!("invalid MAC address: {}", v)))
    }
}

impl<'de> Deserialize<'de> for MacAddr {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        deserializer.deserialize_str(MacAddrVisitor)
    }
}

impl Serialize for MacAddr {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.0.to_string())
    }
}

impl FromStr for MacAddr {
    type Err = SharedTypeError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        s.parse::<MacAddress>()
            .map(MacAddr)
            .map_err(|_| SharedTypeError::InvalidValue {
                field: "mac_address",
                value: s.to_string(),
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_ip() {
        let ip: IpAddress = "192.168.1.10/24".parse().unwrap();
        assert_eq!(ip.prefix_len, Some(24));
        assert!(ip.same_network(&"192.168.1.20/24".parse().unwrap()));
    }

    #[test]
    fn parse_mac() {
        let mac: MacAddr = "aa:bb:cc:dd:ee:ff".parse().unwrap();
        assert_eq!(mac.to_string(), "aa:bb:cc:dd:ee:ff");
    }
}
impl std::fmt::Display for MacAddr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}
