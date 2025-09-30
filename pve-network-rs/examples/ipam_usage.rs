//! IPAM Usage Example
//!
//! Demonstrates how to use the IPAM functionality with different plugins

use anyhow::Result;
use std::net::IpAddr;
use std::sync::Arc;

use pve_sdn_core::{
    IpAllocationRequest, IpamConfig, IpamManager, IpamType, Subnet, SubnetConfig, SubnetType,
};
use pve_sdn_drivers::{IpamPluginFactory, NetBoxIpam, PhpIpam, PveIpam};

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();

    println!("=== Proxmox VE IPAM Usage Example ===\n");

    // Create IPAM manager
    let mut ipam_manager = IpamManager::new();

    // Setup PVE IPAM (built-in)
    setup_pve_ipam(&mut ipam_manager).await?;

    // Setup external IPAM systems (commented out as they require actual services)
    // setup_phpipam(&mut ipam_manager).await?;
    // setup_netbox(&mut ipam_manager).await?;

    // Demonstrate IPAM operations
    demonstrate_ipam_operations(&ipam_manager).await?;

    // Demonstrate Perl compatibility
    demonstrate_perl_compatibility().await?;

    println!("\n=== IPAM Example Complete ===");
    Ok(())
}

async fn setup_pve_ipam(manager: &mut IpamManager) -> Result<()> {
    println!("Setting up PVE IPAM...");

    let config = IpamConfig::new("pve-ipam".to_string(), IpamType::Pve);
    let pve_ipam = Arc::new(PveIpam::new("pve-ipam".to_string(), config));

    manager.register_plugin(pve_ipam);
    manager.set_default_plugin("pve-ipam")?;

    println!("✓ PVE IPAM configured as default\n");
    Ok(())
}

#[allow(dead_code)]
async fn setup_phpipam(manager: &mut IpamManager) -> Result<()> {
    println!("Setting up phpIPAM...");

    let mut config = IpamConfig::new("phpipam".to_string(), IpamType::PhpIpam);
    config.url = Some("http://phpipam.example.com".to_string());
    config.username = Some("admin".to_string());
    config.password = Some("password".to_string());
    config.section = Some("1".to_string());

    let phpipam = Arc::new(PhpIpam::new("phpipam".to_string(), config)?);
    manager.register_plugin(phpipam);

    println!("✓ phpIPAM configured\n");
    Ok(())
}

#[allow(dead_code)]
async fn setup_netbox(manager: &mut IpamManager) -> Result<()> {
    println!("Setting up NetBox IPAM...");

    let mut config = IpamConfig::new("netbox".to_string(), IpamType::NetBox);
    config.url = Some("http://netbox.example.com".to_string());
    config.token = Some("your-netbox-token".to_string());
    config.tenant = Some("1".to_string());

    let netbox = Arc::new(NetBoxIpam::new("netbox".to_string(), config)?);
    manager.register_plugin(netbox);

    println!("✓ NetBox IPAM configured\n");
    Ok(())
}

async fn demonstrate_ipam_operations(manager: &IpamManager) -> Result<()> {
    println!("=== Demonstrating IPAM Operations ===\n");

    // List available plugins
    let plugins = manager.list_plugins();
    println!("Available IPAM plugins:");
    for (name, plugin_type) in &plugins {
        println!("  - {} ({:?})", name, plugin_type);
    }
    println!();

    // Create test subnet
    let subnet = create_test_subnet("demo-subnet", "192.168.100.0/24")?;
    println!("Created test subnet: {} ({})", subnet.name(), subnet.cidr());

    // Add subnet to IPAM
    manager.add_subnet(None, &subnet).await?;
    println!("✓ Added subnet to IPAM\n");

    // Demonstrate IP allocation scenarios
    demonstrate_automatic_allocation(manager).await?;
    demonstrate_specific_allocation(manager).await?;
    demonstrate_ip_management(manager).await?;

    // Clean up
    manager.remove_subnet(None, "demo-subnet").await?;
    println!("✓ Cleaned up test subnet");

    Ok(())
}

async fn demonstrate_automatic_allocation(manager: &IpamManager) -> Result<()> {
    println!("--- Automatic IP Allocation ---");

    let request = IpAllocationRequest {
        subnet: "demo-subnet".to_string(),
        vmid: Some(100),
        hostname: Some("vm-100".to_string()),
        mac: Some("00:11:22:33:44:55".to_string()),
        description: Some("Demo VM 100".to_string()),
        requested_ip: None, // Let IPAM choose
    };

    let allocation = manager.allocate_ip(None, &request).await?;
    println!(
        "✓ Allocated IP {} for VM {}",
        allocation.ip,
        allocation.vmid.unwrap()
    );

    // Check IP availability
    let is_available = manager
        .is_ip_available(None, "demo-subnet", &allocation.ip)
        .await?;
    println!("  IP {} available: {}", allocation.ip, is_available);

    // Get next free IP
    let next_ip = manager.get_next_free_ip(None, "demo-subnet").await?;
    if let Some(ip) = next_ip {
        println!("  Next free IP: {}", ip);
    }

    println!();
    Ok(())
}

async fn demonstrate_specific_allocation(manager: &IpamManager) -> Result<()> {
    println!("--- Specific IP Allocation ---");

    let requested_ip: IpAddr = "192.168.100.50".parse()?;
    let request = IpAllocationRequest {
        subnet: "demo-subnet".to_string(),
        vmid: Some(200),
        hostname: Some("vm-200".to_string()),
        mac: Some("00:11:22:33:44:66".to_string()),
        description: Some("Demo VM 200".to_string()),
        requested_ip: Some(requested_ip),
    };

    let allocation = manager.allocate_ip(None, &request).await?;
    println!(
        "✓ Allocated specific IP {} for VM {}",
        allocation.ip,
        allocation.vmid.unwrap()
    );

    // Try to allocate the same IP again (should fail)
    let duplicate_request = IpAllocationRequest {
        subnet: "demo-subnet".to_string(),
        vmid: Some(201),
        hostname: Some("vm-201".to_string()),
        mac: Some("00:11:22:33:44:67".to_string()),
        description: Some("Demo VM 201".to_string()),
        requested_ip: Some(requested_ip),
    };

    match manager.allocate_ip(None, &duplicate_request).await {
        Ok(_) => println!("  ⚠ Unexpected: duplicate allocation succeeded"),
        Err(e) => println!("  ✓ Correctly rejected duplicate allocation: {}", e),
    }

    println!();
    Ok(())
}

async fn demonstrate_ip_management(manager: &IpamManager) -> Result<()> {
    println!("--- IP Management Operations ---");

    // List all allocated IPs
    let all_ips = manager.list_subnet_ips(None, "demo-subnet").await?;
    println!("All allocated IPs in subnet:");
    for allocation in &all_ips {
        println!(
            "  {} -> VM {} ({})",
            allocation.ip,
            allocation.vmid.unwrap_or(0),
            allocation
                .hostname
                .as_ref()
                .unwrap_or(&"unknown".to_string())
        );
    }

    // Update an IP allocation
    if let Some(first_allocation) = all_ips.first() {
        let mut updated = first_allocation.clone();
        updated.description = Some("Updated description".to_string());
        updated.hostname = Some("updated-hostname".to_string());

        manager
            .update_ip(None, "demo-subnet", &first_allocation.ip, &updated)
            .await?;
        println!("✓ Updated IP {} allocation", first_allocation.ip);

        // Verify update
        let retrieved = manager
            .get_ip(None, "demo-subnet", &first_allocation.ip)
            .await?;
        if let Some(allocation) = retrieved {
            println!(
                "  New description: {}",
                allocation.description.unwrap_or("none".to_string())
            );
            println!(
                "  New hostname: {}",
                allocation.hostname.unwrap_or("none".to_string())
            );
        }
    }

    // Release all IPs
    for allocation in &all_ips {
        manager
            .release_ip(None, "demo-subnet", &allocation.ip)
            .await?;
        println!("✓ Released IP {}", allocation.ip);
    }

    // Verify all IPs are released
    let remaining_ips = manager.list_subnet_ips(None, "demo-subnet").await?;
    println!("Remaining allocated IPs: {}", remaining_ips.len());

    println!();
    Ok(())
}

fn create_test_subnet(name: &str, cidr: &str) -> Result<Subnet> {
    let subnet_config = SubnetConfig {
        subnet: name.to_string(),
        vnet: "demo-vnet".to_string(),
        subnet_type: SubnetType::Subnet,
        cidr: cidr.parse()?,
        gateway: Some("192.168.100.1".parse()?),
        snat: Some(false),
        dhcp: None,
        options: std::collections::HashMap::new(),
    };

    Ok(Subnet::new(subnet_config))
}

async fn demonstrate_perl_compatibility() -> Result<()> {
    println!("=== Perl Compatibility Features ===\n");

    // Initialize IPAM manager using factory (Perl-compatible way)
    let mut ipam_configs = std::collections::HashMap::new();
    let pve_config = IpamConfig::new("pve".to_string(), IpamType::Pve);
    ipam_configs.insert("pve".to_string(), pve_config);

    let manager = IpamPluginFactory::initialize_manager(&ipam_configs, Some("pve")).await?;
    println!("✓ Initialized IPAM manager using factory pattern");

    // Create subnet for Perl compatibility demo
    let subnet = create_test_subnet("perl-compat", "10.0.0.0/24")?;
    manager.add_subnet(None, &subnet).await?;

    // Demonstrate Perl-style request conversion
    use pve_sdn_drivers::factory::perl_compat;
    use serde_json::Value;

    println!("--- Perl Request Format Conversion ---");

    // Simulate Perl-style request data
    let mut perl_request = std::collections::HashMap::new();
    perl_request.insert("vmid".to_string(), Value::Number(300.into()));
    perl_request.insert(
        "hostname".to_string(),
        Value::String("perl-vm.example.com".to_string()),
    );
    perl_request.insert(
        "desc".to_string(),
        Value::String("VM created from Perl".to_string()),
    );
    perl_request.insert(
        "mac".to_string(),
        Value::String("52:54:00:aa:bb:cc".to_string()),
    );
    perl_request.insert("ip".to_string(), Value::String("10.0.0.100".to_string()));

    println!("Perl request data:");
    for (key, value) in &perl_request {
        println!("  {}: {}", key, value);
    }

    // Convert to Rust format
    let rust_request = perl_compat::convert_perl_request(&perl_request, "perl-compat")?;
    println!("\n✓ Converted to Rust format:");
    println!("  Subnet: {}", rust_request.subnet);
    println!("  VMID: {:?}", rust_request.vmid);
    println!("  Hostname: {:?}", rust_request.hostname);
    println!("  Description: {:?}", rust_request.description);
    println!("  MAC: {:?}", rust_request.mac);
    println!("  Requested IP: {:?}", rust_request.requested_ip);

    // Allocate IP using converted request
    let allocation = manager.allocate_ip(None, &rust_request).await?;
    println!(
        "\n✓ Allocated IP using Perl-compatible request: {}",
        allocation.ip
    );

    // Convert allocation back to Perl format
    println!("\n--- Perl Response Format Conversion ---");
    let perl_response = perl_compat::convert_to_perl_format(&allocation);
    println!("Allocation in Perl-compatible format:");
    for (key, value) in &perl_response {
        println!("  {}: {}", key, value);
    }

    // Demonstrate configuration validation
    println!("\n--- Configuration Validation ---");

    // Test PVE IPAM validation
    let pve_config = IpamConfig::new("pve-test".to_string(), IpamType::Pve);
    match perl_compat::validate_perl_compatibility(&pve_config) {
        Ok(()) => println!("✓ PVE IPAM configuration is Perl-compatible"),
        Err(e) => println!("✗ PVE IPAM validation failed: {}", e),
    }

    // Test phpIPAM validation (should fail without proper config)
    let mut phpipam_config = IpamConfig::new("phpipam-test".to_string(), IpamType::PhpIpam);
    match perl_compat::validate_perl_compatibility(&phpipam_config) {
        Ok(()) => println!("✓ phpIPAM configuration is Perl-compatible"),
        Err(e) => println!("✗ phpIPAM validation failed (expected): {}", e),
    }

    // Fix phpIPAM config and test again
    phpipam_config.url = Some("http://phpipam.example.com".to_string());
    phpipam_config.token = Some("test-token".to_string());
    match perl_compat::validate_perl_compatibility(&phpipam_config) {
        Ok(()) => println!("✓ Fixed phpIPAM configuration is Perl-compatible"),
        Err(e) => println!("✗ Fixed phpIPAM validation failed: {}", e),
    }

    // Test NetBox validation
    let mut netbox_config = IpamConfig::new("netbox-test".to_string(), IpamType::NetBox);
    netbox_config.url = Some("http://netbox.example.com".to_string());
    netbox_config.token = Some("test-token".to_string());
    match perl_compat::validate_perl_compatibility(&netbox_config) {
        Ok(()) => println!("✓ NetBox configuration is Perl-compatible"),
        Err(e) => println!("✗ NetBox validation failed: {}", e),
    }

    // Clean up
    manager
        .release_ip(None, "perl-compat", &allocation.ip)
        .await?;
    manager.remove_subnet(None, "perl-compat").await?;
    println!("\n✓ Cleaned up Perl compatibility demo");

    Ok(())
}
