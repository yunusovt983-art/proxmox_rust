//! SDN Subnet management

use ipnet::IpNet;
use serde::{Deserialize, Serialize};

pub use pve_shared_types::{DhcpConfig, SubnetConfig, SubnetType};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Subnet {
    pub config: SubnetConfig,
    pub status: SubnetStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SubnetStatus {
    Active,
    Inactive,
    Error(String),
}

impl Subnet {
    pub fn new(config: SubnetConfig) -> Self {
        Self {
            config,
            status: SubnetStatus::Inactive,
        }
    }

    pub fn name(&self) -> &str {
        &self.config.subnet
    }

    pub fn vnet(&self) -> &str {
        &self.config.vnet
    }

    pub fn cidr(&self) -> &IpNet {
        &self.config.cidr
    }
}
