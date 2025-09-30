//! Proxmox VE Network API
//!
//! REST API endpoints for network management

pub mod container;
pub mod context;
pub mod migration;
pub mod network;
pub mod sdn;
pub mod storage;
pub mod tasks;

#[cfg(test)]
mod tests;

pub use container::ContainerNetworkAPI;
pub use migration::NetApiRustHandler;
pub use network::NetworkAPI;
pub use sdn::SDNAPI;
pub use storage::StorageNetworkAPI;
