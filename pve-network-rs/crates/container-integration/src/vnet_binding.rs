//! VNet binding management for containers

use async_trait::async_trait;
use log::{error, info, warn};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::error::{ContainerError, Result};
use crate::types::{
    ContainerId, ContainerNetworkEvent, ContainerNetworkEventType, ContainerNetworkInterface,
    ContainerNetworkInterfaceExt, VNetBinding as VNetBindingInfo,
};

/// VNet binding manager
pub struct VNetBinding {
    /// Active bindings: VNet -> Container -> Interface
    bindings: Arc<RwLock<HashMap<String, HashMap<ContainerId, Vec<String>>>>>,
    /// Reverse lookup: Container -> Interface -> VNet
    container_bindings: Arc<RwLock<HashMap<ContainerId, HashMap<String, String>>>>,
    /// Binding metadata
    binding_metadata: Arc<RwLock<HashMap<String, VNetBindingInfo>>>,
}

impl VNetBinding {
    /// Create new VNet binding manager
    pub fn new() -> Self {
        Self {
            bindings: Arc::new(RwLock::new(HashMap::new())),
            container_bindings: Arc::new(RwLock::new(HashMap::new())),
            binding_metadata: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Bind VNet to container interface
    pub async fn bind_vnet(
        &self,
        vnet: &str,
        container_id: ContainerId,
        interface: &ContainerNetworkInterface,
    ) -> Result<()> {
        info!(
            "Binding VNet '{}' to container {} interface '{}'",
            vnet, container_id, interface.name
        );

        // Validate interface configuration
        interface.validate()?;

        // Check if VNet exists (this would normally query the SDN configuration)
        if !self.vnet_exists(vnet).await? {
            return Err(ContainerError::VNetNotFound {
                vnet: vnet.to_string(),
            });
        }

        // Check for existing binding
        if self
            .is_interface_bound(container_id, &interface.name)
            .await?
        {
            warn!(
                "Interface '{}' on container {} is already bound",
                interface.name, container_id
            );
            return Ok(());
        }

        // Create binding
        let binding_key = format!("{}:{}:{}", vnet, container_id, interface.name);
        let binding_info =
            VNetBindingInfo::new(vnet.to_string(), container_id, interface.name.clone());

        // Update data structures
        {
            let mut bindings = self.bindings.write().await;
            bindings
                .entry(vnet.to_string())
                .or_insert_with(HashMap::new)
                .entry(container_id)
                .or_insert_with(Vec::new)
                .push(interface.name.clone());
        }

        {
            let mut container_bindings = self.container_bindings.write().await;
            container_bindings
                .entry(container_id)
                .or_insert_with(HashMap::new)
                .insert(interface.name.clone(), vnet.to_string());
        }

        {
            let mut metadata = self.binding_metadata.write().await;
            metadata.insert(binding_key, binding_info);
        }

        // Emit event
        self.emit_binding_event(
            container_id,
            ContainerNetworkEventType::VNetBound,
            serde_json::json!({
                "vnet": vnet,
                "interface": interface.name,
                "container_id": container_id
            }),
        )
        .await;

        info!(
            "Successfully bound VNet '{}' to container {} interface '{}'",
            vnet, container_id, interface.name
        );

        Ok(())
    }

    /// Unbind VNet from container interface
    pub async fn unbind_vnet(&self, container_id: ContainerId, interface_name: &str) -> Result<()> {
        info!(
            "Unbinding VNet from container {} interface '{}'",
            container_id, interface_name
        );

        // Get current binding
        let vnet = {
            let container_bindings = self.container_bindings.read().await;
            container_bindings
                .get(&container_id)
                .and_then(|interfaces| interfaces.get(interface_name))
                .cloned()
        };

        let vnet = match vnet {
            Some(vnet) => vnet,
            None => {
                warn!(
                    "No VNet binding found for container {} interface '{}'",
                    container_id, interface_name
                );
                return Ok(());
            }
        };

        // Remove binding
        let binding_key = format!("{}:{}:{}", vnet, container_id, interface_name);

        {
            let mut bindings = self.bindings.write().await;
            if let Some(container_map) = bindings.get_mut(&vnet) {
                if let Some(interfaces) = container_map.get_mut(&container_id) {
                    interfaces.retain(|iface| iface != interface_name);
                    if interfaces.is_empty() {
                        container_map.remove(&container_id);
                    }
                }
                if container_map.is_empty() {
                    bindings.remove(&vnet);
                }
            }
        }

        {
            let mut container_bindings = self.container_bindings.write().await;
            if let Some(interfaces) = container_bindings.get_mut(&container_id) {
                interfaces.remove(interface_name);
                if interfaces.is_empty() {
                    container_bindings.remove(&container_id);
                }
            }
        }

        {
            let mut metadata = self.binding_metadata.write().await;
            metadata.remove(&binding_key);
        }

        // Emit event
        self.emit_binding_event(
            container_id,
            ContainerNetworkEventType::VNetUnbound,
            serde_json::json!({
                "vnet": vnet,
                "interface": interface_name,
                "container_id": container_id
            }),
        )
        .await;

        info!(
            "Successfully unbound VNet '{}' from container {} interface '{}'",
            vnet, container_id, interface_name
        );

        Ok(())
    }

    /// Get all containers bound to a VNet
    pub async fn get_vnet_containers(&self, vnet: &str) -> Result<Vec<ContainerId>> {
        let bindings = self.bindings.read().await;
        Ok(bindings
            .get(vnet)
            .map(|container_map| container_map.keys().copied().collect())
            .unwrap_or_default())
    }

    /// Get all VNets bound to a container
    pub async fn get_container_vnets(&self, container_id: ContainerId) -> Result<Vec<String>> {
        let container_bindings = self.container_bindings.read().await;
        Ok(container_bindings
            .get(&container_id)
            .map(|interfaces| interfaces.values().cloned().collect())
            .unwrap_or_default())
    }

    /// Get VNet for a specific container interface
    pub async fn get_interface_vnet(
        &self,
        container_id: ContainerId,
        interface_name: &str,
    ) -> Result<Option<String>> {
        let container_bindings = self.container_bindings.read().await;
        Ok(container_bindings
            .get(&container_id)
            .and_then(|interfaces| interfaces.get(interface_name))
            .cloned())
    }

    /// Check if interface is bound to any VNet
    pub async fn is_interface_bound(
        &self,
        container_id: ContainerId,
        interface_name: &str,
    ) -> Result<bool> {
        let container_bindings = self.container_bindings.read().await;
        Ok(container_bindings
            .get(&container_id)
            .map(|interfaces| interfaces.contains_key(interface_name))
            .unwrap_or(false))
    }

    /// Get binding metadata
    pub async fn get_binding_metadata(
        &self,
        vnet: &str,
        container_id: ContainerId,
        interface_name: &str,
    ) -> Result<Option<VNetBindingInfo>> {
        let binding_key = format!("{}:{}:{}", vnet, container_id, interface_name);
        let metadata = self.binding_metadata.read().await;
        Ok(metadata.get(&binding_key).cloned())
    }

    /// List all bindings
    pub async fn list_bindings(&self) -> Result<Vec<VNetBindingInfo>> {
        let metadata = self.binding_metadata.read().await;
        Ok(metadata.values().cloned().collect())
    }

    /// Cleanup bindings for a container
    pub async fn cleanup_container_bindings(&self, container_id: ContainerId) -> Result<()> {
        info!("Cleaning up bindings for container {}", container_id);

        let interfaces_to_unbind: Vec<String> = {
            let container_bindings = self.container_bindings.read().await;
            container_bindings
                .get(&container_id)
                .map(|interfaces| interfaces.keys().cloned().collect())
                .unwrap_or_default()
        };

        for interface_name in interfaces_to_unbind {
            if let Err(e) = self.unbind_vnet(container_id, &interface_name).await {
                error!(
                    "Failed to unbind interface '{}' for container {}: {}",
                    interface_name, container_id, e
                );
            }
        }

        Ok(())
    }

    /// Check if VNet exists (placeholder - would integrate with SDN core)
    async fn vnet_exists(&self, _vnet: &str) -> Result<bool> {
        // TODO: Integrate with SDN core to check if VNet exists
        // For now, assume all VNets exist
        Ok(true)
    }

    /// Emit binding event (placeholder - would integrate with event system)
    async fn emit_binding_event(
        &self,
        container_id: ContainerId,
        event_type: ContainerNetworkEventType,
        data: serde_json::Value,
    ) {
        let event = ContainerNetworkEvent::new(container_id, event_type, data);
        // TODO: Emit event to event system
        info!("Event emitted: {:?}", event);
    }
}

impl Default for VNetBinding {
    fn default() -> Self {
        Self::new()
    }
}

/// VNet binding trait for extensibility
#[async_trait]
pub trait VNetBindingTrait {
    /// Bind VNet to container interface
    async fn bind_vnet(
        &self,
        vnet: &str,
        container_id: ContainerId,
        interface: &ContainerNetworkInterface,
    ) -> Result<()>;

    /// Unbind VNet from container interface
    async fn unbind_vnet(&self, container_id: ContainerId, interface_name: &str) -> Result<()>;

    /// Get containers bound to VNet
    async fn get_vnet_containers(&self, vnet: &str) -> Result<Vec<ContainerId>>;

    /// Get VNets bound to container
    async fn get_container_vnets(&self, container_id: ContainerId) -> Result<Vec<String>>;
}

#[async_trait]
impl VNetBindingTrait for VNetBinding {
    async fn bind_vnet(
        &self,
        vnet: &str,
        container_id: ContainerId,
        interface: &ContainerNetworkInterface,
    ) -> Result<()> {
        self.bind_vnet(vnet, container_id, interface).await
    }

    async fn unbind_vnet(&self, container_id: ContainerId, interface_name: &str) -> Result<()> {
        self.unbind_vnet(container_id, interface_name).await
    }

    async fn get_vnet_containers(&self, vnet: &str) -> Result<Vec<ContainerId>> {
        self.get_vnet_containers(vnet).await
    }

    async fn get_container_vnets(&self, container_id: ContainerId) -> Result<Vec<String>> {
        self.get_container_vnets(container_id).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_vnet_binding() {
        let binding_manager = VNetBinding::new();
        let container_id = 100;
        let vnet = "test-vnet";

        let mut interface = ContainerNetworkInterface::new("net0".to_string());
        interface.vnet = Some(vnet.to_string());

        // Test binding
        assert!(binding_manager
            .bind_vnet(vnet, container_id, &interface)
            .await
            .is_ok());

        // Test checking binding
        assert!(binding_manager
            .is_interface_bound(container_id, "net0")
            .await
            .unwrap());

        // Test getting VNet for interface
        let bound_vnet = binding_manager
            .get_interface_vnet(container_id, "net0")
            .await
            .unwrap();
        assert_eq!(bound_vnet, Some(vnet.to_string()));

        // Test unbinding
        assert!(binding_manager
            .unbind_vnet(container_id, "net0")
            .await
            .is_ok());
        assert!(!binding_manager
            .is_interface_bound(container_id, "net0")
            .await
            .unwrap());
    }

    #[tokio::test]
    async fn test_container_cleanup() {
        let binding_manager = VNetBinding::new();
        let container_id = 101;

        let mut interface1 = ContainerNetworkInterface::new("net0".to_string());
        interface1.vnet = Some("vnet1".to_string());

        let mut interface2 = ContainerNetworkInterface::new("net1".to_string());
        interface2.vnet = Some("vnet2".to_string());

        // Bind multiple interfaces
        assert!(binding_manager
            .bind_vnet("vnet1", container_id, &interface1)
            .await
            .is_ok());
        assert!(binding_manager
            .bind_vnet("vnet2", container_id, &interface2)
            .await
            .is_ok());

        // Verify bindings
        let vnets = binding_manager
            .get_container_vnets(container_id)
            .await
            .unwrap();
        assert_eq!(vnets.len(), 2);

        // Cleanup container
        assert!(binding_manager
            .cleanup_container_bindings(container_id)
            .await
            .is_ok());

        // Verify cleanup
        let vnets_after = binding_manager
            .get_container_vnets(container_id)
            .await
            .unwrap();
        assert!(vnets_after.is_empty());
    }
}
