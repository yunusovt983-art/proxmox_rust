//! Proxmox VE Network Core
//!
//! Core types and business logic for network management

pub mod bond;
pub mod bridge;
pub mod error;
pub mod interface;
pub mod types;
pub mod vlan;

pub use error::NetworkError;
pub use types::*;

/// Result type for network operations
pub type Result<T> = std::result::Result<T, NetworkError>;
