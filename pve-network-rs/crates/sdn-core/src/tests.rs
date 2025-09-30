//! SDN Core tests

use crate::*;
use ipnet::IpNet;

#[tokio::test]
async fn test_zone_configuration() {
    // Test Simple zone
    let simple_zone = ZoneConfig::new(ZoneType::Simple, "simple1".to_string());
    assert!(simple_zone.validate().is_ok());

    // Test VLAN zone
    let mut vlan_zone = ZoneConfig::new(ZoneType::Vlan, "vlan1".to_string());
    vlan_zone.bridge = Some("vmbr0".to_string());
    vlan_zone.vlan_aware = Some(true);
    assert!(vlan_zone.validate().is_ok());

    // Test invalid VLAN tag
    let mut invalid_zone = ZoneConfig::new(ZoneType::Vlan, "invalid".to_string());
    invalid_zone.tag = Some(5000); // Invalid VLAN tag
    assert!(invalid_zone.validate().is_err());
}

#[tokio::test]
async fn test_vnet_configuration() {
    let vnet = VNetConfig::new("vnet1".to_string(), "zone1".to_string());
    assert!(vnet.validate().is_ok());

    // Test with VLAN tag
    let mut vnet_with_tag = VNetConfig::new("vnet2".to_string(), "zone1".to_string());
    vnet_with_tag.tag = Some(100);
    assert!(vnet_with_tag.validate().is_ok());

    // Test with invalid MAC
    let mut vnet_invalid_mac = VNetConfig::new("vnet3".to_string(), "zone1".to_string());
    vnet_invalid_mac.mac = Some("invalid-mac".to_string());
    assert!(vnet_invalid_mac.validate().is_err());

    // Test with valid MAC
    let mut vnet_valid_mac = VNetConfig::new("vnet4".to_string(), "zone1".to_string());
    vnet_valid_mac.mac = Some("02:00:00:00:00:01".to_string());
    assert!(vnet_valid_mac.validate().is_ok());
}

#[tokio::test]
async fn test_subnet_configuration() {
    let cidr: IpNet = "192.168.1.0/24".parse().unwrap();
    let subnet = SubnetConfig::new("subnet1".to_string(), "vnet1".to_string(), cidr);
    assert!(subnet.validate().is_ok());

    // Test with gateway
    let mut subnet_with_gw = SubnetConfig::new("subnet2".to_string(), "vnet1".to_string(), cidr);
    subnet_with_gw.gateway = Some("192.168.1.1".parse().unwrap());
    assert!(subnet_with_gw.validate().is_ok());

    // Test with invalid gateway (outside subnet)
    let mut subnet_invalid_gw = SubnetConfig::new("subnet3".to_string(), "vnet1".to_string(), cidr);
    subnet_invalid_gw.gateway = Some("10.0.0.1".parse().unwrap());
    assert!(subnet_invalid_gw.validate().is_err());
}

#[tokio::test]
async fn test_sdn_configuration_dependencies() {
    let mut config = SdnConfiguration::new();

    // Add zone
    let zone = ZoneConfig::new(ZoneType::Simple, "zone1".to_string());
    config.add_zone(zone).unwrap();

    // Add VNet
    let vnet = VNetConfig::new("vnet1".to_string(), "zone1".to_string());
    config.add_vnet(vnet).unwrap();

    // Add subnet
    let cidr: IpNet = "192.168.1.0/24".parse().unwrap();
    let subnet = SubnetConfig::new("subnet1".to_string(), "vnet1".to_string(), cidr);
    config.add_subnet(subnet).unwrap();

    // Validate complete configuration
    assert!(config.validate().is_ok());

    // Test dependency validation - try to remove zone with dependent VNet
    assert!(config.remove_zone("zone1").is_err());

    // Remove subnet first
    config.remove_subnet("subnet1").unwrap();

    // Try to remove zone with dependent VNet - should still fail
    assert!(config.remove_zone("zone1").is_err());

    // Remove VNet, then zone should work
    config.remove_vnet("vnet1").unwrap();
    config.remove_zone("zone1").unwrap();

    assert!(config.zones.is_empty());
    assert!(config.vnets.is_empty());
    assert!(config.subnets.is_empty());
}

#[tokio::test]
async fn test_sdn_configuration_serialization() {
    let mut config = SdnConfiguration::new();

    // Add complete configuration
    let zone = ZoneConfig::new(ZoneType::Simple, "zone1".to_string());
    config.add_zone(zone).unwrap();

    let vnet = VNetConfig::new("vnet1".to_string(), "zone1".to_string());
    config.add_vnet(vnet).unwrap();

    let cidr: IpNet = "192.168.1.0/24".parse().unwrap();
    let subnet = SubnetConfig::new("subnet1".to_string(), "vnet1".to_string(), cidr);
    config.add_subnet(subnet).unwrap();

    // Test JSON serialization
    let json = config.to_json().unwrap();
    assert!(!json.is_empty());

    // Test JSON deserialization
    let parsed_config = SdnConfiguration::from_json(&json).unwrap();
    assert_eq!(config.zones.len(), parsed_config.zones.len());
    assert_eq!(config.vnets.len(), parsed_config.vnets.len());
    assert_eq!(config.subnets.len(), parsed_config.subnets.len());
}
