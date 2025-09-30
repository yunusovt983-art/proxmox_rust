//! Proxmox VE Network Configuration
//!
//! Configuration parsing and generation with cluster synchronization

pub mod interfaces;
pub mod network_config;
pub mod pmxcfs;
pub mod sdn_config;

#[cfg(test)]
mod tests;

pub use interfaces::InterfacesParser;
pub use network_config::{InterfaceConfig, NetworkConfigManager, NetworkConfiguration};
pub use pmxcfs::{ClusterLock, PmxcfsConfig};
pub use sdn_config::{SdnConfigManager, SdnConfiguration, SubnetConfig, VNetConfig, ZoneConfig};
