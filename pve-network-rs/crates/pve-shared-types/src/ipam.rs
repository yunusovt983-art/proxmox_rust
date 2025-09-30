use std::collections::HashMap;
use std::net::IpAddr;

use anyhow::{bail, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum IpamType {
    #[serde(rename = "pve")]
    Pve,
    #[serde(rename = "phpipam")]
    PhpIpam,
    #[serde(rename = "netbox")]
    NetBox,
}

impl std::fmt::Display for IpamType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            IpamType::Pve => write!(f, "pve"),
            IpamType::PhpIpam => write!(f, "phpipam"),
            IpamType::NetBox => write!(f, "netbox"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IpamConfig {
    #[serde(rename = "type")]
    pub ipam_type: IpamType,
    pub name: String,
    pub url: Option<String>,
    pub token: Option<String>,
    pub username: Option<String>,
    pub password: Option<String>,
    pub section: Option<String>,
    pub tenant: Option<String>,
    #[serde(flatten)]
    pub options: HashMap<String, serde_json::Value>,
}

impl IpamConfig {
    pub fn new(name: String, ipam_type: IpamType) -> Self {
        Self {
            name,
            ipam_type,
            url: None,
            token: None,
            username: None,
            password: None,
            section: None,
            tenant: None,
            options: HashMap::new(),
        }
    }

    pub fn validate(&self) -> Result<()> {
        if self.name.is_empty() {
            bail!("IPAM name cannot be empty");
        }

        match self.ipam_type {
            IpamType::Pve => {}
            IpamType::PhpIpam => {
                if self.url.is_none() {
                    bail!("phpIPAM requires URL configuration");
                }
                if self.token.is_none() && (self.username.is_none() || self.password.is_none()) {
                    bail!("phpIPAM requires either token or username/password");
                }
            }
            IpamType::NetBox => {
                if self.url.is_none() {
                    bail!("NetBox requires URL configuration");
                }
                if self.token.is_none() {
                    bail!("NetBox requires token configuration");
                }
            }
        }

        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IpAllocationRequest {
    pub subnet: String,
    pub vmid: Option<u32>,
    pub hostname: Option<String>,
    pub mac: Option<String>,
    pub description: Option<String>,
    pub requested_ip: Option<IpAddr>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IpAllocation {
    pub ip: IpAddr,
    pub subnet: String,
    pub vmid: Option<u32>,
    pub hostname: Option<String>,
    pub mac: Option<String>,
    pub description: Option<String>,
    pub allocated_at: DateTime<Utc>,
}
