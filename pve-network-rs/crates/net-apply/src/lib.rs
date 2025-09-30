//! Proxmox VE Network Apply
//!
//! Transactional configuration application with rollback support

pub mod ifupdown;
pub mod rollback;
pub mod transaction;

#[cfg(test)]
mod tests;

pub use ifupdown::{IfUpDownIntegration, IfUpDownResult, InterfaceChangeType, InterfaceState};
pub use pve_shared_types::{ChangeType, ConfigChange};
pub use rollback::{BackupFile, RollbackManager, RollbackPoint, RollbackStats};
pub use transaction::{ApplyResult, NetworkApplier, Transaction, TransactionState};
