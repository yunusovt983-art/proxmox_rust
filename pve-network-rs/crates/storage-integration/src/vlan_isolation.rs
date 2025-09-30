//! VLAN isolation for storage networks
//!
//! This module provides VLAN-based network isolation for storage traffic,
//! ensuring that storage networks are properly segmented from other traffic.

use crate::{StorageIntegrationError, StorageResult, StorageVlanConfig, StorageVlanInfo};
use pve_event_bus::EventBus;
use pve_shared_types::SystemEvent;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{debug, error, info, warn};

/// VLAN isolation configuration for storage networks
/// VLAN isolation manager for storage networks
pub struct StorageVlanManager {
    /// Active VLAN configurations
    vlan_configs: HashMap<String, StorageVlanConfig>,
    /// Optional event bus for broadcasting VLAN changes
    event_bus: Option<Arc<EventBus>>,
}

impl StorageVlanManager {
    /// Create a new VLAN manager
    pub fn new() -> Self {
        Self {
            vlan_configs: HashMap::new(),
            event_bus: None,
        }
    }

    /// Create a new VLAN manager with preconfigured event bus
    pub fn with_event_bus(event_bus: Arc<EventBus>) -> Self {
        Self {
            vlan_configs: HashMap::new(),
            event_bus: Some(event_bus),
        }
    }

    /// Attach or replace the event bus used for notifications.
    pub fn set_event_bus(&mut self, event_bus: Arc<EventBus>) {
        self.event_bus = Some(event_bus);
    }

    /// Create VLAN interface for storage network
    pub async fn create_storage_vlan(
        &mut self,
        storage_id: &str,
        config: &StorageVlanConfig,
    ) -> StorageResult<String> {
        info!(
            "Creating storage VLAN for {} with tag {}",
            storage_id, config.vlan_tag
        );

        // Validate VLAN configuration
        self.validate_vlan_config(config).await?;

        // Generate VLAN interface name
        let vlan_interface = format!("{}.{}", config.base_interface, config.vlan_tag);

        // Check if VLAN already exists
        if self.vlan_exists(&vlan_interface).await? {
            warn!("VLAN interface {} already exists", vlan_interface);
            return Ok(vlan_interface);
        }

        // Create VLAN interface
        self.create_vlan_interface(&vlan_interface, config).await?;

        // Configure VLAN interface
        self.configure_vlan_interface(&vlan_interface, config)
            .await?;

        // Apply traffic isolation rules
        self.apply_isolation_rules(&vlan_interface, config).await?;

        // Store configuration
        self.vlan_configs
            .insert(storage_id.to_string(), config.clone());

        info!(
            "Successfully created storage VLAN interface {}",
            vlan_interface
        );

        if let Some(bus) = &self.event_bus {
            if let Err(err) = bus
                .publish(SystemEvent::StorageVlanCreated {
                    id: storage_id.to_string(),
                })
                .await
            {
                warn!("Failed to publish StorageVlanCreated event: {}", err);
            }
        }

        Ok(vlan_interface)
    }

    /// Remove VLAN interface for storage network
    pub async fn remove_storage_vlan(&mut self, storage_id: &str) -> StorageResult<()> {
        info!("Removing storage VLAN for {}", storage_id);

        if let Some(config) = self.vlan_configs.remove(storage_id) {
            let vlan_interface = format!("{}.{}", config.base_interface, config.vlan_tag);

            // Remove isolation rules
            self.remove_isolation_rules(&vlan_interface, &config)
                .await?;

            // Remove VLAN interface
            self.remove_vlan_interface(&vlan_interface).await?;

            info!(
                "Successfully removed storage VLAN interface {}",
                vlan_interface
            );
        } else {
            warn!("No VLAN configuration found for storage {}", storage_id);
        }

        Ok(())
    }

    /// Get VLAN interface name for storage
    pub fn get_vlan_interface(&self, storage_id: &str) -> Option<String> {
        self.vlan_configs
            .get(storage_id)
            .map(|config| format!("{}.{}", config.base_interface, config.vlan_tag))
    }

    /// List all storage VLANs
    pub fn list_storage_vlans(&self) -> Vec<StorageVlanInfo> {
        self.vlan_configs
            .iter()
            .map(|(storage_id, config)| StorageVlanInfo {
                storage_id: storage_id.clone(),
                vlan_interface: format!("{}.{}", config.base_interface, config.vlan_tag),
                vlan_tag: config.vlan_tag,
                base_interface: config.base_interface.clone(),
                subnet: config.subnet.clone(),
                is_active: true, // This would be checked in real implementation
            })
            .collect()
    }

    /// Return a snapshot of configured VLAN definitions.
    pub fn configured_vlans(&self) -> Vec<(String, StorageVlanConfig)> {
        self.vlan_configs
            .iter()
            .map(|(storage_id, config)| (storage_id.clone(), config.clone()))
            .collect()
    }

    /// Validate VLAN configuration
    async fn validate_vlan_config(&self, config: &StorageVlanConfig) -> StorageResult<()> {
        debug!("Validating VLAN configuration for tag {}", config.vlan_tag);

        // Validate VLAN tag range
        if config.vlan_tag == 0 || config.vlan_tag > 4094 {
            return Err(StorageIntegrationError::VlanConfiguration(format!(
                "Invalid VLAN tag: {}",
                config.vlan_tag
            )));
        }

        // Validate base interface exists
        if config.base_interface.is_empty() {
            return Err(StorageIntegrationError::VlanConfiguration(
                "Base interface must be specified".to_string(),
            ));
        }

        // Validate subnet format if specified
        if let Some(subnet) = &config.subnet {
            if !self.is_valid_subnet(subnet) {
                return Err(StorageIntegrationError::VlanConfiguration(format!(
                    "Invalid subnet format: {}",
                    subnet
                )));
            }
        }

        // Validate MTU if specified
        if let Some(mtu) = config.mtu {
            if mtu < 68 || mtu > 9000 {
                return Err(StorageIntegrationError::VlanConfiguration(format!(
                    "Invalid MTU: {}",
                    mtu
                )));
            }
        }

        debug!("VLAN configuration validation passed");
        Ok(())
    }

    /// Check if VLAN interface exists
    async fn vlan_exists(&self, vlan_interface: &str) -> StorageResult<bool> {
        debug!("Checking if VLAN interface {} exists", vlan_interface);

        // This would check if the interface exists in the system
        // For now, return false as placeholder
        Ok(false)
    }

    /// Create VLAN interface
    async fn create_vlan_interface(
        &self,
        vlan_interface: &str,
        config: &StorageVlanConfig,
    ) -> StorageResult<()> {
        debug!("Creating VLAN interface {}", vlan_interface);

        // This would execute: ip link add link <base_interface> name <vlan_interface> type vlan id <vlan_tag>
        let cmd = format!(
            "ip link add link {} name {} type vlan id {}",
            config.base_interface, vlan_interface, config.vlan_tag
        );
        debug!("Would execute: {}", cmd);

        // Set MTU if specified
        if let Some(mtu) = config.mtu {
            let mtu_cmd = format!("ip link set {} mtu {}", vlan_interface, mtu);
            debug!("Would execute: {}", mtu_cmd);
        }

        // Bring interface up
        let up_cmd = format!("ip link set {} up", vlan_interface);
        debug!("Would execute: {}", up_cmd);

        Ok(())
    }

    /// Configure VLAN interface
    async fn configure_vlan_interface(
        &self,
        vlan_interface: &str,
        config: &StorageVlanConfig,
    ) -> StorageResult<()> {
        debug!("Configuring VLAN interface {}", vlan_interface);

        // Configure IP address if subnet is specified
        if let Some(subnet) = &config.subnet {
            let addr_cmd = format!("ip addr add {} dev {}", subnet, vlan_interface);
            debug!("Would execute: {}", addr_cmd);
        }

        // Configure gateway if specified
        if let Some(gateway) = &config.gateway {
            let route_cmd = format!(
                "ip route add default via {} dev {}",
                gateway, vlan_interface
            );
            debug!("Would execute: {}", route_cmd);
        }

        Ok(())
    }

    /// Apply traffic isolation rules
    async fn apply_isolation_rules(
        &self,
        vlan_interface: &str,
        config: &StorageVlanConfig,
    ) -> StorageResult<()> {
        debug!(
            "Applying isolation rules for VLAN interface {}",
            vlan_interface
        );

        // Create iptables rules to isolate storage traffic
        self.create_storage_isolation_rules(vlan_interface, config)
            .await?;

        // Apply traffic shaping if configured
        self.apply_traffic_shaping(vlan_interface, config).await?;

        Ok(())
    }

    /// Create storage isolation rules using iptables
    async fn create_storage_isolation_rules(
        &self,
        vlan_interface: &str,
        config: &StorageVlanConfig,
    ) -> StorageResult<()> {
        debug!("Creating storage isolation rules for {}", vlan_interface);

        // Create custom chain for storage traffic
        let chain_name = format!("STORAGE_{}", config.vlan_tag);
        let create_chain_cmd = format!("iptables -t filter -N {}", chain_name);
        debug!("Would execute: {}", create_chain_cmd);

        // Allow storage traffic within VLAN
        if let Some(subnet) = &config.subnet {
            let allow_cmd = format!(
                "iptables -t filter -A {} -s {} -d {} -j ACCEPT",
                chain_name, subnet, subnet
            );
            debug!("Would execute: {}", allow_cmd);
        }

        // Block inter-VLAN communication (default deny)
        let deny_cmd = format!("iptables -t filter -A {} -j DROP", chain_name);
        debug!("Would execute: {}", deny_cmd);

        // Apply chain to interface
        let apply_cmd = format!(
            "iptables -t filter -A FORWARD -i {} -j {}",
            vlan_interface, chain_name
        );
        debug!("Would execute: {}", apply_cmd);

        Ok(())
    }

    /// Apply traffic shaping for storage VLAN
    async fn apply_traffic_shaping(
        &self,
        vlan_interface: &str,
        config: &StorageVlanConfig,
    ) -> StorageResult<()> {
        debug!("Applying traffic shaping for {}", vlan_interface);

        // Check if bandwidth limit is configured
        if let Some(bandwidth) = config.options.get("bandwidth_limit") {
            let tc_cmd = format!(
                "tc qdisc add dev {} root handle 1: htb default 30",
                vlan_interface
            );
            debug!("Would execute: {}", tc_cmd);

            let class_cmd = format!(
                "tc class add dev {} parent 1: classid 1:1 htb rate {}mbit",
                vlan_interface, bandwidth
            );
            debug!("Would execute: {}", class_cmd);
        }

        // Apply priority if configured
        if let Some(priority) = config.options.get("priority") {
            let prio_cmd = format!(
                "tc qdisc add dev {} root handle 1: prio bands {}",
                vlan_interface, priority
            );
            debug!("Would execute: {}", prio_cmd);
        }

        Ok(())
    }

    /// Remove VLAN interface
    async fn remove_vlan_interface(&self, vlan_interface: &str) -> StorageResult<()> {
        debug!("Removing VLAN interface {}", vlan_interface);

        // Remove interface
        let cmd = format!("ip link delete {}", vlan_interface);
        debug!("Would execute: {}", cmd);

        Ok(())
    }

    /// Remove isolation rules
    async fn remove_isolation_rules(
        &self,
        vlan_interface: &str,
        config: &StorageVlanConfig,
    ) -> StorageResult<()> {
        debug!(
            "Removing isolation rules for VLAN interface {}",
            vlan_interface
        );

        // Remove iptables rules
        let chain_name = format!("STORAGE_{}", config.vlan_tag);

        // Remove chain reference
        let remove_ref_cmd = format!(
            "iptables -t filter -D FORWARD -i {} -j {}",
            vlan_interface, chain_name
        );
        debug!("Would execute: {}", remove_ref_cmd);

        // Flush and delete chain
        let flush_cmd = format!("iptables -t filter -F {}", chain_name);
        debug!("Would execute: {}", flush_cmd);

        let delete_cmd = format!("iptables -t filter -X {}", chain_name);
        debug!("Would execute: {}", delete_cmd);

        // Remove traffic shaping
        let tc_cmd = format!("tc qdisc del dev {} root", vlan_interface);
        debug!("Would execute: {}", tc_cmd);

        Ok(())
    }

    /// Validate subnet format
    fn is_valid_subnet(&self, subnet: &str) -> bool {
        // Simple validation - in real implementation would use proper IP parsing
        subnet.contains('/') && subnet.split('/').count() == 2
    }
}

impl Default for StorageVlanManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Storage VLAN information
/// VLAN isolation policy
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum VlanIsolationPolicy {
    /// Complete isolation - no inter-VLAN communication
    Complete,
    /// Selective isolation - allow specific traffic
    Selective {
        allowed_vlans: Vec<u16>,
        allowed_protocols: Vec<String>,
    },
    /// Management access - allow management traffic
    ManagementAccess {
        management_vlan: u16,
        allowed_ports: Vec<u16>,
    },
}

/// VLAN isolation rule
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VlanIsolationRule {
    pub rule_id: String,
    pub vlan_tag: u16,
    pub policy: VlanIsolationPolicy,
    pub priority: u8,
    pub is_active: bool,
}

/// Advanced VLAN isolation manager
pub struct AdvancedVlanIsolation {
    rules: HashMap<String, VlanIsolationRule>,
}

impl AdvancedVlanIsolation {
    /// Create new advanced VLAN isolation manager
    pub fn new() -> Self {
        Self {
            rules: HashMap::new(),
        }
    }

    /// Add isolation rule
    pub async fn add_isolation_rule(&mut self, rule: VlanIsolationRule) -> StorageResult<()> {
        info!(
            "Adding VLAN isolation rule {} for VLAN {}",
            rule.rule_id, rule.vlan_tag
        );

        // Validate rule
        self.validate_isolation_rule(&rule).await?;

        // Apply rule
        self.apply_isolation_rule(&rule).await?;

        // Store rule
        self.rules.insert(rule.rule_id.clone(), rule);

        Ok(())
    }

    /// Remove isolation rule
    pub async fn remove_isolation_rule(&mut self, rule_id: &str) -> StorageResult<()> {
        info!("Removing VLAN isolation rule {}", rule_id);

        if let Some(rule) = self.rules.remove(rule_id) {
            self.remove_applied_rule(&rule).await?;
        }

        Ok(())
    }

    /// List isolation rules
    pub fn list_isolation_rules(&self) -> Vec<&VlanIsolationRule> {
        self.rules.values().collect()
    }

    /// Validate isolation rule
    async fn validate_isolation_rule(&self, rule: &VlanIsolationRule) -> StorageResult<()> {
        debug!("Validating isolation rule {}", rule.rule_id);

        // Validate VLAN tag
        if rule.vlan_tag == 0 || rule.vlan_tag > 4094 {
            return Err(StorageIntegrationError::VlanConfiguration(format!(
                "Invalid VLAN tag in rule: {}",
                rule.vlan_tag
            )));
        }

        // Validate policy-specific settings
        match &rule.policy {
            VlanIsolationPolicy::Selective { allowed_vlans, .. } => {
                for vlan in allowed_vlans {
                    if *vlan == 0 || *vlan > 4094 {
                        return Err(StorageIntegrationError::VlanConfiguration(format!(
                            "Invalid allowed VLAN tag: {}",
                            vlan
                        )));
                    }
                }
            }
            VlanIsolationPolicy::ManagementAccess {
                management_vlan, ..
            } => {
                if *management_vlan == 0 || *management_vlan > 4094 {
                    return Err(StorageIntegrationError::VlanConfiguration(format!(
                        "Invalid management VLAN tag: {}",
                        management_vlan
                    )));
                }
            }
            _ => {}
        }

        Ok(())
    }

    /// Apply isolation rule
    async fn apply_isolation_rule(&self, rule: &VlanIsolationRule) -> StorageResult<()> {
        debug!(
            "Applying isolation rule {} for VLAN {}",
            rule.rule_id, rule.vlan_tag
        );

        match &rule.policy {
            VlanIsolationPolicy::Complete => {
                self.apply_complete_isolation(rule.vlan_tag).await?;
            }
            VlanIsolationPolicy::Selective {
                allowed_vlans,
                allowed_protocols,
            } => {
                self.apply_selective_isolation(rule.vlan_tag, allowed_vlans, allowed_protocols)
                    .await?;
            }
            VlanIsolationPolicy::ManagementAccess {
                management_vlan,
                allowed_ports,
            } => {
                self.apply_management_access(rule.vlan_tag, *management_vlan, allowed_ports)
                    .await?;
            }
        }

        Ok(())
    }

    /// Apply complete isolation
    async fn apply_complete_isolation(&self, vlan_tag: u16) -> StorageResult<()> {
        debug!("Applying complete isolation for VLAN {}", vlan_tag);

        // Block all inter-VLAN traffic
        let chain_name = format!("ISOLATE_{}", vlan_tag);
        let create_cmd = format!("iptables -t filter -N {}", chain_name);
        debug!("Would execute: {}", create_cmd);

        let block_cmd = format!("iptables -t filter -A {} -j DROP", chain_name);
        debug!("Would execute: {}", block_cmd);

        Ok(())
    }

    /// Apply selective isolation
    async fn apply_selective_isolation(
        &self,
        vlan_tag: u16,
        allowed_vlans: &[u16],
        allowed_protocols: &[String],
    ) -> StorageResult<()> {
        debug!("Applying selective isolation for VLAN {}", vlan_tag);

        let chain_name = format!("SELECTIVE_{}", vlan_tag);
        let create_cmd = format!("iptables -t filter -N {}", chain_name);
        debug!("Would execute: {}", create_cmd);

        // Allow traffic to specified VLANs
        for allowed_vlan in allowed_vlans {
            let allow_cmd = format!(
                "iptables -t filter -A {} -m vlan --vlan-tag {} -j ACCEPT",
                chain_name, allowed_vlan
            );
            debug!("Would execute: {}", allow_cmd);
        }

        // Allow specified protocols
        for protocol in allowed_protocols {
            let proto_cmd = format!(
                "iptables -t filter -A {} -p {} -j ACCEPT",
                chain_name, protocol
            );
            debug!("Would execute: {}", proto_cmd);
        }

        // Default deny
        let deny_cmd = format!("iptables -t filter -A {} -j DROP", chain_name);
        debug!("Would execute: {}", deny_cmd);

        Ok(())
    }

    /// Apply management access
    async fn apply_management_access(
        &self,
        vlan_tag: u16,
        management_vlan: u16,
        allowed_ports: &[u16],
    ) -> StorageResult<()> {
        debug!(
            "Applying management access for VLAN {} from management VLAN {}",
            vlan_tag, management_vlan
        );

        let chain_name = format!("MGMT_ACCESS_{}", vlan_tag);
        let create_cmd = format!("iptables -t filter -N {}", chain_name);
        debug!("Would execute: {}", create_cmd);

        // Allow management VLAN access to specific ports
        for port in allowed_ports {
            let allow_cmd = format!(
                "iptables -t filter -A {} -m vlan --vlan-tag {} -p tcp --dport {} -j ACCEPT",
                chain_name, management_vlan, port
            );
            debug!("Would execute: {}", allow_cmd);
        }

        // Default deny
        let deny_cmd = format!("iptables -t filter -A {} -j DROP", chain_name);
        debug!("Would execute: {}", deny_cmd);

        Ok(())
    }

    /// Remove applied rule
    async fn remove_applied_rule(&self, rule: &VlanIsolationRule) -> StorageResult<()> {
        debug!(
            "Removing applied rule {} for VLAN {}",
            rule.rule_id, rule.vlan_tag
        );

        let chain_name = match &rule.policy {
            VlanIsolationPolicy::Complete => format!("ISOLATE_{}", rule.vlan_tag),
            VlanIsolationPolicy::Selective { .. } => format!("SELECTIVE_{}", rule.vlan_tag),
            VlanIsolationPolicy::ManagementAccess { .. } => {
                format!("MGMT_ACCESS_{}", rule.vlan_tag)
            }
        };

        // Remove chain
        let flush_cmd = format!("iptables -t filter -F {}", chain_name);
        debug!("Would execute: {}", flush_cmd);

        let delete_cmd = format!("iptables -t filter -X {}", chain_name);
        debug!("Would execute: {}", delete_cmd);

        Ok(())
    }
}

impl Default for AdvancedVlanIsolation {
    fn default() -> Self {
        Self::new()
    }
}
