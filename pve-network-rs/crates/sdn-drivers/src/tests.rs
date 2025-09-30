//! SDN Drivers tests

use crate::zones::{SimpleZone, VlanZone};
use pve_sdn_core::{Zone, ZoneConfig, ZoneType};

#[tokio::test]
async fn test_simple_zone_driver() {
    let zone = SimpleZone::new("simple1".to_string());
    assert_eq!(zone.zone_type(), ZoneType::Simple);
    assert_eq!(zone.name(), "simple1");

    // Test valid configuration
    let mut config = ZoneConfig::new(ZoneType::Simple, "simple1".to_string());
    config.bridge = Some("vmbr0".to_string());

    assert!(zone.validate_config(&config).await.is_ok());
    assert!(zone.apply_config(&config).await.is_ok());

    let generated_config = zone.generate_config(&config).await.unwrap();
    assert!(generated_config.contains_key("bridge_vmbr0"));

    // Test invalid configuration - Simple zone with VLAN tag
    let mut invalid_config = ZoneConfig::new(ZoneType::Simple, "simple1".to_string());
    invalid_config.bridge = Some("vmbr0".to_string());
    invalid_config.tag = Some(100); // Should not be allowed for Simple zone

    assert!(zone.validate_config(&invalid_config).await.is_err());
}

#[tokio::test]
async fn test_vlan_zone_driver() {
    let zone = VlanZone::new("vlan1".to_string());
    assert_eq!(zone.zone_type(), ZoneType::Vlan);
    assert_eq!(zone.name(), "vlan1");

    // Test valid configuration
    let mut config = ZoneConfig::new(ZoneType::Vlan, "vlan1".to_string());
    config.bridge = Some("vmbr0".to_string());
    config.vlan_aware = Some(true);

    assert!(zone.validate_config(&config).await.is_ok());
    assert!(zone.apply_config(&config).await.is_ok());

    let generated_config = zone.generate_config(&config).await.unwrap();
    assert!(generated_config.contains_key("bridge_vmbr0"));

    // Test configuration without bridge - should fail
    let mut invalid_config = ZoneConfig::new(ZoneType::Vlan, "vlan1".to_string());
    invalid_config.vlan_aware = Some(true);
    // No bridge specified

    assert!(zone.validate_config(&invalid_config).await.is_err());

    // Test configuration with VXLAN - should fail for VLAN zone
    let mut invalid_config2 = ZoneConfig::new(ZoneType::Vlan, "vlan1".to_string());
    invalid_config2.bridge = Some("vmbr0".to_string());
    invalid_config2.vxlan_port = Some(4789); // Should not be allowed for VLAN zone

    assert!(zone.validate_config(&invalid_config2).await.is_err());
}

#[tokio::test]
async fn test_zone_config_generation() {
    let simple_zone = SimpleZone::new("simple1".to_string());
    let mut config = ZoneConfig::new(ZoneType::Simple, "simple1".to_string());
    config.bridge = Some("vmbr0".to_string());
    config.mtu = Some(1500);

    let generated = simple_zone.generate_config(&config).await.unwrap();
    let bridge_config = generated.get("bridge_vmbr0").unwrap();

    assert!(bridge_config.contains("auto vmbr0"));
    assert!(bridge_config.contains("iface vmbr0 inet manual"));
    assert!(bridge_config.contains("bridge_ports none"));
    assert!(bridge_config.contains("mtu 1500"));

    // Test VLAN zone config generation
    let vlan_zone = VlanZone::new("vlan1".to_string());
    let mut vlan_config = ZoneConfig::new(ZoneType::Vlan, "vlan1".to_string());
    vlan_config.bridge = Some("vmbr1".to_string());
    vlan_config.vlan_aware = Some(true);

    let vlan_generated = vlan_zone.generate_config(&vlan_config).await.unwrap();
    let vlan_bridge_config = vlan_generated.get("bridge_vmbr1").unwrap();

    assert!(vlan_bridge_config.contains("auto vmbr1"));
    assert!(vlan_bridge_config.contains("bridge_vlan_aware yes"));
}
