//! Container integration types

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub use pve_shared_types::{
    ContainerId, ContainerNetworkConfig, ContainerNetworkEvent, ContainerNetworkEventType,
    ContainerNetworkInterface, ContainerNetworkState, ContainerNetworkStatus, VNetBinding,
};

/// Extension helpers for container network interfaces
pub trait ContainerNetworkInterfaceExt {
    fn validate(&self) -> crate::Result<()>;
}

impl ContainerNetworkInterfaceExt for ContainerNetworkInterface {
    fn validate(&self) -> crate::Result<()> {
        if self.name.is_empty() {
            return Err(crate::ContainerError::InvalidConfiguration {
                field: "name".to_string(),
                reason: "Interface name cannot be empty".to_string(),
            });
        }

        if self.vnet.is_some() && self.bridge.is_some() {
            return Err(crate::ContainerError::InvalidConfiguration {
                field: "network_backend".to_string(),
                reason: "Cannot specify both vnet and bridge".to_string(),
            });
        }

        if self.vnet.is_none() && self.bridge.is_none() {
            return Err(crate::ContainerError::InvalidConfiguration {
                field: "network_backend".to_string(),
                reason: "Must specify either vnet or bridge".to_string(),
            });
        }

        if let Some(tag) = self.tag {
            if tag == 0 || tag > 4094 {
                return Err(crate::ContainerError::InvalidConfiguration {
                    field: "tag".to_string(),
                    reason: "VLAN tag must be between 1 and 4094".to_string(),
                });
            }
        }

        Ok(())
    }
}

/// Extension helpers for container network configurations
pub trait ContainerNetworkConfigExt {
    fn add_interface(&mut self, interface: ContainerNetworkInterface) -> crate::Result<()>;
    fn remove_interface(&mut self, name: &str) -> Option<ContainerNetworkInterface>;
    fn get_interface(&self, name: &str) -> Option<&ContainerNetworkInterface>;
    fn vnet_interfaces(&self) -> Vec<&ContainerNetworkInterface>;
    fn bridge_interfaces(&self) -> Vec<&ContainerNetworkInterface>;
    fn validate(&self) -> crate::Result<()>;
}

impl ContainerNetworkConfigExt for ContainerNetworkConfig {
    fn add_interface(&mut self, interface: ContainerNetworkInterface) -> crate::Result<()> {
        interface.validate()?;
        self.interfaces.insert(interface.name.clone(), interface);
        Ok(())
    }

    fn remove_interface(&mut self, name: &str) -> Option<ContainerNetworkInterface> {
        self.interfaces.remove(name)
    }

    fn get_interface(&self, name: &str) -> Option<&ContainerNetworkInterface> {
        self.interfaces.get(name)
    }

    fn vnet_interfaces(&self) -> Vec<&ContainerNetworkInterface> {
        self.interfaces
            .values()
            .filter(|iface| iface.uses_sdn())
            .collect()
    }

    fn bridge_interfaces(&self) -> Vec<&ContainerNetworkInterface> {
        self.interfaces
            .values()
            .filter(|iface| iface.uses_bridge())
            .collect()
    }

    fn validate(&self) -> crate::Result<()> {
        for interface in self.interfaces.values() {
            interface.validate()?;
        }
        Ok(())
    }
}

/// Container network operation types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ContainerNetworkOperation {
    Create {
        container_id: ContainerId,
        config: ContainerNetworkConfig,
    },
    Update {
        container_id: ContainerId,
        config: ContainerNetworkConfig,
    },
    Delete {
        container_id: ContainerId,
    },
    HotplugAdd {
        container_id: ContainerId,
        interface: ContainerNetworkInterface,
    },
    HotplugRemove {
        container_id: ContainerId,
        interface_name: String,
    },
    Migrate {
        container_id: ContainerId,
        source_node: String,
        target_node: String,
        config: ContainerNetworkConfig,
    },
}
