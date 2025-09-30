use std::collections::HashMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageNetworkConfig {
    pub backend_type: StorageBackendType,
    pub interface: String,
    pub vlan_tag: Option<u16>,
    #[serde(default)]
    pub network_options: HashMap<String, String>,
    pub qos_settings: Option<QosSettings>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum StorageBackendType {
    Nfs {
        server: String,
        export: String,
        version: Option<String>,
        options: HashMap<String, String>,
    },
    Cifs {
        server: String,
        share: String,
        username: Option<String>,
        domain: Option<String>,
        options: HashMap<String, String>,
    },
    Iscsi {
        portal: String,
        target: String,
        lun: Option<u32>,
        options: HashMap<String, String>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QosSettings {
    pub bandwidth_limit: Option<u32>,
    pub priority: Option<u8>,
    pub dscp: Option<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageNetworkInfo {
    pub storage_id: String,
    pub backend_type: StorageBackendType,
    pub interface: String,
    pub vlan_tag: Option<u16>,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageNetworkStatus {
    pub storage_id: String,
    pub backend_type: StorageBackendType,
    pub interface: String,
    pub vlan_tag: Option<u16>,
    pub is_active: bool,
    pub last_check: DateTime<Utc>,
    pub error_message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageVlanConfig {
    pub base_interface: String,
    pub vlan_tag: u16,
    pub subnet: Option<String>,
    pub gateway: Option<String>,
    pub mtu: Option<u16>,
    #[serde(default)]
    pub options: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageVlanInfo {
    pub storage_id: String,
    pub vlan_interface: String,
    pub vlan_tag: u16,
    pub base_interface: String,
    pub subnet: Option<String>,
    pub is_active: bool,
}
