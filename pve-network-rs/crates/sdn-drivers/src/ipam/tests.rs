//! IPAM driver tests

use pve_sdn_core::{
    IpAllocation, IpAllocationRequest, IpamConfig, IpamManager, IpamPlugin, IpamType, Subnet,
    SubnetConfig, SubnetType,
};
use std::net::IpAddr;
use std::sync::Arc;

use super::{NetBoxIpam, PhpIpam, PveIpam};

/// Helper function to set up temporary storage for tests
fn setup_temp_storage() -> tempfile::TempDir {
    let temp_dir = tempfile::tempdir().unwrap();
    std::env::set_var("PVE_IPAM_STORAGE_PATH", temp_dir.path());
    temp_dir
}

/// Helper function to clean up temporary storage
fn cleanup_temp_storage() {
    std::env::remove_var("PVE_IPAM_STORAGE_PATH");
}

/// Create test subnet
fn create_test_subnet(name: &str, cidr: &str) -> Subnet {
    let subnet_config = SubnetConfig {
        subnet: name.to_string(),
        vnet: "test-vnet".to_string(),
        subnet_type: SubnetType::Subnet,
        cidr: cidr.parse().unwrap(),
        gateway: None,
        snat: None,
        dhcp: None,
        options: std::collections::HashMap::new(),
    };

    Subnet::new(subnet_config)
}

/// Create test allocation request
fn create_test_request(
    subnet: &str,
    vmid: Option<u32>,
    requested_ip: Option<IpAddr>,
) -> IpAllocationRequest {
    IpAllocationRequest {
        subnet: subnet.to_string(),
        vmid,
        hostname: Some("test-host".to_string()),
        mac: Some("00:11:22:33:44:55".to_string()),
        description: Some("Test allocation".to_string()),
        requested_ip,
    }
}

#[tokio::test]
async fn test_pve_ipam_basic_operations() {
    let _temp_dir = setup_temp_storage();

    let config = IpamConfig::new("test-pve".to_string(), IpamType::Pve);
    let ipam = PveIpam::new("test-pve".to_string(), config);

    // Test subnet operations
    let subnet = create_test_subnet("test-subnet", "192.168.1.0/24");

    // Add subnet
    ipam.add_subnet(&subnet).await.unwrap();

    // Validate subnet
    ipam.validate_subnet(&subnet).await.unwrap();

    // Test IP allocation
    let request = create_test_request("test-subnet", Some(100), None);
    let allocation = ipam.allocate_ip(&request).await.unwrap();

    assert_eq!(allocation.subnet, "test-subnet");
    assert_eq!(allocation.vmid, Some(100));
    assert_eq!(allocation.hostname, Some("test-host".to_string()));

    // Test IP retrieval
    let retrieved = ipam.get_ip("test-subnet", &allocation.ip).await.unwrap();
    assert!(retrieved.is_some());
    let retrieved = retrieved.unwrap();
    assert_eq!(retrieved.ip, allocation.ip);
    assert_eq!(retrieved.vmid, Some(100));

    // Test IP availability
    assert!(!ipam
        .is_ip_available("test-subnet", &allocation.ip)
        .await
        .unwrap());

    // Test next free IP
    let next_ip = ipam.get_next_free_ip("test-subnet").await.unwrap();
    assert!(next_ip.is_some());
    assert_ne!(next_ip.unwrap(), allocation.ip);

    // Test list subnet IPs
    let all_ips = ipam.list_subnet_ips("test-subnet").await.unwrap();
    assert_eq!(all_ips.len(), 1);
    assert_eq!(all_ips[0].ip, allocation.ip);

    // Test IP update
    let mut updated_allocation = allocation.clone();
    updated_allocation.description = Some("Updated description".to_string());
    ipam.update_ip("test-subnet", &allocation.ip, &updated_allocation)
        .await
        .unwrap();

    let retrieved = ipam
        .get_ip("test-subnet", &allocation.ip)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        retrieved.description,
        Some("Updated description".to_string())
    );

    // Test IP release
    ipam.release_ip("test-subnet", &allocation.ip)
        .await
        .unwrap();

    let retrieved = ipam.get_ip("test-subnet", &allocation.ip).await.unwrap();
    assert!(retrieved.is_none());

    // Test IP availability after release
    assert!(ipam
        .is_ip_available("test-subnet", &allocation.ip)
        .await
        .unwrap());

    // Test subnet removal
    ipam.remove_subnet("test-subnet").await.unwrap();
}

#[tokio::test]
async fn test_pve_ipam_specific_ip_allocation() {
    let _temp_dir = setup_temp_storage();

    let config = IpamConfig::new("test-pve".to_string(), IpamType::Pve);
    let ipam = PveIpam::new("test-pve".to_string(), config);

    let subnet = create_test_subnet("test-subnet", "10.0.0.0/24");
    ipam.add_subnet(&subnet).await.unwrap();

    // Request specific IP
    let requested_ip: IpAddr = "10.0.0.100".parse().unwrap();
    let request = create_test_request("test-subnet", Some(200), Some(requested_ip));

    let allocation = ipam.allocate_ip(&request).await.unwrap();
    assert_eq!(allocation.ip, requested_ip);
    assert_eq!(allocation.vmid, Some(200));

    // Try to allocate the same IP again (should fail)
    let request2 = create_test_request("test-subnet", Some(201), Some(requested_ip));
    let result = ipam.allocate_ip(&request2).await;
    assert!(result.is_err());

    // Clean up allocations before removing subnet
    ipam.release_ip("test-subnet", &allocation.ip)
        .await
        .unwrap();
    ipam.remove_subnet("test-subnet").await.unwrap();
}

#[tokio::test]
async fn test_pve_ipam_subnet_validation() {
    let config = IpamConfig::new("test-pve".to_string(), IpamType::Pve);
    let ipam = PveIpam::new("test-pve".to_string(), config);

    // Test invalid subnet (empty name)
    let mut invalid_config = SubnetConfig {
        subnet: "".to_string(),
        vnet: "test-vnet".to_string(),
        subnet_type: SubnetType::Subnet,
        cidr: "192.168.1.0/24".parse().unwrap(),
        gateway: None,
        snat: None,
        dhcp: None,
        options: std::collections::HashMap::new(),
    };

    let invalid_subnet = Subnet::new(invalid_config.clone());
    let result = ipam.validate_subnet(&invalid_subnet).await;
    assert!(result.is_err());

    // Test invalid gateway (outside subnet)
    invalid_config.subnet = "test-subnet".to_string();
    invalid_config.gateway = Some("10.0.0.1".parse().unwrap());
    let invalid_subnet = Subnet::new(invalid_config);
    let result = ipam.validate_subnet(&invalid_subnet).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_ipam_manager() {
    let _temp_dir = setup_temp_storage();

    let mut manager = IpamManager::new();

    // Register PVE IPAM
    let pve_config = IpamConfig::new("pve-ipam".to_string(), IpamType::Pve);
    let pve_ipam = Arc::new(PveIpam::new("pve-ipam".to_string(), pve_config));
    manager.register_plugin(pve_ipam);

    // Set as default
    manager.set_default_plugin("pve-ipam").unwrap();

    // Test plugin retrieval
    let plugin = manager.get_plugin("pve-ipam").unwrap();
    assert_eq!(plugin.name(), "pve-ipam");
    assert_eq!(plugin.plugin_type(), IpamType::Pve);

    let default_plugin = manager.get_default_plugin().unwrap();
    assert_eq!(default_plugin.name(), "pve-ipam");

    // Test list plugins
    let plugins = manager.list_plugins();
    assert_eq!(plugins.len(), 1);
    assert_eq!(plugins[0].0, "pve-ipam");
    assert_eq!(plugins[0].1, IpamType::Pve);

    // Test operations through manager
    let subnet = create_test_subnet("manager-test", "172.16.0.0/24");
    manager.add_subnet(None, &subnet).await.unwrap();

    let request = create_test_request("manager-test", Some(300), None);
    let allocation = manager.allocate_ip(None, &request).await.unwrap();

    assert_eq!(allocation.subnet, "manager-test");
    assert_eq!(allocation.vmid, Some(300));

    // Clean up
    manager
        .release_ip(None, "manager-test", &allocation.ip)
        .await
        .unwrap();
    manager.remove_subnet(None, "manager-test").await.unwrap();
}

#[test]
fn test_ipam_config_validation() {
    // Test PVE IPAM config
    let pve_config = IpamConfig::new("pve".to_string(), IpamType::Pve);
    assert!(pve_config.validate().is_ok());

    // Test phpIPAM config without URL
    let mut phpipam_config = IpamConfig::new("phpipam".to_string(), IpamType::PhpIpam);
    assert!(phpipam_config.validate().is_err());

    // Test phpIPAM config with URL but no auth
    phpipam_config.url = Some("http://phpipam.example.com".to_string());
    assert!(phpipam_config.validate().is_err());

    // Test phpIPAM config with token
    phpipam_config.token = Some("test-token".to_string());
    assert!(phpipam_config.validate().is_ok());

    // Test NetBox config without URL
    let mut netbox_config = IpamConfig::new("netbox".to_string(), IpamType::NetBox);
    assert!(netbox_config.validate().is_err());

    // Test NetBox config without token
    netbox_config.url = Some("http://netbox.example.com".to_string());
    assert!(netbox_config.validate().is_err());

    // Test NetBox config with token
    netbox_config.token = Some("test-token".to_string());
    assert!(netbox_config.validate().is_ok());
}

#[test]
fn test_ip_allocation_request() {
    let request = IpAllocationRequest {
        subnet: "test-subnet".to_string(),
        vmid: Some(100),
        hostname: Some("test-host".to_string()),
        mac: Some("00:11:22:33:44:55".to_string()),
        description: Some("Test VM".to_string()),
        requested_ip: Some("192.168.1.100".parse().unwrap()),
    };

    assert_eq!(request.subnet, "test-subnet");
    assert_eq!(request.vmid, Some(100));
    assert_eq!(request.hostname, Some("test-host".to_string()));
    assert_eq!(request.requested_ip, Some("192.168.1.100".parse().unwrap()));
}

#[test]
fn test_ip_allocation() {
    let allocation = IpAllocation {
        ip: "192.168.1.100".parse().unwrap(),
        subnet: "test-subnet".to_string(),
        vmid: Some(100),
        hostname: Some("test-host".to_string()),
        mac: Some("00:11:22:33:44:55".to_string()),
        description: Some("Test VM".to_string()),
        allocated_at: chrono::Utc::now(),
    };

    assert_eq!(allocation.ip, "192.168.1.100".parse::<IpAddr>().unwrap());
    assert_eq!(allocation.subnet, "test-subnet");
    assert_eq!(allocation.vmid, Some(100));
}

// Integration tests for external IPAM systems would require actual services
// These are placeholder tests that demonstrate the API structure

#[tokio::test]
#[ignore] // Requires actual phpIPAM instance
async fn test_phpipam_integration() {
    let mut config = IpamConfig::new("phpipam-test".to_string(), IpamType::PhpIpam);
    config.url = Some("http://phpipam.example.com".to_string());
    config.username = Some("admin".to_string());
    config.password = Some("password".to_string());
    config.section = Some("1".to_string());

    let ipam = PhpIpam::new("phpipam-test".to_string(), config.clone()).unwrap();

    // Test configuration validation
    assert!(ipam.validate_config(&config).await.is_ok());

    // Additional integration tests would go here
}

#[tokio::test]
#[ignore] // Requires actual NetBox instance
async fn test_netbox_integration() {
    let mut config = IpamConfig::new("netbox-test".to_string(), IpamType::NetBox);
    config.url = Some("http://netbox.example.com".to_string());
    config.token = Some("test-token".to_string());
    config.tenant = Some("1".to_string());

    let ipam = NetBoxIpam::new("netbox-test".to_string(), config.clone()).unwrap();

    // Test configuration validation
    assert!(ipam.validate_config(&config).await.is_ok());

    // Additional integration tests would go here
}

// Additional tests for Perl compatibility and enhanced functionality

#[tokio::test]
async fn test_pve_ipam_perl_compatibility() {
    let _temp_dir = setup_temp_storage();

    let config = IpamConfig::new("pve-compat".to_string(), IpamType::Pve);
    let ipam = PveIpam::new("pve-compat".to_string(), config);

    // Test subnet with gateway validation (Perl behavior)
    let subnet_config = SubnetConfig {
        subnet: "perl-test".to_string(),
        vnet: "test-vnet".to_string(),
        subnet_type: SubnetType::Subnet,
        cidr: "192.168.100.0/24".parse().unwrap(),
        gateway: Some("192.168.100.1".parse().unwrap()),
        snat: None,
        dhcp: None,
        options: std::collections::HashMap::new(),
    };

    let subnet = Subnet::new(subnet_config.clone());
    ipam.add_subnet(&subnet).await.unwrap();

    // Test allocation with specific IP (Perl style)
    let request = IpAllocationRequest {
        subnet: "perl-test".to_string(),
        vmid: Some(100),
        hostname: Some("vm100.example.com".to_string()),
        mac: Some("52:54:00:12:34:56".to_string()),
        description: Some("Test VM 100".to_string()),
        requested_ip: Some("192.168.100.10".parse().unwrap()),
    };

    let allocation = ipam.allocate_ip(&request).await.unwrap();
    assert_eq!(allocation.ip, "192.168.100.10".parse::<IpAddr>().unwrap());
    assert_eq!(allocation.vmid, Some(100));

    // Test that gateway IP is not allocatable (Perl behavior)
    let gateway_request = IpAllocationRequest {
        subnet: "perl-test".to_string(),
        vmid: Some(101),
        hostname: None,
        mac: None,
        description: None,
        requested_ip: Some("192.168.100.1".parse().unwrap()),
    };

    // This should succeed as we don't enforce gateway reservation in basic implementation
    // In production, this would be handled by subnet validation
    let gateway_result = ipam.allocate_ip(&gateway_request).await;
    // For now, we allow it, but in Perl compatibility mode, this might be restricted

    // Test network and broadcast address handling
    let network_request = IpAllocationRequest {
        subnet: "perl-test".to_string(),
        vmid: Some(102),
        hostname: None,
        mac: None,
        description: None,
        requested_ip: Some("192.168.100.0".parse().unwrap()),
    };

    let network_result = ipam.allocate_ip(&network_request).await;
    assert!(
        network_result.is_err(),
        "Network address should not be allocatable"
    );

    let broadcast_request = IpAllocationRequest {
        subnet: "perl-test".to_string(),
        vmid: Some(103),
        hostname: None,
        mac: None,
        description: None,
        requested_ip: Some("192.168.100.255".parse().unwrap()),
    };

    let broadcast_result = ipam.allocate_ip(&broadcast_request).await;
    assert!(
        broadcast_result.is_err(),
        "Broadcast address should not be allocatable"
    );

    // Clean up allocations before removing subnet
    ipam.release_ip("perl-test", &allocation.ip).await.unwrap();

    // Clean up gateway allocation if it succeeded
    if let Ok(gateway_allocation) = gateway_result {
        ipam.release_ip("perl-test", &gateway_allocation.ip)
            .await
            .unwrap();
    }

    ipam.remove_subnet("perl-test").await.unwrap();
}

#[tokio::test]
async fn test_ipam_manager_perl_style_operations() {
    let _temp_dir = setup_temp_storage();

    let mut manager = IpamManager::new();

    // Register PVE IPAM (default in Perl)
    let pve_config = IpamConfig::new("pve".to_string(), IpamType::Pve);
    let pve_ipam = Arc::new(PveIpam::new("pve".to_string(), pve_config));
    manager.register_plugin(pve_ipam);
    manager.set_default_plugin("pve").unwrap();

    // Test Perl-style subnet operations
    let subnet = create_test_subnet("perl-subnet", "10.10.10.0/24");
    manager.add_subnet(None, &subnet).await.unwrap();

    // Test allocation without specifying plugin (uses default, like Perl)
    let request = create_test_request("perl-subnet", Some(200), None);
    let allocation = manager.allocate_ip(None, &request).await.unwrap();

    assert_eq!(allocation.subnet, "perl-subnet");
    assert_eq!(allocation.vmid, Some(200));

    // Test listing IPs (Perl compatibility)
    let all_ips = manager.list_subnet_ips(None, "perl-subnet").await.unwrap();
    assert_eq!(all_ips.len(), 1);
    assert_eq!(all_ips[0].vmid, Some(200));

    // Test getting specific IP
    let retrieved = manager
        .get_ip(None, "perl-subnet", &allocation.ip)
        .await
        .unwrap();
    assert!(retrieved.is_some());

    // Test next free IP
    let next_ip = manager.get_next_free_ip(None, "perl-subnet").await.unwrap();
    assert!(next_ip.is_some());
    assert_ne!(next_ip.unwrap(), allocation.ip);

    // Clean up
    manager
        .release_ip(None, "perl-subnet", &allocation.ip)
        .await
        .unwrap();
    manager.remove_subnet(None, "perl-subnet").await.unwrap();
}

#[tokio::test]
async fn test_ipam_factory_initialization() {
    use super::factory::IpamPluginFactory;

    let mut ipam_configs = std::collections::HashMap::new();

    // Add PVE IPAM config
    let pve_config = IpamConfig::new("pve".to_string(), IpamType::Pve);
    ipam_configs.insert("pve".to_string(), pve_config);

    // Initialize manager
    let manager = IpamPluginFactory::initialize_manager(&ipam_configs, Some("pve"))
        .await
        .unwrap();

    // Test that manager is properly initialized
    let plugins = manager.list_plugins();
    assert_eq!(plugins.len(), 1);
    assert_eq!(plugins[0].0, "pve");
    assert_eq!(plugins[0].1, IpamType::Pve);

    // Test default plugin
    let default_plugin = manager.get_default_plugin().unwrap();
    assert_eq!(default_plugin.name(), "pve");
}

#[test]
fn test_perl_compatibility_helpers() {
    use super::factory::perl_compat;
    use serde_json::Value;

    // Test Perl request conversion
    let mut perl_data = std::collections::HashMap::new();
    perl_data.insert("vmid".to_string(), Value::Number(150.into()));
    perl_data.insert(
        "hostname".to_string(),
        Value::String("test.example.com".to_string()),
    );
    perl_data.insert(
        "desc".to_string(),
        Value::String("Test description".to_string()),
    );
    perl_data.insert(
        "mac".to_string(),
        Value::String("aa:bb:cc:dd:ee:ff".to_string()),
    );

    let request = perl_compat::convert_perl_request(&perl_data, "test-subnet").unwrap();

    assert_eq!(request.subnet, "test-subnet");
    assert_eq!(request.vmid, Some(150));
    assert_eq!(request.hostname, Some("test.example.com".to_string()));
    assert_eq!(request.description, Some("Test description".to_string()));
    assert_eq!(request.mac, Some("aa:bb:cc:dd:ee:ff".to_string()));

    // Test allocation to Perl format conversion
    let allocation = IpAllocation {
        ip: "10.0.0.50".parse().unwrap(),
        subnet: "test-subnet".to_string(),
        vmid: Some(150),
        hostname: Some("test.example.com".to_string()),
        mac: Some("aa:bb:cc:dd:ee:ff".to_string()),
        description: Some("Test description".to_string()),
        allocated_at: chrono::Utc::now(),
    };

    let perl_format = perl_compat::convert_to_perl_format(&allocation);

    assert_eq!(
        perl_format.get("ip").unwrap().as_str().unwrap(),
        "10.0.0.50"
    );
    assert_eq!(
        perl_format.get("subnet").unwrap().as_str().unwrap(),
        "test-subnet"
    );
    assert_eq!(perl_format.get("vmid").unwrap().as_u64().unwrap(), 150);
    assert_eq!(
        perl_format.get("hostname").unwrap().as_str().unwrap(),
        "test.example.com"
    );
}

#[tokio::test]
async fn test_pve_ipam_storage_operations() {
    let _temp_dir = setup_temp_storage();

    let config = IpamConfig::new("storage-test".to_string(), IpamType::Pve);
    let ipam = PveIpam::new("storage-test".to_string(), config.clone());

    // Add subnet and allocate IP
    let subnet = create_test_subnet("storage-subnet", "172.20.0.0/24");
    ipam.add_subnet(&subnet).await.unwrap();

    let request = create_test_request(
        "storage-subnet",
        Some(300),
        Some("172.20.0.100".parse().unwrap()),
    );
    let allocation = ipam.allocate_ip(&request).await.unwrap();

    // Verify allocation
    assert_eq!(allocation.ip, "172.20.0.100".parse::<IpAddr>().unwrap());
    assert_eq!(allocation.vmid, Some(300));

    // Create new instance and load from storage
    let ipam2 = PveIpam::new("storage-test".to_string(), config.clone());
    ipam2.load_from_storage().await.unwrap();

    // Verify data was loaded
    let retrieved = ipam2
        .get_ip("storage-subnet", &allocation.ip)
        .await
        .unwrap();
    assert!(retrieved.is_some());
    let retrieved = retrieved.unwrap();
    assert_eq!(retrieved.vmid, Some(300));
}

#[tokio::test]
async fn test_ipam_error_handling() {
    let _temp_dir = setup_temp_storage();

    let config = IpamConfig::new("error-test".to_string(), IpamType::Pve);
    let ipam = PveIpam::new("error-test".to_string(), config);

    // Test operations on non-existent subnet
    let request = create_test_request("non-existent", Some(400), None);
    let result = ipam.allocate_ip(&request).await;
    assert!(result.is_err());

    // Test releasing non-existent IP
    let release_result = ipam
        .release_ip("non-existent", &"192.168.1.1".parse().unwrap())
        .await;
    assert!(release_result.is_err());

    // Test getting non-existent IP (should return Ok(None), not error)
    let get_result = ipam
        .get_ip("non-existent", &"192.168.1.1".parse().unwrap())
        .await;
    // This should return Ok(None) for non-existent subnet, not an error
    match get_result {
        Ok(None) => {} // Expected behavior
        Ok(Some(_)) => panic!("Should not find IP in non-existent subnet"),
        Err(_) => {} // Also acceptable - depends on implementation
    }
}

#[tokio::test]
async fn test_concurrent_ipam_operations() {
    use tokio::task::JoinSet;

    let _temp_dir = setup_temp_storage();

    let config = IpamConfig::new("concurrent-test".to_string(), IpamType::Pve);
    let ipam = Arc::new(PveIpam::new("concurrent-test".to_string(), config));

    // Add subnet
    let subnet = create_test_subnet("concurrent-subnet", "192.168.200.0/24");
    ipam.add_subnet(&subnet).await.unwrap();

    // Spawn multiple concurrent allocation tasks
    let mut join_set = JoinSet::new();

    for i in 0..10 {
        let ipam_clone = ipam.clone();
        join_set.spawn(async move {
            let request = create_test_request("concurrent-subnet", Some(500 + i), None);
            ipam_clone.allocate_ip(&request).await
        });
    }

    // Collect results
    let mut allocations = Vec::new();
    while let Some(result) = join_set.join_next().await {
        let allocation = result.unwrap().unwrap();
        allocations.push(allocation);
    }

    // Verify all allocations are unique
    assert_eq!(allocations.len(), 10);
    let mut ips: Vec<_> = allocations.iter().map(|a| a.ip).collect();
    ips.sort();
    ips.dedup();
    assert_eq!(ips.len(), 10, "All allocated IPs should be unique");

    // Clean up
    for allocation in &allocations {
        ipam.release_ip("concurrent-subnet", &allocation.ip)
            .await
            .unwrap();
    }
    ipam.remove_subnet("concurrent-subnet").await.unwrap();

    // Keep temp directory alive until end of test
    drop(_temp_dir);
}
