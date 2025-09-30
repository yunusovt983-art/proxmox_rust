use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::container::ContainerId;
use crate::migration::MigrationPhase;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum SystemEvent {
    /// Network configuration was applied through the transactional applier
    NetworkApplied { changes: Vec<ConfigChange> },
    /// Container finished its start sequence
    ContainerStarted { id: ContainerId },
    /// Storage VLAN was (re)created for a given storage backend identifier
    StorageVlanCreated { id: String },
    /// Migration workflow advanced to a new phase
    MigrationPhaseChanged { phase: MigrationPhase },
    /// Custom extensibility hook for prototype integrations
    Custom {
        name: String,
        data: HashMap<String, serde_json::Value>,
    },
}

/// Type of change detected when applying network configuration
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ChangeType {
    Create,
    Update,
    Delete,
    Modify,
}

/// Description of an applied configuration change
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ConfigChange {
    pub change_type: ChangeType,
    pub target: String,
    pub old_config: Option<serde_json::Value>,
    pub new_config: Option<serde_json::Value>,
    pub description: String,
}
