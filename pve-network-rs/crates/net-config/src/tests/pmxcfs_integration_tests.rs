//! Integration tests for pmxcfs cluster functionality

use std::sync::Arc;
use std::time::Duration;
use tempfile::TempDir;
use tokio::time::timeout;

use crate::network_config::{
    AddressMethod, InterfaceConfig, InterfaceType, NetworkConfigManager, NetworkConfiguration,
};
use crate::pmxcfs::PmxcfsConfig;
use crate::sdn_config::{SdnConfigManager, ZoneConfig, ZoneType};

/// Test cluster locking functionality
#[tokio::test]
async fn test_cluster_lock_basic_functionality() {
    let temp_dir = TempDir::new().unwrap();
    let pmxcfs = PmxcfsConfig::with_base_path(temp_dir.path()).unwrap();

    // Test acquiring a lock
    let lock = pmxcfs.acquire_lock("test_lock", "test_operation").await;
    assert!(lock.is_ok());

    let lock = lock.unwrap();
    assert_eq!(lock.lock_info().operation, "test_operation");
    assert!(!lock.lock_info().node.is_empty());

    // Lock should be released when dropped
    drop(lock);
}

/// Test concurrent lock acquisition
#[tokio::test]
async fn test_concurrent_lock_acquisition() {
    let temp_dir = TempDir::new().unwrap();
    let pmxcfs1 = Arc::new(PmxcfsConfig::with_base_path(temp_dir.path()).unwrap());
    let pmxcfs2 = pmxcfs1.clone();

    // First task acquires lock
    let pmxcfs1_clone = pmxcfs1.clone();
    let task1 = tokio::spawn(async move {
        let _lock = pmxcfs1_clone
            .acquire_lock("concurrent_test", "operation1")
            .await
            .unwrap();
        tokio::time::sleep(Duration::from_millis(200)).await;
        "task1_completed"
    });

    // Give first task time to acquire lock
    tokio::time::sleep(Duration::from_millis(50)).await;

    // Second task should wait for lock
    let task2 = tokio::spawn(async move {
        let _lock = pmxcfs2
            .acquire_lock("concurrent_test", "operation2")
            .await
            .unwrap();
        "task2_completed"
    });

    // Both tasks should complete, but task2 should wait
    let results = tokio::join!(task1, task2);
    assert_eq!(results.0.unwrap(), "task1_completed");
    assert_eq!(results.1.unwrap(), "task2_completed");
}

/// Test lock timeout functionality
#[tokio::test]
async fn test_lock_timeout() {
    let temp_dir = TempDir::new().unwrap();
    let pmxcfs = PmxcfsConfig::with_base_path(temp_dir.path()).unwrap();

    // Acquire a lock and hold it
    let _lock = pmxcfs
        .acquire_lock("timeout_test", "long_operation")
        .await
        .unwrap();

    // Try to acquire the same lock with a short timeout
    let result = timeout(
        Duration::from_millis(100),
        pmxcfs.acquire_lock("timeout_test", "short_operation"),
    )
    .await;

    // Should timeout
    assert!(result.is_err());
}

/// Test with_lock convenience method
#[tokio::test]
async fn test_with_lock_method() {
    let temp_dir = TempDir::new().unwrap();
    let pmxcfs = PmxcfsConfig::with_base_path(temp_dir.path()).unwrap();

    let result = pmxcfs
        .with_lock("with_lock_test", "test_operation", || {
            Ok("operation_result".to_string())
        })
        .await;

    assert!(result.is_ok());
    assert_eq!(result.unwrap(), "operation_result");
}

/// Test SDN configuration read/write with locking
#[tokio::test]
async fn test_sdn_config_with_locking() {
    let temp_dir = TempDir::new().unwrap();
    let pmxcfs = PmxcfsConfig::with_base_path(temp_dir.path()).unwrap();
    let sdn_manager = SdnConfigManager::with_pmxcfs(pmxcfs);

    // Create test zone configuration
    let zone_config = ZoneConfig {
        zone_type: ZoneType::Vlan,
        bridge: Some("vmbr0".to_string()),
        vlan_aware: Some(true),
        tag: Some(100),
        vxlan_port: None,
        peers: None,
        mtu: Some(1500),
        nodes: None,
        options: std::collections::HashMap::new(),
    };

    // Test zone update with locking
    let result = sdn_manager
        .update_zone("test_zone", zone_config.clone())
        .await;
    assert!(result.is_ok());

    // Verify zone was written
    let config = sdn_manager.read_config().await.unwrap();
    assert!(config.zones.contains_key("test_zone"));

    let stored_zone = &config.zones["test_zone"];
    assert!(matches!(stored_zone.zone_type, ZoneType::Vlan));
    assert_eq!(stored_zone.bridge, Some("vmbr0".to_string()));
    assert_eq!(stored_zone.tag, Some(100));
}

/// Test network configuration with locking
#[tokio::test]
async fn test_network_config_with_locking() {
    let temp_dir = TempDir::new().unwrap();
    let pmxcfs = PmxcfsConfig::with_base_path(temp_dir.path()).unwrap();
    let network_manager = NetworkConfigManager::with_pmxcfs(pmxcfs);

    // Create test interface configuration
    let interface_config = InterfaceConfig {
        name: "eth0".to_string(),
        iface_type: InterfaceType::Physical,
        method: AddressMethod::Static,
        addresses: vec!["192.168.1.10/24".parse().unwrap()],
        gateway: Some("192.168.1.1".parse().unwrap()),
        mtu: Some(1500),
        options: std::collections::HashMap::new(),
        enabled: true,
        comments: Vec::new(),
    };

    // Create initial empty configuration first
    let initial_config = NetworkConfiguration::default();
    let result = network_manager
        .write_node_config("test_node", &initial_config)
        .await;
    assert!(result.is_ok());

    // Test interface update with locking
    let result = network_manager
        .update_interface("test_node", "eth0", interface_config.clone())
        .await;
    if let Err(e) = &result {
        println!("Error updating interface: {:?}", e);
    }
    assert!(result.is_ok());

    // Verify interface was written
    let config = network_manager.read_node_config("test_node").await.unwrap();
    assert!(config.interfaces.contains_key("eth0"));

    let stored_interface = &config.interfaces["eth0"];
    assert_eq!(stored_interface.name, "eth0");
    assert!(matches!(
        stored_interface.iface_type,
        InterfaceType::Physical
    ));
    assert_eq!(
        stored_interface.addresses,
        vec!["192.168.1.10/24".parse().unwrap()]
    );
}

/// Test concurrent SDN configuration modifications
#[tokio::test]
async fn test_concurrent_sdn_modifications() {
    let temp_dir = TempDir::new().unwrap();
    let pmxcfs = Arc::new(PmxcfsConfig::with_base_path(temp_dir.path()).unwrap());

    let manager1 = SdnConfigManager::with_pmxcfs((*pmxcfs).clone());
    let manager2 = SdnConfigManager::with_pmxcfs((*pmxcfs).clone());

    // Create different zone configurations
    let zone_config1 = ZoneConfig {
        zone_type: ZoneType::Simple,
        bridge: Some("vmbr0".to_string()),
        vlan_aware: None,
        tag: None,
        vxlan_port: None,
        peers: None,
        mtu: None,
        nodes: None,
        options: std::collections::HashMap::new(),
    };

    let zone_config2 = ZoneConfig {
        zone_type: ZoneType::Vlan,
        bridge: Some("vmbr1".to_string()),
        vlan_aware: Some(true),
        tag: Some(200),
        vxlan_port: None,
        peers: None,
        mtu: None,
        nodes: None,
        options: std::collections::HashMap::new(),
    };

    // Run concurrent modifications
    let task1 = tokio::spawn(async move { manager1.update_zone("zone1", zone_config1).await });

    let task2 = tokio::spawn(async move { manager2.update_zone("zone2", zone_config2).await });

    let results = tokio::join!(task1, task2);
    assert!(results.0.unwrap().is_ok());
    assert!(results.1.unwrap().is_ok());

    // Verify both zones were created
    let final_manager = SdnConfigManager::with_pmxcfs((*pmxcfs).clone());
    let config = final_manager.read_config().await.unwrap();
    assert!(config.zones.contains_key("zone1"));
    assert!(config.zones.contains_key("zone2"));
}

/// Test concurrent network configuration modifications
#[tokio::test]
async fn test_concurrent_network_modifications() {
    let temp_dir = TempDir::new().unwrap();
    let pmxcfs = Arc::new(PmxcfsConfig::with_base_path(temp_dir.path()).unwrap());

    let manager1 = NetworkConfigManager::with_pmxcfs((*pmxcfs).clone());
    let manager2 = NetworkConfigManager::with_pmxcfs((*pmxcfs).clone());

    // Create different interface configurations
    let interface_config1 = InterfaceConfig {
        name: "eth0".to_string(),
        iface_type: InterfaceType::Physical,
        method: AddressMethod::Static,
        addresses: vec!["192.168.1.10/24".parse().unwrap()],
        gateway: Some("192.168.1.1".parse().unwrap()),
        mtu: Some(1500),
        options: std::collections::HashMap::new(),
        enabled: true,
        comments: Vec::new(),
    };

    let interface_config2 = InterfaceConfig {
        name: "eth1".to_string(),
        iface_type: InterfaceType::Physical,
        method: AddressMethod::Dhcp,
        addresses: Vec::new(),
        gateway: None,
        mtu: Some(1500),
        options: std::collections::HashMap::new(),
        enabled: true,
        comments: Vec::new(),
    };

    // Run concurrent modifications on the same node
    let task1 = tokio::spawn(async move {
        manager1
            .update_interface("test_node", "eth0", interface_config1)
            .await
    });

    let task2 = tokio::spawn(async move {
        manager2
            .update_interface("test_node", "eth1", interface_config2)
            .await
    });

    let results = tokio::join!(task1, task2);
    assert!(results.0.unwrap().is_ok());
    assert!(results.1.unwrap().is_ok());

    // Verify both interfaces were created
    let final_manager = NetworkConfigManager::with_pmxcfs((*pmxcfs).clone());
    let config = final_manager.read_node_config("test_node").await.unwrap();
    assert!(config.interfaces.contains_key("eth0"));
    assert!(config.interfaces.contains_key("eth1"));
}

/// Test rollback functionality
#[tokio::test]
async fn test_configuration_rollback() {
    let temp_dir = TempDir::new().unwrap();
    let pmxcfs = PmxcfsConfig::with_base_path(temp_dir.path()).unwrap();
    let network_manager = NetworkConfigManager::with_pmxcfs(pmxcfs);

    // Create initial configuration
    let initial_config = NetworkConfiguration {
        interfaces: std::collections::HashMap::new(),
        auto_interfaces: vec!["lo".to_string()],
        hotplug_interfaces: vec![],
        comments: std::collections::HashMap::new(),
        ordering: vec!["lo".to_string()],
    };

    let result = network_manager
        .write_node_config("test_node", &initial_config)
        .await;
    assert!(result.is_ok());

    // Create new configuration with interface
    let mut new_config = initial_config.clone();
    let interface_config = InterfaceConfig {
        name: "eth0".to_string(),
        iface_type: InterfaceType::Physical,
        method: AddressMethod::Static,
        addresses: vec!["192.168.1.10/24".parse().unwrap()],
        gateway: Some("192.168.1.1".parse().unwrap()),
        mtu: Some(1500),
        options: std::collections::HashMap::new(),
        enabled: true,
        comments: Vec::new(),
    };
    new_config
        .interfaces
        .insert("eth0".to_string(), interface_config);

    // Apply configuration with rollback support
    let result = network_manager
        .apply_config_with_rollback("test_node", &new_config)
        .await;
    assert!(result.is_ok());

    // Verify new configuration was applied
    let applied_config = network_manager.read_node_config("test_node").await.unwrap();
    assert!(applied_config.interfaces.contains_key("eth0"));
}

/// Test cluster synchronization verification
#[tokio::test]
async fn test_cluster_sync_verification() {
    let temp_dir = TempDir::new().unwrap();
    let pmxcfs = PmxcfsConfig::with_base_path(temp_dir.path()).unwrap();
    let network_manager = NetworkConfigManager::with_pmxcfs(pmxcfs);

    // Test sync verification
    let result = network_manager.verify_cluster_sync("test_node").await;
    assert!(result.is_ok());
    assert!(result.unwrap()); // Should return true for single node
}

/// Test cluster nodes discovery
#[tokio::test]
async fn test_cluster_nodes_discovery() {
    let temp_dir = TempDir::new().unwrap();
    let pmxcfs = PmxcfsConfig::with_base_path(temp_dir.path()).unwrap();
    let network_manager = NetworkConfigManager::with_pmxcfs(pmxcfs);

    // Create some node directories
    let nodes_dir = temp_dir.path().join("nodes");
    tokio::fs::create_dir_all(&nodes_dir).await.unwrap();
    tokio::fs::create_dir_all(nodes_dir.join("node1"))
        .await
        .unwrap();
    tokio::fs::create_dir_all(nodes_dir.join("node2"))
        .await
        .unwrap();

    let nodes = network_manager.get_cluster_nodes().await.unwrap();
    assert!(nodes.contains(&"node1".to_string()));
    assert!(nodes.contains(&"node2".to_string()));
}

/// Test configuration synchronization across cluster
#[tokio::test]
async fn test_config_sync_across_cluster() {
    let temp_dir = TempDir::new().unwrap();
    let pmxcfs = PmxcfsConfig::with_base_path(temp_dir.path()).unwrap();
    let network_manager = NetworkConfigManager::with_pmxcfs(pmxcfs);

    // Create node directories
    let nodes_dir = temp_dir.path().join("nodes");
    tokio::fs::create_dir_all(&nodes_dir).await.unwrap();
    tokio::fs::create_dir_all(nodes_dir.join("source_node"))
        .await
        .unwrap();
    tokio::fs::create_dir_all(nodes_dir.join("target_node"))
        .await
        .unwrap();

    // Create configuration on source node
    let interface_config = InterfaceConfig {
        name: "eth0".to_string(),
        iface_type: InterfaceType::Physical,
        method: AddressMethod::Static,
        addresses: vec!["192.168.1.10/24".parse().unwrap()],
        gateway: Some("192.168.1.1".parse().unwrap()),
        mtu: Some(1500),
        options: std::collections::HashMap::new(),
        enabled: true,
        comments: Vec::new(),
    };

    let result = network_manager
        .update_interface("source_node", "eth0", interface_config)
        .await;
    assert!(result.is_ok());

    // Sync configuration to target node
    let target_nodes = vec!["target_node".to_string()];
    let result = network_manager
        .sync_config_to_cluster("source_node", &target_nodes)
        .await;
    assert!(result.is_ok());

    // Verify configuration was synced
    let target_config = network_manager
        .read_node_config("target_node")
        .await
        .unwrap();
    assert!(target_config.interfaces.contains_key("eth0"));
}

/// Test handling of stale locks
#[tokio::test]
async fn test_stale_lock_handling() {
    let temp_dir = TempDir::new().unwrap();
    let pmxcfs = PmxcfsConfig::with_base_path(temp_dir.path()).unwrap();

    // Create a lock file manually to simulate a stale lock
    let lock_dir = temp_dir.path().join(".locks");
    tokio::fs::create_dir_all(&lock_dir).await.unwrap();

    let stale_lock_info = crate::pmxcfs::LockInfo {
        node: "old_node".to_string(),
        pid: 99999,   // Non-existent PID
        timestamp: 0, // Very old timestamp
        operation: "stale_operation".to_string(),
    };

    let lock_content = serde_json::to_string(&stale_lock_info).unwrap();
    tokio::fs::write(lock_dir.join("stale_test.lock"), lock_content)
        .await
        .unwrap();

    // Should be able to acquire the lock despite the stale lock file
    let result = pmxcfs.acquire_lock("stale_test", "new_operation").await;
    assert!(result.is_ok());
}
