//! SDN Controller abstractions

use anyhow::Result;
use async_trait::async_trait;
use std::collections::HashMap;

use crate::{VNet, Zone};

pub use pve_shared_types::{ControllerConfig, ControllerStatus, ControllerType};

#[async_trait]
pub trait Controller: Send + Sync {
    fn controller_type(&self) -> ControllerType;
    fn name(&self) -> &str;
    async fn validate_configuration(&self, config: &ControllerConfig) -> Result<()>;
    async fn apply_configuration(&self, zones: &[Box<dyn Zone>], vnets: &[VNet]) -> Result<()>;
    async fn generate_config(&self, config: &ControllerConfig) -> Result<HashMap<String, String>>;
    async fn start(&self) -> Result<()>;
    async fn stop(&self) -> Result<()>;
    async fn status(&self) -> Result<ControllerStatus>;
    async fn reload(&self) -> Result<()>;
}
