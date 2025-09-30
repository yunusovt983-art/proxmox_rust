//! Migration configuration management
//!
//! Handles configuration for phased migration from Perl to Rust

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::Path;
use thiserror::Error;

/// Migration configuration errors
#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("Failed to load configuration: {0}")]
    Load(#[from] config::ConfigError),

    #[error("Invalid migration phase: {0}")]
    InvalidPhase(String),

    #[error("Endpoint configuration error: {0}")]
    EndpointConfig(String),
}

/// Migration phases for gradual rollout
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MigrationPhase {
    /// All requests handled by Perl (default/fallback)
    PerlOnly,
    /// Read-only operations handled by Rust, writes by Perl
    RustReadOnly,
    /// Basic write operations handled by Rust
    RustBasicWrite,
    /// Advanced network functions handled by Rust
    RustAdvanced,
    /// SDN operations handled by Rust
    RustSdn,
    /// Full Rust implementation
    RustFull,
}

impl Default for MigrationPhase {
    fn default() -> Self {
        MigrationPhase::PerlOnly
    }
}

impl std::str::FromStr for MigrationPhase {
    type Err = ConfigError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "perl-only" | "perl_only" => Ok(MigrationPhase::PerlOnly),
            "rust-read-only" | "rust_read_only" => Ok(MigrationPhase::RustReadOnly),
            "rust-basic-write" | "rust_basic_write" => Ok(MigrationPhase::RustBasicWrite),
            "rust-advanced" | "rust_advanced" => Ok(MigrationPhase::RustAdvanced),
            "rust-sdn" | "rust_sdn" => Ok(MigrationPhase::RustSdn),
            "rust-full" | "rust_full" => Ok(MigrationPhase::RustFull),
            _ => Err(ConfigError::InvalidPhase(s.to_string())),
        }
    }
}

/// Configuration for individual endpoints
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EndpointConfig {
    /// Whether this endpoint should use Rust implementation
    pub use_rust: bool,
    /// Whether to fallback to Perl on Rust errors
    pub fallback_on_error: bool,
    /// Timeout for Rust operations before fallback (seconds)
    pub rust_timeout: Option<u64>,
    /// Specific HTTP methods to handle with Rust (if empty, all methods)
    pub rust_methods: HashSet<String>,
}

impl Default for EndpointConfig {
    fn default() -> Self {
        Self {
            use_rust: false,
            fallback_on_error: true,
            rust_timeout: Some(30),
            rust_methods: HashSet::new(),
        }
    }
}

/// Main migration configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MigrationConfig {
    /// Current migration phase
    pub phase: MigrationPhase,

    /// Global fallback settings
    pub fallback_enabled: bool,
    pub fallback_timeout: u64,

    /// Perl API configuration
    pub perl_api_base_url: String,
    pub perl_api_timeout: u64,

    /// Per-endpoint configuration overrides
    pub endpoints: HashMap<String, EndpointConfig>,

    /// Feature flags for specific functionality
    pub features: HashMap<String, bool>,

    /// Monitoring and logging settings
    pub log_migration_decisions: bool,
    pub metrics_enabled: bool,
}

impl Default for MigrationConfig {
    fn default() -> Self {
        let mut endpoints = HashMap::new();

        // Configure endpoints with both read and write methods
        let endpoint_configs = vec![
            // Network endpoints - support both read and write
            ("/api2/json/nodes/{node}/network", vec!["GET", "POST"], 30),
            (
                "/api2/json/nodes/{node}/network/{iface}",
                vec!["GET", "PUT", "DELETE"],
                30,
            ),
            ("/api2/json/nodes/{node}/network/reload", vec!["POST"], 60),
            // SDN read-only endpoints
            ("/api2/json/sdn/zones", vec!["GET"], 30),
            ("/api2/json/sdn/vnets", vec!["GET"], 30),
            ("/api2/json/sdn/subnets", vec!["GET"], 30),
            ("/api2/json/sdn/controllers", vec!["GET"], 30),
            ("/api2/json/sdn/ipams", vec!["GET"], 30),
            // SDN write endpoints
            (
                "/api2/json/sdn/zones/{zone}",
                vec!["POST", "PUT", "DELETE"],
                60,
            ),
            (
                "/api2/json/sdn/vnets/{vnet}",
                vec!["POST", "PUT", "DELETE"],
                60,
            ),
        ];

        for (endpoint, methods, timeout) in endpoint_configs {
            endpoints.insert(
                endpoint.to_string(),
                EndpointConfig {
                    use_rust: false, // Start with Perl, enable per phase
                    fallback_on_error: true,
                    rust_timeout: Some(timeout),
                    rust_methods: methods.into_iter().map(|s| s.to_string()).collect(),
                },
            );
        }

        Self {
            phase: MigrationPhase::PerlOnly,
            fallback_enabled: true,
            fallback_timeout: 30,
            perl_api_base_url: "http://localhost:8006".to_string(),
            perl_api_timeout: 60,
            endpoints,
            features: HashMap::new(),
            log_migration_decisions: true,
            metrics_enabled: true,
        }
    }
}

impl MigrationConfig {
    /// Load configuration from file
    pub fn load_from_file<P: AsRef<Path>>(path: P) -> Result<Self, ConfigError> {
        let settings = config::Config::builder()
            .add_source(config::File::with_name(path.as_ref().to_str().unwrap()))
            .add_source(config::Environment::with_prefix("PVE_NETWORK_MIGRATION"))
            .build()?;

        let config: MigrationConfig = settings.try_deserialize()?;
        Ok(config)
    }

    /// Load configuration with defaults and environment overrides
    pub fn load_with_defaults() -> Result<Self, ConfigError> {
        let mut config = Self::default();

        // Try to load from standard locations
        let config_paths = vec![
            "/etc/pve/network-migration.conf",
            "/etc/proxmox/network-migration.conf",
            "./network-migration.conf",
        ];

        for path in config_paths {
            if std::path::Path::new(path).exists() {
                match Self::load_from_file(path) {
                    Ok(loaded_config) => {
                        config = loaded_config;
                        break;
                    }
                    Err(e) => {
                        log::warn!("Failed to load config from {}: {}", path, e);
                    }
                }
            }
        }

        // Apply environment overrides
        if let Ok(phase_str) = std::env::var("PVE_NETWORK_MIGRATION_PHASE") {
            if let Ok(phase) = phase_str.parse() {
                config.phase = phase;
            }
        }

        if let Ok(fallback_str) = std::env::var("PVE_NETWORK_MIGRATION_FALLBACK") {
            config.fallback_enabled = fallback_str.parse().unwrap_or(true);
        }

        // Update endpoint configurations based on phase
        config.update_endpoints_for_phase();

        Ok(config)
    }

    /// Update endpoint configurations based on current migration phase
    pub fn update_endpoints_for_phase(&mut self) {
        match self.phase {
            MigrationPhase::PerlOnly => {
                // All endpoints use Perl
                for endpoint_config in self.endpoints.values_mut() {
                    endpoint_config.use_rust = false;
                }
            }
            MigrationPhase::RustReadOnly => {
                // Only GET operations use Rust
                for endpoint_config in self.endpoints.values_mut() {
                    endpoint_config.use_rust = endpoint_config.rust_methods.contains("GET");
                }
            }
            MigrationPhase::RustBasicWrite => {
                // Basic network operations use Rust
                let basic_endpoints = vec![
                    "/api2/json/nodes/{node}/network",
                    "/api2/json/nodes/{node}/network/{iface}",
                ];

                for (endpoint, config) in &mut self.endpoints {
                    config.use_rust = basic_endpoints.iter().any(|&e| endpoint.contains(e));
                }
            }
            MigrationPhase::RustAdvanced => {
                // Advanced network functions use Rust
                for (endpoint, config) in &mut self.endpoints {
                    if endpoint.contains("/network") {
                        config.use_rust = true;
                    }
                }
            }
            MigrationPhase::RustSdn => {
                // Network and SDN operations use Rust
                for (endpoint, config) in &mut self.endpoints {
                    if endpoint.contains("/network") || endpoint.contains("/sdn") {
                        config.use_rust = true;
                    }
                }
            }
            MigrationPhase::RustFull => {
                // All endpoints use Rust
                for endpoint_config in self.endpoints.values_mut() {
                    endpoint_config.use_rust = true;
                }
            }
        }
    }

    /// Check if an endpoint should use Rust implementation
    pub fn should_use_rust(&self, endpoint: &str, method: &str) -> bool {
        if let Some(config) = self.endpoints.get(endpoint) {
            config.use_rust
                && (config.rust_methods.is_empty() || config.rust_methods.contains(method))
        } else {
            // Default behavior based on phase
            match self.phase {
                MigrationPhase::PerlOnly => false,
                MigrationPhase::RustReadOnly => method == "GET",
                MigrationPhase::RustFull => true,
                _ => false, // Conservative default
            }
        }
    }

    /// Check if fallback is enabled for an endpoint
    pub fn should_fallback(&self, endpoint: &str) -> bool {
        if let Some(config) = self.endpoints.get(endpoint) {
            config.fallback_on_error && self.fallback_enabled
        } else {
            self.fallback_enabled
        }
    }

    /// Get timeout for Rust operations on an endpoint
    pub fn get_rust_timeout(&self, endpoint: &str) -> u64 {
        if let Some(config) = self.endpoints.get(endpoint) {
            config.rust_timeout.unwrap_or(self.fallback_timeout)
        } else {
            self.fallback_timeout
        }
    }

    /// Check if a feature is enabled
    pub fn is_feature_enabled(&self, feature: &str) -> bool {
        self.features.get(feature).copied().unwrap_or(false)
    }

    /// Enable or disable a feature
    pub fn set_feature(&mut self, feature: &str, enabled: bool) {
        self.features.insert(feature.to_string(), enabled);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_migration_phase_parsing() {
        assert_eq!(
            "perl-only".parse::<MigrationPhase>().unwrap(),
            MigrationPhase::PerlOnly
        );
        assert_eq!(
            "rust-read-only".parse::<MigrationPhase>().unwrap(),
            MigrationPhase::RustReadOnly
        );
        assert_eq!(
            "rust_full".parse::<MigrationPhase>().unwrap(),
            MigrationPhase::RustFull
        );
    }

    #[test]
    fn test_default_config() {
        let config = MigrationConfig::default();
        assert_eq!(config.phase, MigrationPhase::PerlOnly);
        assert!(config.fallback_enabled);
        assert!(!config.endpoints.is_empty());
    }

    #[test]
    fn test_phase_updates() {
        let mut config = MigrationConfig::default();

        // Test read-only phase
        config.phase = MigrationPhase::RustReadOnly;
        config.update_endpoints_for_phase();

        // Should enable Rust for GET-only endpoints
        let network_endpoint = config
            .endpoints
            .get("/api2/json/nodes/{node}/network")
            .unwrap();
        assert!(network_endpoint.use_rust);
    }

    #[test]
    fn test_should_use_rust() {
        let mut config = MigrationConfig::default();
        config.phase = MigrationPhase::RustReadOnly;
        config.update_endpoints_for_phase();

        assert!(config.should_use_rust("/api2/json/nodes/test/network", "GET"));
        assert!(!config.should_use_rust("/api2/json/nodes/test/network", "POST"));
    }
}
