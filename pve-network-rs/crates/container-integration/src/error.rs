//! Container integration error types

use thiserror::Error;

/// Container integration error types
#[derive(Debug, Error)]
pub enum ContainerError {
    /// Configuration error
    #[error("Configuration error in field '{field}': {reason}")]
    InvalidConfiguration { field: String, reason: String },

    /// VNet not found
    #[error("VNet '{vnet}' not found")]
    VNetNotFound { vnet: String },

    /// Container not found
    #[error("Container {container_id} not found")]
    ContainerNotFound { container_id: u32 },

    /// Interface not found
    #[error("Interface '{interface}' not found on container {container_id}")]
    InterfaceNotFound {
        container_id: u32,
        interface: String,
    },

    /// VNet binding error
    #[error("VNet binding error: {message}")]
    VNetBinding { message: String },

    /// Hotplug operation error
    #[error("Hotplug operation failed: {message}")]
    HotplugFailed { message: String },

    /// pve-container compatibility error
    #[error("pve-container compatibility error: {message}")]
    PveContainerCompat { message: String },

    /// Network operation error
    #[error("Network operation failed: {message}")]
    NetworkOperation { message: String },

    /// System error
    #[error("System error: {source}")]
    System {
        #[from]
        source: std::io::Error,
    },

    /// Serialization error
    #[error("Serialization error: {source}")]
    Serialization {
        #[from]
        source: serde_json::Error,
    },

    /// Network core error
    #[error("Network core error: {source}")]
    NetworkCore {
        #[from]
        source: pve_network_core::NetworkError,
    },

    /// SDN core error
    #[error("SDN error: {source}")]
    Sdn {
        #[from]
        source: anyhow::Error,
    },
}

/// Result type for container operations
pub type Result<T> = std::result::Result<T, ContainerError>;
