//! Advanced Network Functions Example
//!
//! This example demonstrates the advanced network functions implemented in task 8:
//! - Bridge interface creation and management with VLAN-aware support
//! - Bonding operations with various modes
//! - VLAN interface and tagging support

use pve_network_core::{
    bond::{BondConfig, BondManager, LacpRate, PrimaryReselect, XmitHashPolicy},
    bridge::{
        BridgeConfig, BridgeManager, BridgeVlanConfig, VlanAwareBridgeManager,
        VlanProtocol as BridgeVlanProtocol,
    },
    vlan::{QinQConfig, VlanConfig, VlanManager, VlanProtocol},
    BondMode,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Advanced Network Functions Demo ===\n");

    // 1. Advanced Bridge Configuration
    demonstrate_advanced_bridge()?;

    // 2. Advanced Bonding Configuration
    demonstrate_advanced_bonding()?;

    // 3. Advanced VLAN Configuration
    demonstrate_advanced_vlan()?;

    // 4. VLAN-aware Bridge with Port VLANs
    demonstrate_vlan_aware_bridge()?;

    // 5. QinQ (802.1ad) Configuration
    demonstrate_qinq()?;

    println!("=== Demo completed successfully! ===");
    Ok(())
}

fn demonstrate_advanced_bridge() -> Result<(), Box<dyn std::error::Error>> {
    println!("1. Advanced Bridge Configuration");
    println!("================================");

    // Create a VLAN-aware bridge with advanced options
    let bridge_config = BridgeConfig::new()
        .with_port("eth0".to_string())
        .with_port("eth1".to_string())
        .with_vlan_aware(true)
        .with_stp(true)
        .with_priority(32768)
        .with_vlan_filtering(true)
        .with_vlan_default_pvid(1)
        .with_vlan_protocol(BridgeVlanProtocol::Ieee8021Q)
        .with_multicast_snooping(true)
        .with_forward_delay(15)
        .with_hello_time(2);

    // Validate configuration
    BridgeManager::validate_config(&bridge_config)?;
    println!("✓ Bridge configuration validated");

    // Convert to interface config
    let interface_config = bridge_config.to_interface_config("vmbr0".to_string());
    let mut interface = interface_config.to_interface();

    println!("✓ Created VLAN-aware bridge: {}", interface.name);
    println!(
        "  - Ports: {:?}",
        BridgeManager::get_ports(&interface).unwrap()
    );
    println!(
        "  - VLAN-aware: {}",
        BridgeManager::is_vlan_aware(&interface)
    );

    // Add additional port dynamically
    BridgeManager::add_port(&mut interface, "eth2".to_string())?;
    println!("✓ Added port eth2 to bridge");

    // Enable VLAN awareness
    BridgeManager::enable_vlan_aware(&mut interface)?;
    println!("✓ VLAN awareness enabled");

    // Set bridge priority
    BridgeManager::set_priority(&mut interface, 16384)?;
    println!("✓ Bridge priority set to 16384");

    println!();
    Ok(())
}

fn demonstrate_advanced_bonding() -> Result<(), Box<dyn std::error::Error>> {
    println!("2. Advanced Bonding Configuration");
    println!("=================================");

    // Create 802.3ad bond with advanced options
    let bond_config = BondConfig::new(BondMode::Ieee8023ad)
        .with_slave("eth0".to_string())
        .with_slave("eth1".to_string())
        .with_miimon(100)
        .with_updelay(200)
        .with_downdelay(200)
        .with_xmit_hash_policy(XmitHashPolicy::Layer3Plus4)
        .with_lacp_rate(LacpRate::Fast)
        .with_min_links(1);

    // Validate configuration
    BondManager::validate_config(&bond_config)?;
    println!("✓ 802.3ad bond configuration validated");

    // Convert to interface config
    let interface_config = bond_config.to_interface_config("bond0".to_string());
    let mut interface = interface_config.to_interface();

    println!("✓ Created 802.3ad bond: {}", interface.name);
    println!(
        "  - Slaves: {:?}",
        BondManager::get_slaves(&interface).unwrap()
    );
    println!("  - Mode: {:?}", BondManager::get_mode(&interface).unwrap());

    // Set XMIT hash policy
    BondManager::set_xmit_hash_policy(&mut interface, XmitHashPolicy::Layer2Plus3)?;
    println!("✓ XMIT hash policy set to layer2+3");

    // Set LACP rate
    BondManager::set_lacp_rate(&mut interface, LacpRate::Slow)?;
    println!("✓ LACP rate set to slow");

    // Create active-backup bond with primary slave
    let ab_bond_config = BondConfig::new(BondMode::ActiveBackup)
        .with_slave("eth2".to_string())
        .with_slave("eth3".to_string())
        .with_primary("eth2".to_string())
        .with_primary_reselect(PrimaryReselect::Always)
        .with_miimon(100);

    BondManager::validate_config(&ab_bond_config)?;
    let ab_interface_config = ab_bond_config.to_interface_config("bond1".to_string());
    let mut ab_interface = ab_interface_config.to_interface();

    println!("✓ Created active-backup bond: {}", ab_interface.name);

    // Set primary slave
    BondManager::set_primary_slave(&mut ab_interface, "eth3".to_string())?;
    println!("✓ Primary slave changed to eth3");

    // Configure ARP monitoring
    BondManager::configure_arp_monitoring(
        &mut ab_interface,
        1000,
        vec!["192.168.1.1".to_string(), "192.168.1.254".to_string()],
    )?;
    println!("✓ ARP monitoring configured");

    println!();
    Ok(())
}

fn demonstrate_advanced_vlan() -> Result<(), Box<dyn std::error::Error>> {
    println!("3. Advanced VLAN Configuration");
    println!("==============================");

    // Create VLAN with advanced options
    let vlan_config = VlanConfig::new("eth0".to_string(), 100)
        .with_protocol(VlanProtocol::Ieee8021Q)
        .with_ingress_qos_map("1:2,3:4".to_string())
        .with_egress_qos_map("2:1,4:3".to_string())
        .with_gvrp(true)
        .with_loose_binding(false)
        .with_reorder_hdr(true);

    // Validate configuration
    VlanManager::validate_config(&vlan_config)?;
    println!("✓ VLAN configuration validated");

    // Convert to interface config
    let interface_config = vlan_config.to_interface_config("eth0.100".to_string());
    let mut interface = interface_config.to_interface();

    println!("✓ Created VLAN interface: {}", interface.name);
    println!(
        "  - Parent: {}",
        VlanManager::get_parent(&interface).unwrap()
    );
    println!("  - Tag: {}", VlanManager::get_tag(&interface).unwrap());

    // Set QoS mapping
    VlanManager::set_qos_mapping(
        &mut interface,
        Some("0:1,1:2,2:3".to_string()),
        Some("1:0,2:1,3:2".to_string()),
    )?;
    println!("✓ QoS mapping configured");

    // Enable GVRP
    VlanManager::set_gvrp(&mut interface, true)?;
    println!("✓ GVRP enabled");

    // Enable MVRP
    VlanManager::set_mvrp(&mut interface, true)?;
    println!("✓ MVRP enabled");

    // Test VLAN name parsing
    let (parent, tag) = VlanManager::parse_vlan_name("eth0.200").unwrap();
    println!("✓ Parsed VLAN name: parent={}, tag={}", parent, tag);

    // Generate VLAN name
    let generated_name = VlanConfig::generate_name("br0", 300);
    println!("✓ Generated VLAN name: {}", generated_name);

    println!();
    Ok(())
}

fn demonstrate_vlan_aware_bridge() -> Result<(), Box<dyn std::error::Error>> {
    println!("4. VLAN-aware Bridge with Port VLANs");
    println!("=====================================");

    // Create VLAN-aware bridge
    let bridge_config = BridgeConfig::new()
        .with_port("eth0".to_string())
        .with_port("eth1".to_string())
        .with_vlan_aware(true)
        .with_vlan_filtering(true)
        .with_vlan_default_pvid(1);

    let interface_config = bridge_config.to_interface_config("vmbr1".to_string());
    let mut bridge_interface = interface_config.to_interface();

    println!("✓ Created VLAN-aware bridge: {}", bridge_interface.name);

    // Configure VLAN on bridge port eth0
    let port_vlan_config = BridgeVlanConfig::new("eth0".to_string())
        .with_vid(100)
        .with_vid(200)
        .with_vid_range(300, 310)
        .with_pvid(100)
        .with_untagged(true);

    VlanAwareBridgeManager::configure_port_vlan(&mut bridge_interface, "eth0", port_vlan_config)?;
    println!("✓ Configured VLANs on port eth0: 100 (untagged), 200, 300-310");

    // Add VLAN to port
    VlanAwareBridgeManager::add_vlan_to_port(&mut bridge_interface, "eth1", 150, false)?;
    println!("✓ Added VLAN 150 (tagged) to port eth1");

    // Get port VLAN configuration
    if let Some(config) = VlanAwareBridgeManager::get_port_vlan_config(&bridge_interface, "eth0") {
        let all_vids = config.get_all_vids();
        println!("✓ Port eth0 VLANs: {:?}", all_vids);
        println!("  - PVID: {:?}", config.pvid);
        println!("  - VLAN 205 allowed: {}", config.is_vid_allowed(205));
    }

    // Remove VLAN from port
    VlanAwareBridgeManager::remove_vlan_from_port(&mut bridge_interface, "eth0", 200)?;
    println!("✓ Removed VLAN 200 from port eth0");

    println!();
    Ok(())
}

fn demonstrate_qinq() -> Result<(), Box<dyn std::error::Error>> {
    println!("5. QinQ (802.1ad) Configuration");
    println!("===============================");

    // Create QinQ configuration
    let qinq_config = QinQConfig::new("eth0".to_string(), 100, 200);
    let interface_name = qinq_config.generate_name();
    println!("✓ Generated QinQ interface name: {}", interface_name);

    // Convert to VLAN configuration
    let vlan_config = qinq_config.to_vlan_config(interface_name.clone());
    let interface_config = vlan_config.to_interface_config(interface_name.clone());
    let interface = interface_config.to_interface();

    println!("✓ Created QinQ interface: {}", interface.name);
    println!(
        "  - Parent: {}",
        VlanManager::get_parent(&interface).unwrap()
    );
    println!(
        "  - Inner tag: {}",
        VlanManager::get_tag(&interface).unwrap()
    );

    // Parse QinQ name
    if let Some((parent, outer_tag, inner_tag)) = VlanManager::parse_qinq_name(&interface_name) {
        println!("✓ Parsed QinQ name:");
        println!("  - Parent: {}", parent);
        println!("  - Outer tag (S-TAG): {}", outer_tag);
        println!("  - Inner tag (C-TAG): {}", inner_tag);
    }

    // Check if name is QinQ format
    println!(
        "✓ Is QinQ format: {}",
        VlanManager::is_qinq_name(&interface_name)
    );
    println!(
        "✓ Is regular VLAN format: {}",
        VlanManager::is_valid_vlan_name("eth0.100")
    );

    // Create QinQ interface using manager
    let (qinq_name, _qinq_vlan_config) = VlanManager::create_qinq_interface("br0", 500, 600)?;
    println!("✓ Created QinQ interface using manager: {}", qinq_name);

    println!();
    Ok(())
}
