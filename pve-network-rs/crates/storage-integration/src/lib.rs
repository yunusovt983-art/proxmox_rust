//! Storage system integration for pve-network
//!
//! This crate provides integration between pve-network and storage systems,
//! supporting network storage backends (NFS, CIFS, iSCSI) with proper
//! network isolation and VLAN tagging.

pub mod future_integration;
pub mod hooks;
pub mod path_resolution;
pub mod storage_network;
pub mod storage_plugins;
pub mod vlan_isolation;

#[cfg(test)]
mod tests;

pub use future_integration::*;
pub use hooks::{
    StorageEventLogger, StorageHook, StorageHooks, StorageStatusRefresher, StorageVlanReconciler,
};
pub use path_resolution::*;
pub use storage_network::*;
pub use storage_plugins::*;
pub use vlan_isolation::*;

use anyhow::Result;
use async_trait::async_trait;

pub use pve_shared_types::{
    QosSettings, StorageBackendType, StorageNetworkConfig, StorageNetworkInfo,
    StorageNetworkStatus, StorageVlanConfig, StorageVlanInfo,
};

#[async_trait]
pub trait StorageNetworkManager {
    async fn configure_storage_network(
        &self,
        storage_id: &str,
        config: &StorageNetworkConfig,
    ) -> Result<()>;

    async fn remove_storage_network(&self, storage_id: &str) -> Result<()>;

    async fn validate_storage_network(&self, config: &StorageNetworkConfig) -> Result<()>;

    async fn get_storage_network_status(&self, storage_id: &str) -> Result<StorageNetworkStatus>;

    async fn list_storage_networks(&self) -> Result<Vec<StorageNetworkInfo>>;
}

#[derive(Debug, thiserror::Error)]
pub enum StorageIntegrationError {
    #[error("Storage network configuration error: {0}")]
    Configuration(String),

    #[error("Storage backend not supported: {0}")]
    UnsupportedBackend(String),

    #[error("Network interface error: {0}")]
    NetworkInterface(String),

    #[error("VLAN configuration error: {0}")]
    VlanConfiguration(String),

    #[error("Storage plugin error: {0}")]
    StoragePlugin(String),

    #[error("Path resolution error: {0}")]
    PathResolution(String),

    #[error("System error: {0}")]
    System(#[from] anyhow::Error),
}

pub type StorageResult<T> = Result<T, StorageIntegrationError>;
