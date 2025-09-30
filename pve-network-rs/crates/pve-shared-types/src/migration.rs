use std::collections::HashMap;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum MigrationPhase {
    PerlOnly,
    RustReadOnly,
    RustBasicWrite,
    RustAdvanced,
    RustSdn,
    RustFull,
}

impl Default for MigrationPhase {
    fn default() -> Self {
        MigrationPhase::PerlOnly
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EndpointConfig {
    #[serde(default)]
    pub use_rust: bool,
    #[serde(default = "default_true")]
    pub fallback_on_error: bool,
    pub rust_timeout: Option<u64>,
    #[serde(default)]
    pub rust_methods: Vec<String>,
}

fn default_true() -> bool {
    true
}

impl Default for EndpointConfig {
    fn default() -> Self {
        Self {
            use_rust: false,
            fallback_on_error: true,
            rust_timeout: Some(30),
            rust_methods: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MigrationConfig {
    #[serde(default)]
    pub phase: MigrationPhase,
    #[serde(default = "default_true")]
    pub fallback_enabled: bool,
    #[serde(default = "default_timeout")]
    pub fallback_timeout: u64,
    pub perl_api_base_url: Option<String>,
    pub perl_api_timeout: Option<u64>,
    #[serde(default)]
    pub endpoints: HashMap<String, EndpointConfig>,
    #[serde(default)]
    pub features: HashMap<String, bool>,
    #[serde(default)]
    pub log_migration_decisions: bool,
}

fn default_timeout() -> u64 {
    30
}

impl MigrationConfig {
    pub fn should_use_rust(&self, endpoint: &str, method: &str) -> bool {
        if let Some(config) = self.endpoints.get(endpoint) {
            if !config.use_rust {
                return false;
            }
            if config.rust_methods.is_empty() {
                return true;
            }
            config
                .rust_methods
                .iter()
                .any(|m| m.eq_ignore_ascii_case(method))
        } else {
            matches!(self.phase, MigrationPhase::RustFull)
        }
    }

    pub fn should_fallback(&self, endpoint: &str) -> bool {
        if let Some(config) = self.endpoints.get(endpoint) {
            config.fallback_on_error && self.fallback_enabled
        } else {
            self.fallback_enabled
        }
    }

    pub fn feature_enabled(&self, feature: &str) -> bool {
        self.features.get(feature).copied().unwrap_or(false)
    }
}
