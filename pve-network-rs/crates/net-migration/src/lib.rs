//! Migration system for pve-network Perl to Rust transition
//!
//! This module provides middleware and configuration for gradual migration
//! from Perl to Rust implementation with fallback capabilities.

pub mod config;
pub mod fallback;
pub mod hooks;
pub mod middleware;
pub mod perl_client;

pub use config::{EndpointConfig, MigrationConfig, MigrationPhase};
pub use fallback::FallbackHandler;
pub use hooks::{MigrationEventLogger, MigrationHook, MigrationHooks};
pub use middleware::MigrationMiddleware;
pub use perl_client::PerlApiClient;

use thiserror::Error;

/// Migration-specific errors
#[derive(Debug, Error)]
pub enum MigrationError {
    #[error("Configuration error: {0}")]
    Config(#[from] config::ConfigError),

    #[error("Migration hook configuration error: {0}")]
    Configuration(String),

    #[error("Perl API error: {0}")]
    PerlApi(#[from] perl_client::PerlApiError),

    #[error("Fallback error: {0}")]
    Fallback(String),

    #[error("Migration phase error: {phase:?} - {message}")]
    Phase {
        phase: MigrationPhase,
        message: String,
    },

    #[error("Endpoint not available in current migration phase")]
    EndpointNotAvailable,
}

pub type Result<T> = std::result::Result<T, MigrationError>;
