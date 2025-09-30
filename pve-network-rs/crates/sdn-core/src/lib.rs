//! Proxmox VE SDN Core
//!
//! SDN core abstractions

pub mod config;
pub mod controller;
pub mod ipam;
pub mod ipam_manager;
pub mod subnet;
pub mod vnet;
pub mod zone;

#[cfg(test)]
mod tests;

pub use config::SdnConfiguration;
pub use controller::{Controller, ControllerType};
pub use ipam::{IpAllocation, IpAllocationRequest, IpamConfig, IpamError, IpamPlugin, IpamType};
pub use ipam_manager::IpamManager;
pub use subnet::{DhcpConfig, Subnet, SubnetConfig, SubnetStatus, SubnetType};
pub use vnet::{VNet, VNetConfig, VNetStatus};
pub use zone::{Zone, ZoneConfig, ZoneType};
