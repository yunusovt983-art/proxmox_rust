//! Error types for network operations

use thiserror::Error;

/// Main error type for network operations
#[derive(Debug, Error)]
pub enum NetworkError {
    #[error("Configuration error: {0}")]
    Configuration(#[from] ConfigError),

    #[error("Validation error: {0}")]
    Validation(#[from] ValidationError),

    #[error("System error: {0}")]
    System(#[from] SystemError),

    #[error("API error: {0}")]
    Api(#[from] ApiError),

    #[error("SDN error: {0}")]
    Sdn(#[from] SdnError),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
}

/// Configuration parsing and generation errors
#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("Parse error at line {line}: {message}")]
    Parse { line: usize, message: String },

    #[error("Invalid interface name: {name}")]
    InvalidInterfaceName { name: String },

    #[error("Duplicate interface: {name}")]
    DuplicateInterface { name: String },

    #[error("Missing required field: {field}")]
    MissingField { field: String },

    #[error("Invalid configuration value for {field}: {value}")]
    InvalidValue { field: String, value: String },
}

/// Validation errors
#[derive(Debug, Error)]
pub enum ValidationError {
    #[error("Interface {name} validation failed: {reason}")]
    Interface { name: String, reason: String },

    #[error("Network conflict: {message}")]
    NetworkConflict { message: String },

    #[error("ifupdown2 validation failed: {output}")]
    IfUpDown { output: String },

    #[error("Schema validation failed: {message}")]
    Schema { message: String },
}

/// System operation errors
#[derive(Debug, Error)]
pub enum SystemError {
    #[error("Command execution failed: {command}")]
    CommandFailed { command: String },

    #[error("File operation failed: {path}")]
    FileOperation { path: String },

    #[error("Network interface operation failed: {interface}")]
    InterfaceOperation { interface: String },

    #[error("Configuration write failed: {path}")]
    ConfigWrite {
        path: String,
        source: Box<dyn std::error::Error + Send + Sync>,
    },
}

/// API related errors
#[derive(Debug, Error)]
pub enum ApiError {
    #[error("Invalid request: {message}")]
    InvalidRequest { message: String },

    #[error("Bad request: {message}")]
    BadRequest { message: String },

    #[error("Resource not found: {resource}")]
    NotFound { resource: String },

    #[error("Resource conflict: {resource} - {message}")]
    Conflict { resource: String, message: String },

    #[error("Permission denied: {operation}")]
    PermissionDenied { operation: String },
}

/// SDN specific errors
#[derive(Debug, Error)]
pub enum SdnError {
    #[error("Zone error: {message}")]
    Zone { message: String },

    #[error("VNet error: {message}")]
    VNet { message: String },

    #[error("IPAM error: {message}")]
    Ipam { message: String },

    #[error("Controller error: {message}")]
    Controller { message: String },
}
