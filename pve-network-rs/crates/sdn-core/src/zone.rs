//! SDN Zone abstractions

use anyhow::Result;
use async_trait::async_trait;

pub use pve_shared_types::{ZoneConfig, ZoneType};

#[async_trait]
pub trait Zone: Send + Sync {
    fn zone_type(&self) -> ZoneType;
    fn name(&self) -> &str;
    async fn validate_config(&self, config: &ZoneConfig) -> Result<()>;
    async fn apply_config(&self, config: &ZoneConfig) -> Result<()>;
    async fn generate_config(
        &self,
        config: &ZoneConfig,
    ) -> Result<std::collections::HashMap<String, String>>;
}
