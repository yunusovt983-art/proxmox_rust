//! SDN VNet management

use anyhow::Result;
use serde::{Deserialize, Serialize};

pub use pve_shared_types::VNetConfig;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VNet {
    pub config: VNetConfig,
    pub status: VNetStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum VNetStatus {
    Active,
    Inactive,
    Error(String),
}

impl VNet {
    pub fn new(config: VNetConfig) -> Self {
        Self {
            config,
            status: VNetStatus::Inactive,
        }
    }

    pub fn name(&self) -> &str {
        &self.config.vnet
    }

    pub fn zone(&self) -> &str {
        &self.config.zone
    }
}
