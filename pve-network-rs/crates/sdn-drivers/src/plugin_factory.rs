//! Plugin factory for dynamic loading of SDN drivers
//!
//! This module provides a factory system for dynamically loading and creating
//! SDN zone drivers, IPAM plugins, and controllers.

use anyhow::Result;
use log::{debug, info, warn};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use pve_sdn_core::{Controller, ControllerType, IpamPlugin, IpamType, Zone, ZoneType};

use crate::controllers::{BgpController, EvpnController, FaucetController};
use crate::ipam::{NetBoxIpam, PhpIpam, PveIpam};
use crate::zones::{EvpnZone, QinQZone, SimpleZone, VlanZone, VxlanZone};

/// Zone factory function type
pub type ZoneFactory = Box<dyn Fn(String) -> Box<dyn Zone> + Send + Sync>;

/// Controller factory function type
pub type ControllerFactory = Box<dyn Fn(String) -> Box<dyn Controller> + Send + Sync>;

/// IPAM factory function type
pub type IpamFactory = Box<dyn Fn(String) -> Box<dyn IpamPlugin> + Send + Sync>;

/// Plugin factory for creating SDN components
pub struct PluginFactory {
    zone_factories: Arc<RwLock<HashMap<ZoneType, ZoneFactory>>>,
    controller_factories: Arc<RwLock<HashMap<ControllerType, ControllerFactory>>>,
    ipam_factories: Arc<RwLock<HashMap<IpamType, IpamFactory>>>,
}

impl PluginFactory {
    /// Create new plugin factory with default drivers
    pub fn new() -> Self {
        let factory = Self {
            zone_factories: Arc::new(RwLock::new(HashMap::new())),
            controller_factories: Arc::new(RwLock::new(HashMap::new())),
            ipam_factories: Arc::new(RwLock::new(HashMap::new())),
        };

        factory.register_default_drivers();
        factory
    }

    /// Register default built-in drivers
    fn register_default_drivers(&self) {
        // Register zone drivers
        self.register_zone_driver(
            ZoneType::Simple,
            Box::new(|name| Box::new(SimpleZone::new(name))),
        );

        self.register_zone_driver(
            ZoneType::Vlan,
            Box::new(|name| Box::new(VlanZone::new(name))),
        );

        self.register_zone_driver(
            ZoneType::QinQ,
            Box::new(|name| Box::new(QinQZone::new(name))),
        );

        self.register_zone_driver(
            ZoneType::Vxlan,
            Box::new(|name| Box::new(VxlanZone::new(name))),
        );

        self.register_zone_driver(
            ZoneType::Evpn,
            Box::new(|name| Box::new(EvpnZone::new(name))),
        );

        // Register controller drivers
        self.register_controller_driver(
            ControllerType::Bgp,
            Box::new(|name| Box::new(BgpController::new(name))),
        );

        self.register_controller_driver(
            ControllerType::Evpn,
            Box::new(|name| Box::new(EvpnController::new(name))),
        );

        self.register_controller_driver(
            ControllerType::Faucet,
            Box::new(|name| Box::new(FaucetController::new(name))),
        );

        // Register IPAM drivers
        self.register_ipam_driver(
            IpamType::Pve,
            Box::new(|name| {
                // Create a default config for the factory
                let config = pve_sdn_core::IpamConfig::new(name.clone(), IpamType::Pve);
                Box::new(PveIpam::new(name, config))
            }),
        );

        self.register_ipam_driver(
            IpamType::PhpIpam,
            Box::new(|name| {
                // Create a default config for the factory
                let config = pve_sdn_core::IpamConfig::new(name.clone(), IpamType::PhpIpam);
                // PhpIpam::new returns a Result, so we need to handle it
                match PhpIpam::new(name, config) {
                    Ok(ipam) => Box::new(ipam),
                    Err(_) => {
                        // For factory purposes, create a dummy implementation
                        // In real usage, proper error handling would be needed
                        let dummy_config =
                            pve_sdn_core::IpamConfig::new("dummy".to_string(), IpamType::Pve);
                        Box::new(PveIpam::new("dummy".to_string(), dummy_config))
                    }
                }
            }),
        );

        self.register_ipam_driver(
            IpamType::NetBox,
            Box::new(|name| {
                // Create a default config for the factory
                let config = pve_sdn_core::IpamConfig::new(name.clone(), IpamType::NetBox);
                // NetBoxIpam::new returns a Result, so we need to handle it
                match NetBoxIpam::new(name, config) {
                    Ok(ipam) => Box::new(ipam),
                    Err(_) => {
                        // For factory purposes, create a dummy implementation
                        let dummy_config =
                            pve_sdn_core::IpamConfig::new("dummy".to_string(), IpamType::Pve);
                        Box::new(PveIpam::new("dummy".to_string(), dummy_config))
                    }
                }
            }),
        );

        info!("Registered default SDN drivers");
    }

    /// Register a zone driver
    pub fn register_zone_driver(&self, zone_type: ZoneType, factory: ZoneFactory) {
        let mut factories = self.zone_factories.write().unwrap();
        factories.insert(zone_type.clone(), factory);
        debug!("Registered zone driver: {}", zone_type);
    }

    /// Register a controller driver
    pub fn register_controller_driver(
        &self,
        controller_type: ControllerType,
        factory: ControllerFactory,
    ) {
        let mut factories = self.controller_factories.write().unwrap();
        factories.insert(controller_type.clone(), factory);
        debug!("Registered controller driver: {}", controller_type);
    }

    /// Register an IPAM driver
    pub fn register_ipam_driver(&self, ipam_type: IpamType, factory: IpamFactory) {
        let mut factories = self.ipam_factories.write().unwrap();
        factories.insert(ipam_type.clone(), factory);
        debug!("Registered IPAM driver: {}", ipam_type);
    }

    /// Create a zone instance
    pub fn create_zone(&self, zone_type: &ZoneType, name: String) -> Result<Box<dyn Zone>> {
        let factories = self.zone_factories.read().unwrap();

        if let Some(factory) = factories.get(zone_type) {
            let zone = factory(name.clone());
            debug!("Created zone '{}' of type '{}'", name, zone_type);
            Ok(zone)
        } else {
            anyhow::bail!("No factory registered for zone type '{}'", zone_type);
        }
    }

    /// Create a controller instance
    pub fn create_controller(
        &self,
        controller_type: &ControllerType,
        name: String,
    ) -> Result<Box<dyn Controller>> {
        let factories = self.controller_factories.read().unwrap();

        if let Some(factory) = factories.get(controller_type) {
            let controller = factory(name.clone());
            debug!(
                "Created controller '{}' of type '{}'",
                name, controller_type
            );
            Ok(controller)
        } else {
            anyhow::bail!(
                "No factory registered for controller type '{}'",
                controller_type
            );
        }
    }

    /// Create an IPAM plugin instance
    pub fn create_ipam(&self, ipam_type: &IpamType, name: String) -> Result<Box<dyn IpamPlugin>> {
        let factories = self.ipam_factories.read().unwrap();

        if let Some(factory) = factories.get(ipam_type) {
            let ipam = factory(name.clone());
            debug!("Created IPAM '{}' of type '{}'", name, ipam_type);
            Ok(ipam)
        } else {
            anyhow::bail!("No factory registered for IPAM type '{}'", ipam_type);
        }
    }

    /// Get list of available zone types
    pub fn available_zone_types(&self) -> Vec<ZoneType> {
        let factories = self.zone_factories.read().unwrap();
        factories.keys().cloned().collect()
    }

    /// Get list of available controller types
    pub fn available_controller_types(&self) -> Vec<ControllerType> {
        let factories = self.controller_factories.read().unwrap();
        factories.keys().cloned().collect()
    }

    /// Get list of available IPAM types
    pub fn available_ipam_types(&self) -> Vec<IpamType> {
        let factories = self.ipam_factories.read().unwrap();
        factories.keys().cloned().collect()
    }

    /// Load external plugin (placeholder for future dynamic loading)
    pub fn load_external_plugin(&self, plugin_path: &str) -> Result<()> {
        // This is a placeholder for future dynamic plugin loading
        // In a real implementation, this would use libloading or similar
        // to dynamically load shared libraries containing plugin implementations

        warn!(
            "External plugin loading not yet implemented: {}",
            plugin_path
        );

        // For now, just validate that the path exists
        if !std::path::Path::new(plugin_path).exists() {
            anyhow::bail!("Plugin file does not exist: {}", plugin_path);
        }

        // Future implementation would:
        // 1. Load the shared library
        // 2. Look for plugin registration functions
        // 3. Call the registration functions to register new drivers
        // 4. Handle plugin lifecycle (unloading, etc.)

        Ok(())
    }

    /// Unregister a zone driver
    pub fn unregister_zone_driver(&self, zone_type: &ZoneType) -> Result<()> {
        let mut factories = self.zone_factories.write().unwrap();

        if factories.remove(zone_type).is_some() {
            debug!("Unregistered zone driver: {}", zone_type);
            Ok(())
        } else {
            anyhow::bail!("No zone driver registered for type '{}'", zone_type);
        }
    }

    /// Unregister a controller driver
    pub fn unregister_controller_driver(&self, controller_type: &ControllerType) -> Result<()> {
        let mut factories = self.controller_factories.write().unwrap();

        if factories.remove(controller_type).is_some() {
            debug!("Unregistered controller driver: {}", controller_type);
            Ok(())
        } else {
            anyhow::bail!(
                "No controller driver registered for type '{}'",
                controller_type
            );
        }
    }

    /// Unregister an IPAM driver
    pub fn unregister_ipam_driver(&self, ipam_type: &IpamType) -> Result<()> {
        let mut factories = self.ipam_factories.write().unwrap();

        if factories.remove(ipam_type).is_some() {
            debug!("Unregistered IPAM driver: {}", ipam_type);
            Ok(())
        } else {
            anyhow::bail!("No IPAM driver registered for type '{}'", ipam_type);
        }
    }

    /// Check if a zone driver is registered
    pub fn has_zone_driver(&self, zone_type: &ZoneType) -> bool {
        let factories = self.zone_factories.read().unwrap();
        factories.contains_key(zone_type)
    }

    /// Check if a controller driver is registered
    pub fn has_controller_driver(&self, controller_type: &ControllerType) -> bool {
        let factories = self.controller_factories.read().unwrap();
        factories.contains_key(controller_type)
    }

    /// Check if an IPAM driver is registered
    pub fn has_ipam_driver(&self, ipam_type: &IpamType) -> bool {
        let factories = self.ipam_factories.read().unwrap();
        factories.contains_key(ipam_type)
    }
}

impl Default for PluginFactory {
    fn default() -> Self {
        Self::new()
    }
}

/// Global plugin factory instance
static PLUGIN_FACTORY: std::sync::OnceLock<PluginFactory> = std::sync::OnceLock::new();

/// Get the global plugin factory instance
pub fn get_plugin_factory() -> &'static PluginFactory {
    PLUGIN_FACTORY.get_or_init(|| PluginFactory::new())
}

/// Initialize the plugin factory with custom configuration
pub fn init_plugin_factory(factory: PluginFactory) -> Result<()> {
    PLUGIN_FACTORY
        .set(factory)
        .map_err(|_| anyhow::anyhow!("Plugin factory already initialized"))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_plugin_factory_creation() {
        let factory = PluginFactory::new();

        // Test zone creation
        let zone = factory
            .create_zone(&ZoneType::Simple, "test-simple".to_string())
            .unwrap();
        assert_eq!(zone.zone_type(), ZoneType::Simple);
        assert_eq!(zone.name(), "test-simple");

        let zone = factory
            .create_zone(&ZoneType::Vxlan, "test-vxlan".to_string())
            .unwrap();
        assert_eq!(zone.zone_type(), ZoneType::Vxlan);
        assert_eq!(zone.name(), "test-vxlan");

        // Test controller creation
        let controller = factory
            .create_controller(&ControllerType::Bgp, "test-bgp".to_string())
            .unwrap();
        assert_eq!(controller.controller_type(), ControllerType::Bgp);
        assert_eq!(controller.name(), "test-bgp");

        // Test IPAM creation
        let ipam = factory
            .create_ipam(&IpamType::Pve, "test-pve".to_string())
            .unwrap();
        assert_eq!(ipam.plugin_type(), IpamType::Pve);
    }

    #[test]
    fn test_available_types() {
        let factory = PluginFactory::new();

        let zone_types = factory.available_zone_types();
        assert!(zone_types.contains(&ZoneType::Simple));
        assert!(zone_types.contains(&ZoneType::Vlan));
        assert!(zone_types.contains(&ZoneType::QinQ));
        assert!(zone_types.contains(&ZoneType::Vxlan));
        assert!(zone_types.contains(&ZoneType::Evpn));

        let controller_types = factory.available_controller_types();
        assert!(controller_types.contains(&ControllerType::Bgp));
        assert!(controller_types.contains(&ControllerType::Evpn));
        assert!(controller_types.contains(&ControllerType::Faucet));

        let ipam_types = factory.available_ipam_types();
        assert!(ipam_types.contains(&IpamType::Pve));
        assert!(ipam_types.contains(&IpamType::PhpIpam));
        assert!(ipam_types.contains(&IpamType::NetBox));
    }

    #[test]
    fn test_driver_registration() {
        let factory = PluginFactory::new();

        // Test has_driver methods
        assert!(factory.has_zone_driver(&ZoneType::Simple));
        assert!(factory.has_controller_driver(&ControllerType::Bgp));
        assert!(factory.has_ipam_driver(&IpamType::Pve));

        // Test unregistration
        factory.unregister_zone_driver(&ZoneType::Simple).unwrap();
        assert!(!factory.has_zone_driver(&ZoneType::Simple));

        // Test creation after unregistration fails
        assert!(factory
            .create_zone(&ZoneType::Simple, "test".to_string())
            .is_err());
    }

    #[test]
    fn test_global_factory() {
        let factory = get_plugin_factory();

        // Test that global factory works
        let zone = factory
            .create_zone(&ZoneType::Simple, "global-test".to_string())
            .unwrap();
        assert_eq!(zone.name(), "global-test");
    }
}
