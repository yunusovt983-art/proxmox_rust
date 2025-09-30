//! Container Integration for pve-network
//!
//! This crate provides integration between pve-network and LXC containers,
//! supporting VNet binding, hotplug operations, and compatibility with pve-container.

pub mod error;
pub mod hooks;
pub mod hotplug;
pub mod pve_container_compat;
pub mod types;
pub mod vnet_binding;

pub use error::{ContainerError, Result};
pub use hooks::ContainerNetworkHooks;
pub use hotplug::ContainerNetworkHotplug;
pub use pve_container_compat::PveContainerCompat;
pub use types::*;
pub use vnet_binding::VNetBinding;

use std::sync::Arc;

use pve_event_bus::{EventBus, EventBusResult};

/// Container integration manager
pub struct ContainerIntegration {
    vnet_binding: VNetBinding,
    hotplug: ContainerNetworkHotplug,
    compat: PveContainerCompat,
    hooks: ContainerNetworkHooks,
}

impl ContainerIntegration {
    /// Create new container integration manager
    pub fn new() -> Self {
        Self {
            vnet_binding: VNetBinding::new(),
            hotplug: ContainerNetworkHotplug::new(),
            compat: PveContainerCompat::new(),
            hooks: ContainerNetworkHooks::new(),
        }
    }

    /// Get VNet binding manager
    pub fn vnet_binding(&self) -> &VNetBinding {
        &self.vnet_binding
    }

    /// Get hotplug manager
    pub fn hotplug(&self) -> &ContainerNetworkHotplug {
        &self.hotplug
    }

    /// Get pve-container compatibility layer
    pub fn compat(&self) -> &PveContainerCompat {
        &self.compat
    }

    /// Get network hooks
    pub fn hooks(&self) -> &ContainerNetworkHooks {
        &self.hooks
    }

    /// Attach hooks to the shared system event bus.
    pub async fn bind_event_bus(&self, bus: Arc<EventBus>) -> EventBusResult<()> {
        self.hooks.bind_event_bus(bus).await
    }
}

impl Default for ContainerIntegration {
    fn default() -> Self {
        Self::new()
    }
}
