use std::collections::HashMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub type ContainerId = u32;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContainerNetworkInterface {
    pub name: String,
    pub vnet: Option<String>,
    pub bridge: Option<String>,
    pub hwaddr: Option<String>,
    pub ip: Option<String>,
    pub ip6: Option<String>,
    pub gw: Option<String>,
    pub gw6: Option<String>,
    pub tag: Option<u16>,
    pub trunks: Option<String>,
    pub firewall: Option<bool>,
    pub link_down: Option<bool>,
    pub mtu: Option<u16>,
    pub rate: Option<f64>,
    #[serde(default)]
    pub options: HashMap<String, String>,
}

impl ContainerNetworkInterface {
    pub fn new(name: String) -> Self {
        Self {
            name,
            vnet: None,
            bridge: None,
            hwaddr: None,
            ip: None,
            ip6: None,
            gw: None,
            gw6: None,
            tag: None,
            trunks: None,
            firewall: None,
            link_down: None,
            mtu: None,
            rate: None,
            options: HashMap::new(),
        }
    }

    pub fn uses_sdn(&self) -> bool {
        self.vnet.is_some()
    }

    pub fn uses_bridge(&self) -> bool {
        self.bridge.is_some()
    }

    pub fn network_backend(&self) -> Option<&str> {
        self.vnet.as_deref().or(self.bridge.as_deref())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContainerNetworkConfig {
    pub container_id: ContainerId,
    #[serde(default)]
    pub interfaces: HashMap<String, ContainerNetworkInterface>,
    pub hostname: Option<String>,
    #[serde(default)]
    pub nameservers: Vec<String>,
    #[serde(default)]
    pub search_domains: Vec<String>,
    #[serde(default)]
    pub options: HashMap<String, serde_json::Value>,
}

impl ContainerNetworkConfig {
    pub fn new(container_id: ContainerId) -> Self {
        Self {
            container_id,
            interfaces: HashMap::new(),
            hostname: None,
            nameservers: Vec::new(),
            search_domains: Vec::new(),
            options: HashMap::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum ContainerNetworkEventType {
    InterfaceAdded,
    InterfaceRemoved,
    InterfaceUpdated,
    VNetBound,
    VNetUnbound,
    ContainerStarted,
    ContainerStopped,
    ContainerMigrated,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContainerNetworkEvent {
    pub id: Uuid,
    pub timestamp: DateTime<Utc>,
    pub container_id: ContainerId,
    pub event_type: ContainerNetworkEventType,
    pub data: serde_json::Value,
}

impl ContainerNetworkEvent {
    pub fn new(
        container_id: ContainerId,
        event_type: ContainerNetworkEventType,
        data: serde_json::Value,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            timestamp: Utc::now(),
            container_id,
            event_type,
            data,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ContainerNetworkStatus {
    Configuring,
    Active,
    Inactive,
    Error(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContainerNetworkState {
    pub container_id: ContainerId,
    pub config: ContainerNetworkConfig,
    pub status: ContainerNetworkStatus,
    #[serde(default)]
    pub vnet_bindings: Vec<VNetBinding>,
    pub last_updated: DateTime<Utc>,
}

impl ContainerNetworkState {
    pub fn new(container_id: ContainerId, config: ContainerNetworkConfig) -> Self {
        Self {
            container_id,
            config,
            status: ContainerNetworkStatus::Inactive,
            vnet_bindings: Vec::new(),
            last_updated: Utc::now(),
        }
    }

    pub fn set_status(&mut self, status: ContainerNetworkStatus) {
        self.status = status;
        self.last_updated = Utc::now();
    }

    pub fn add_vnet_binding(&mut self, binding: VNetBinding) {
        self.vnet_bindings.push(binding);
        self.last_updated = Utc::now();
    }

    pub fn remove_vnet_binding(&mut self, vnet: &str, interface_name: &str) {
        self.vnet_bindings
            .retain(|binding| !(binding.vnet == vnet && binding.interface_name == interface_name));
        self.last_updated = Utc::now();
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VNetBinding {
    pub vnet: String,
    pub container_id: ContainerId,
    pub interface_name: String,
    pub bound_at: DateTime<Utc>,
    #[serde(default)]
    pub metadata: HashMap<String, String>,
}

impl VNetBinding {
    pub fn new(vnet: String, container_id: ContainerId, interface_name: String) -> Self {
        Self {
            vnet,
            container_id,
            interface_name,
            bound_at: Utc::now(),
            metadata: HashMap::new(),
        }
    }
}
