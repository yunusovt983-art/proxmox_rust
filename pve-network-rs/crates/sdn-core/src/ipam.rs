//! IPAM abstractions

use anyhow::Result;
use async_trait::async_trait;
use std::net::IpAddr;

use crate::subnet::Subnet;

pub use pve_shared_types::{IpAllocation, IpAllocationRequest, IpamConfig, IpamType};

#[async_trait]
pub trait IpamPlugin: Send + Sync {
    fn plugin_type(&self) -> IpamType;
    fn name(&self) -> &str;
    async fn validate_config(&self, config: &IpamConfig) -> Result<()>;
    async fn allocate_ip(&self, request: &IpAllocationRequest) -> Result<IpAllocation>;
    async fn release_ip(&self, subnet: &str, ip: &IpAddr) -> Result<()>;
    async fn update_ip(&self, subnet: &str, ip: &IpAddr, allocation: &IpAllocation) -> Result<()>;
    async fn get_ip(&self, subnet: &str, ip: &IpAddr) -> Result<Option<IpAllocation>>;
    async fn list_subnet_ips(&self, subnet: &str) -> Result<Vec<IpAllocation>>;
    async fn validate_subnet(&self, subnet: &Subnet) -> Result<()>;
    async fn add_subnet(&self, subnet: &Subnet) -> Result<()>;
    async fn remove_subnet(&self, subnet_name: &str) -> Result<()>;
    async fn get_next_free_ip(&self, subnet: &str) -> Result<Option<IpAddr>>;
    async fn is_ip_available(&self, subnet: &str, ip: &IpAddr) -> Result<bool>;
}

#[derive(Debug, thiserror::Error)]
pub enum IpamError {
    #[error("IP address {ip} already allocated in subnet {subnet}")]
    IpAlreadyAllocated { ip: IpAddr, subnet: String },

    #[error("IP address {ip} not found in subnet {subnet}")]
    IpNotFound { ip: IpAddr, subnet: String },

    #[error("No free IP addresses available in subnet {subnet}")]
    NoFreeIps { subnet: String },

    #[error("Subnet {subnet} not found in IPAM")]
    SubnetNotFound { subnet: String },

    #[error("IPAM configuration error: {message}")]
    Configuration { message: String },

    #[error("IPAM API error: {message}")]
    Api { message: String },

    #[error("Network error: {source}")]
    Network {
        #[from]
        source: reqwest::Error,
    },

    #[error("Serialization error: {source}")]
    Serialization {
        #[from]
        source: serde_json::Error,
    },
}
