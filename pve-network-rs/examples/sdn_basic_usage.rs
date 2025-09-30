//! Basic SDN usage example
//!
//! This example demonstrates how to use the basic SDN functionality:
//! - Creating zones, VNets, and subnets
//! - Validating configurations
//! - Generating network configurations

use anyhow::Result;
use ipnet::IpNet;
use pve_sdn_core::Zone;
use pve_sdn_core::{SdnConfiguration, SubnetConfig, SubnetType, VNetConfig, ZoneConfig, ZoneType};
use pve_sdn_drivers::zones::{SimpleZone, VlanZone};

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();

    println!("=== Proxmox VE SDN Basic Usage Example ===\n");

    // Create a complete SDN configuration
    let mut sdn_config = SdnConfiguration::new();

    // 1. Create and add a Simple zone
    println!("1. Creating Simple zone...");
    let mut simple_zone_config = ZoneConfig::new(ZoneType::Simple, "simple1".to_string());
    simple_zone_config.bridge = Some("vmbr0".to_string());
    simple_zone_config.mtu = Some(1500);

    sdn_config.add_zone(simple_zone_config.clone())?;
    println!("   ✓ Simple zone 'simple1' created");

    // 2. Create and add a VLAN zone
    println!("2. Creating VLAN zone...");
    let mut vlan_zone_config = ZoneConfig::new(ZoneType::Vlan, "vlan1".to_string());
    vlan_zone_config.bridge = Some("vmbr1".to_string());
    vlan_zone_config.vlan_aware = Some(true);
    vlan_zone_config.mtu = Some(1500);

    sdn_config.add_zone(vlan_zone_config.clone())?;
    println!("   ✓ VLAN zone 'vlan1' created");

    // 3. Create VNets
    println!("3. Creating VNets...");

    // VNet in Simple zone
    let simple_vnet = VNetConfig::new("vnet-simple".to_string(), "simple1".to_string());
    sdn_config.add_vnet(simple_vnet)?;
    println!("   ✓ VNet 'vnet-simple' created in Simple zone");

    // VNet in VLAN zone with tag
    let mut vlan_vnet = VNetConfig::new("vnet-vlan".to_string(), "vlan1".to_string());
    vlan_vnet.tag = Some(100);
    sdn_config.add_vnet(vlan_vnet)?;
    println!("   ✓ VNet 'vnet-vlan' created in VLAN zone with tag 100");

    // 4. Create subnets
    println!("4. Creating subnets...");

    // Subnet in Simple VNet
    let simple_cidr: IpNet = "192.168.1.0/24".parse()?;
    let mut simple_subnet = SubnetConfig::new(
        "subnet-simple".to_string(),
        "vnet-simple".to_string(),
        simple_cidr,
    );
    simple_subnet.gateway = Some("192.168.1.1".parse()?);
    sdn_config.add_subnet(simple_subnet)?;
    println!("   ✓ Subnet 'subnet-simple' created: 192.168.1.0/24");

    // Subnet in VLAN VNet
    let vlan_cidr: IpNet = "10.0.100.0/24".parse()?;
    let mut vlan_subnet = SubnetConfig::new(
        "subnet-vlan".to_string(),
        "vnet-vlan".to_string(),
        vlan_cidr,
    );
    vlan_subnet.gateway = Some("10.0.100.1".parse()?);
    sdn_config.add_subnet(vlan_subnet)?;
    println!("   ✓ Subnet 'subnet-vlan' created: 10.0.100.0/24");

    // 5. Validate complete configuration
    println!("5. Validating complete SDN configuration...");
    sdn_config.validate()?;
    println!("   ✓ Configuration validation passed");

    // 6. Test zone drivers
    println!("6. Testing zone drivers...");

    // Test Simple zone driver
    let simple_zone = SimpleZone::new("simple1".to_string());
    simple_zone.validate_config(&simple_zone_config).await?;
    let simple_configs = simple_zone.generate_config(&simple_zone_config).await?;
    println!("   ✓ Simple zone driver validation and config generation successful");
    println!(
        "     Generated {} configuration files",
        simple_configs.len()
    );

    // Test VLAN zone driver
    let vlan_zone = VlanZone::new("vlan1".to_string());
    vlan_zone.validate_config(&vlan_zone_config).await?;
    let vlan_configs = vlan_zone.generate_config(&vlan_zone_config).await?;
    println!("   ✓ VLAN zone driver validation and config generation successful");
    println!("     Generated {} configuration files", vlan_configs.len());

    // 7. Display configuration summary
    println!("\n7. Configuration Summary:");
    println!("   Zones: {}", sdn_config.zones.len());
    for (name, zone) in &sdn_config.zones {
        println!("     - {} ({})", name, zone.zone_type);
    }

    println!("   VNets: {}", sdn_config.vnets.len());
    for (name, vnet) in &sdn_config.vnets {
        println!("     - {} (zone: {})", name, vnet.zone);
        if let Some(tag) = vnet.tag {
            println!("       VLAN tag: {}", tag);
        }
    }

    println!("   Subnets: {}", sdn_config.subnets.len());
    for (name, subnet) in &sdn_config.subnets {
        println!(
            "     - {} (vnet: {}, cidr: {})",
            name, subnet.vnet, subnet.cidr
        );
        if let Some(gw) = subnet.gateway {
            println!("       Gateway: {}", gw);
        }
    }

    // 8. Export configuration as JSON
    println!("\n8. Exporting configuration...");
    let json_config = sdn_config.to_json()?;
    println!(
        "   ✓ Configuration exported to JSON ({} bytes)",
        json_config.len()
    );

    // 9. Test configuration import
    println!("9. Testing configuration import...");
    let imported_config = SdnConfiguration::from_json(&json_config)?;
    println!("   ✓ Configuration imported successfully");
    println!(
        "   ✓ Imported {} zones, {} VNets, {} subnets",
        imported_config.zones.len(),
        imported_config.vnets.len(),
        imported_config.subnets.len()
    );

    println!("\n=== SDN Basic Usage Example Completed Successfully ===");

    Ok(())
}
