//! Proxmox VE Network Validation
//!
//! Configuration validation with syntax, semantic, and ifupdown2 integration

pub mod ifupdown;
pub mod semantic;
pub mod syntax;

pub use crate::syntax::SyntaxValidator;
pub use ifupdown::IfUpDownValidator;
pub use semantic::SemanticValidator;

use pve_network_core::error::ValidationError;
use pve_network_core::{Interface, NetworkConfiguration, NetworkError};

/// Comprehensive network configuration validator
pub struct NetworkValidator {
    syntax_validator: SyntaxValidator,
    semantic_validator: SemanticValidator,
    ifupdown_validator: IfUpDownValidator,
}

impl NetworkValidator {
    /// Create new network validator
    pub fn new() -> Self {
        Self {
            syntax_validator: SyntaxValidator::new(),
            semantic_validator: SemanticValidator::new(),
            ifupdown_validator: IfUpDownValidator::new(),
        }
    }

    /// Create validator with custom ifupdown2 path
    pub fn with_ifupdown_path<P: AsRef<std::path::Path>>(ifupdown_path: P) -> Self {
        Self {
            syntax_validator: SyntaxValidator::new(),
            semantic_validator: SemanticValidator::new(),
            ifupdown_validator: IfUpDownValidator::with_path(ifupdown_path),
        }
    }

    /// Validate configuration with all validators
    pub fn validate_configuration(
        &self,
        config: &NetworkConfiguration,
    ) -> Result<(), NetworkError> {
        let mut errors = Vec::new();

        // 1. Syntax validation
        if let Err(e) = self.syntax_validator.validate_configuration(config) {
            errors.push(format!("Syntax validation failed: {}", e));
        }

        // 2. Semantic validation (only if syntax is valid)
        if errors.is_empty() {
            if let Err(e) = self.semantic_validator.validate_configuration(config) {
                errors.push(format!("Semantic validation failed: {}", e));
            }
        }

        // 3. ifupdown2 validation (only if previous validations pass)
        if errors.is_empty() {
            if let Err(e) = self.ifupdown_validator.validate_configuration(config) {
                errors.push(format!("ifupdown2 validation failed: {}", e));
            }
        }

        if !errors.is_empty() {
            return Err(NetworkError::Validation(ValidationError::Schema {
                message: errors.join("; "),
            }));
        }

        log::info!("Network configuration validation passed");
        Ok(())
    }

    /// Async validate configuration with all validators
    pub async fn validate(&self, config: &NetworkConfiguration) -> Result<(), NetworkError> {
        // For now, just call the sync version
        // In a real implementation, this could perform async validation
        self.validate_configuration(config)
    }

    /// Validate configuration syntax only
    pub fn validate_syntax(&self, config: &NetworkConfiguration) -> Result<(), NetworkError> {
        self.syntax_validator.validate_configuration(config)
    }

    /// Validate configuration semantics only
    pub fn validate_semantics(&self, config: &NetworkConfiguration) -> Result<(), NetworkError> {
        self.semantic_validator.validate_configuration(config)
    }

    /// Validate configuration with ifupdown2 only
    pub fn validate_ifupdown(&self, config: &NetworkConfiguration) -> Result<(), NetworkError> {
        self.ifupdown_validator.validate_configuration(config)
    }

    /// Validate individual interface
    pub fn validate_interface(&self, interface: &Interface) -> Result<(), NetworkError> {
        // Syntax validation
        self.syntax_validator.validate_interface(interface)?;

        // ifupdown2 validation for individual interface
        self.ifupdown_validator.validate_interface(interface)?;

        Ok(())
    }
}

impl Default for NetworkValidator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn test_validator_creation() {
        let validator = NetworkValidator::new();
        assert!(true); // Just test that it can be created
    }

    #[test]
    fn test_validator_with_custom_path() {
        let validator = NetworkValidator::with_ifupdown_path("/custom/path/ifup".to_string());
        assert!(true); // Just test that it can be created
    }

    #[test]
    fn test_empty_configuration_validation() {
        let validator = NetworkValidator::new();
        let config = NetworkConfiguration::default();

        // Empty configuration should pass syntax and semantic validation
        assert!(validator.validate_syntax(&config).is_ok());
        assert!(validator.validate_semantics(&config).is_ok());
    }
}
