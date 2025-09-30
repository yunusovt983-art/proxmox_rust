//! Storage Integration Example
//!
//! This example demonstrates how to use the storage integration functionality
//! to configure network storage backends with VLAN isolation and path resolution.

use anyhow::Result;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;

use storage_integration::{
    AccessControlSettings, AuthenticationMethod, CacheMode, DefaultFutureStorageIntegration,
    DefaultStorageEventHandler, DefaultStorageNetworkManager, DefaultStoragePathResolver,
    PerformanceSettings, QosSettings, SecuritySettings, StorageBackendConfig, StorageBackendType,
    StorageIntegrationBuilder, StorageNetworkConfig, StoragePathConfig, StorageVlanConfig,
    StorageVlanManager, TimeoutSettings,
};

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    env_logger::init();

    println!("Storage Integration Example");
    println!("==========================");

    // Example 1: Basic Storage Network Configuration
    println!("\n1. Basic Storage Network Configuration");
    basic_storage_network_example().await?;

    // Example 2: VLAN Isolation for Storage Networks
    println!("\n2. VLAN Isolation for Storage Networks");
    vlan_isolation_example().await?;

    // Example 3: Storage Path Resolution
    println!("\n3. Storage Path Resolution");
    path_resolution_example().await?;

    // Example 4: Future Integration with Rust pve-storage
    println!("\n4. Future Integration with Rust pve-storage");
    future_integration_example().await?;

    // Example 5: Complete Storage Integration Workflow
    println!("\n5. Complete Storage Integration Workflow");
    complete_workflow_example().await?;

    println!("\nStorage integration examples completed successfully!");
    Ok(())
}

async fn basic_storage_network_example() -> Result<()> {
    println!("Configuring basic storage network...");

    // Create a mock network configuration manager
    let network_config = Arc::new(MockNetworkConfigManager::new());
    let storage_manager = DefaultStorageNetworkManager::new(network_config);

    // Configure NFS storage network
    let nfs_config = StorageNetworkConfig {
        backend_type: StorageBackendType::Nfs {
            server: "192.168.1.100".to_string(),
            export: "/export/proxmox".to_string(),
            version: Some("4".to_string()),
            options: HashMap::new(),
        },
        interface: "eth0".to_string(),
        vlan_tag: Some(100),
        network_options: {
            let mut opts = HashMap::new();
            opts.insert("timeout".to_string(), "30".to_string());
            opts.insert("tcp_window_size".to_string(), "65536".to_string());
            opts
        },
        qos_settings: Some(QosSettings {
            bandwidth_limit: Some(1000), // 1 Gbps
            priority: Some(5),
            dscp: Some(46), // Expedited Forwarding
        }),
    };

    // Configure the storage network
    storage_manager
        .configure_storage_network("nfs-storage", &nfs_config)
        .await?;
    println!("✓ NFS storage network configured");

    // Get storage network status
    let status = storage_manager
        .get_storage_network_status("nfs-storage")
        .await?;
    println!("✓ Storage network status: {:?}", status.is_active);

    // Configure CIFS storage network
    let cifs_config = StorageNetworkConfig {
        backend_type: StorageBackendType::Cifs {
            server: "192.168.1.101".to_string(),
            share: "proxmox".to_string(),
            username: Some("proxmox".to_string()),
            domain: Some("WORKGROUP".to_string()),
            options: HashMap::new(),
        },
        interface: "eth0".to_string(),
        vlan_tag: Some(101),
        network_options: HashMap::new(),
        qos_settings: None,
    };

    storage_manager
        .configure_storage_network("cifs-storage", &cifs_config)
        .await?;
    println!("✓ CIFS storage network configured");

    // List all storage networks
    let networks = storage_manager.list_storage_networks().await?;
    println!("✓ Total storage networks configured: {}", networks.len());

    Ok(())
}

async fn vlan_isolation_example() -> Result<()> {
    println!("Configuring VLAN isolation for storage networks...");

    let mut vlan_manager = StorageVlanManager::new();

    // Create storage VLAN for NFS
    let nfs_vlan_config = StorageVlanConfig {
        base_interface: "eth0".to_string(),
        vlan_tag: 200,
        subnet: Some("192.168.200.0/24".to_string()),
        gateway: Some("192.168.200.1".to_string()),
        mtu: Some(9000), // Jumbo frames for storage
        options: {
            let mut opts = HashMap::new();
            opts.insert("bandwidth_limit".to_string(), "10000".to_string()); // 10 Gbps
            opts.insert("priority".to_string(), "7".to_string());
            opts
        },
    };

    let nfs_vlan_interface = vlan_manager
        .create_storage_vlan("nfs-isolated", &nfs_vlan_config)
        .await?;
    println!("✓ NFS storage VLAN created: {}", nfs_vlan_interface);

    // Create storage VLAN for iSCSI
    let iscsi_vlan_config = StorageVlanConfig {
        base_interface: "eth1".to_string(),
        vlan_tag: 201,
        subnet: Some("192.168.201.0/24".to_string()),
        gateway: Some("192.168.201.1".to_string()),
        mtu: Some(9000),
        options: HashMap::new(),
    };

    let iscsi_vlan_interface = vlan_manager
        .create_storage_vlan("iscsi-isolated", &iscsi_vlan_config)
        .await?;
    println!("✓ iSCSI storage VLAN created: {}", iscsi_vlan_interface);

    // List all storage VLANs
    let vlans = vlan_manager.list_storage_vlans();
    println!("✓ Total storage VLANs: {}", vlans.len());

    for vlan in &vlans {
        println!(
            "  - Storage: {}, VLAN: {}, Interface: {}",
            vlan.storage_id, vlan.vlan_tag, vlan.vlan_interface
        );
    }

    Ok(())
}

async fn path_resolution_example() -> Result<()> {
    println!("Demonstrating storage path resolution...");

    let mut path_resolver = DefaultStoragePathResolver::new(PathBuf::from("/mnt"));

    // Add NFS storage configuration
    let nfs_path_config = StoragePathConfig {
        storage_id: "nfs-storage".to_string(),
        backend_type: StorageBackendType::Nfs {
            server: "192.168.1.100".to_string(),
            export: "/export/proxmox".to_string(),
            version: Some("4".to_string()),
            options: HashMap::new(),
        },
        mount_point: PathBuf::from("/mnt/nfs-storage"),
        path_prefix: Some("images".to_string()),
        network_interface: Some("eth0.100".to_string()),
        options: HashMap::new(),
    };

    path_resolver.add_storage_config(nfs_path_config);

    // Add CIFS storage configuration
    let cifs_path_config = StoragePathConfig {
        storage_id: "cifs-storage".to_string(),
        backend_type: StorageBackendType::Cifs {
            server: "192.168.1.101".to_string(),
            share: "proxmox".to_string(),
            username: Some("proxmox".to_string()),
            domain: Some("WORKGROUP".to_string()),
            options: HashMap::new(),
        },
        mount_point: PathBuf::from("/mnt/cifs-storage"),
        path_prefix: Some("backups".to_string()),
        network_interface: Some("eth0.101".to_string()),
        options: HashMap::new(),
    };

    path_resolver.add_storage_config(cifs_path_config);

    // Resolve paths
    let vm_disk_path = path_resolver.resolve_path("nfs-storage", "vm-100-disk-0.qcow2")?;
    println!("✓ VM disk path resolved: {:?}", vm_disk_path);

    let backup_path = path_resolver.resolve_path(
        "cifs-storage",
        "vzdump-qemu-100-2024_01_15-10_30_00.vma.zst",
    )?;
    println!("✓ Backup path resolved: {:?}", backup_path);

    // Get mount points
    let nfs_mount = path_resolver.get_mount_point("nfs-storage")?;
    let cifs_mount = path_resolver.get_mount_point("cifs-storage")?;
    println!("✓ NFS mount point: {:?}", nfs_mount);
    println!("✓ CIFS mount point: {:?}", cifs_mount);

    Ok(())
}

async fn future_integration_example() -> Result<()> {
    println!("Demonstrating future integration with Rust pve-storage...");

    // Build storage integration with event handler
    let integration = StorageIntegrationBuilder::new()
        .with_event_handler(Box::new(DefaultStorageEventHandler))
        .await
        .build()
        .await?;

    // Register NFS backend
    let backend_type = StorageBackendType::Nfs {
        server: "192.168.1.200".to_string(),
        export: "/export/future".to_string(),
        version: Some("4.2".to_string()),
        options: HashMap::new(),
    };

    let mut network_options = HashMap::new();
    network_options.insert("ip_address".to_string(), "192.168.300.10/24".to_string());
    network_options.insert("gateway".to_string(), "192.168.300.1".to_string());
    network_options.insert("dns_servers".to_string(), "8.8.8.8,8.8.4.4".to_string());
    network_options.insert("mtu".to_string(), "9000".to_string());
    network_options.insert("bandwidth_limit".to_string(), "10000".to_string());

    let nfs_backend = StorageBackendConfig {
        storage_id: "future-nfs".to_string(),
        backend_type: backend_type.clone(),
        network_config: StorageNetworkConfig {
            backend_type,
            interface: "eth0".to_string(),
            vlan_tag: Some(300),
            network_options,
            qos_settings: None,
        },
        mount_options: {
            let mut opts = HashMap::new();
            opts.insert("rsize".to_string(), "1048576".to_string());
            opts.insert("wsize".to_string(), "1048576".to_string());
            opts.insert("timeo".to_string(), "600".to_string());
            opts
        },
        performance_settings: PerformanceSettings {
            cache_mode: CacheMode::WriteBack,
            io_scheduler: Some("mq-deadline".to_string()),
            read_ahead: Some(256),
            max_concurrent_operations: Some(64),
            timeout_settings: TimeoutSettings {
                connect_timeout: Some(30),
                read_timeout: Some(120),
                write_timeout: Some(120),
                retry_count: Some(5),
            },
        },
        security_settings: SecuritySettings {
            encryption_enabled: true,
            authentication_method: AuthenticationMethod::Certificate {
                cert_path: "/etc/ssl/certs/storage.crt".to_string(),
                key_path: "/etc/ssl/private/storage.key".to_string(),
            },
            access_control: AccessControlSettings {
                allowed_hosts: vec!["192.168.0.0/16".to_string()],
                allowed_users: vec!["root".to_string(), "proxmox".to_string()],
                read_only: false,
                quota_enabled: true,
                quota_limit: Some(1024 * 1024 * 1024 * 1024), // 1TB
            },
            audit_logging: true,
        },
    };

    integration
        .register_storage_backend("future-nfs", nfs_backend)
        .await?;
    println!("✓ Future NFS backend registered");

    // List all backends
    let backends = integration.list_storage_backends().await?;
    println!("✓ Total storage backends: {}", backends.len());

    // Get backend status
    let status = integration.get_storage_network_status("future-nfs").await?;
    println!("✓ Backend network status: active={}", status.is_active);

    Ok(())
}

async fn complete_workflow_example() -> Result<()> {
    println!("Demonstrating complete storage integration workflow...");

    // 1. Initialize all components
    let network_config = Arc::new(MockNetworkConfigManager::new());
    let storage_manager = Arc::new(DefaultStorageNetworkManager::new(network_config));
    let vlan_manager = Arc::new(RwLock::new(StorageVlanManager::new()));
    let path_resolver = Arc::new(DefaultStoragePathResolver::new(PathBuf::from("/mnt")));
    let future_integration = Arc::new(DefaultFutureStorageIntegration::new());

    // Initialize future integration
    future_integration.initialize().await?;

    // 2. Configure storage network with VLAN isolation
    let storage_config = StorageNetworkConfig {
        backend_type: StorageBackendType::Iscsi {
            portal: "192.168.1.150:3260".to_string(),
            target: "iqn.2024-01.com.example:storage.target1".to_string(),
            lun: Some(0),
            options: HashMap::new(),
        },
        interface: "eth0".to_string(),
        vlan_tag: Some(400),
        network_options: {
            let mut opts = HashMap::new();
            opts.insert("timeout".to_string(), "60".to_string());
            opts.insert("retry_count".to_string(), "5".to_string());
            opts
        },
        qos_settings: Some(QosSettings {
            bandwidth_limit: Some(8000), // 8 Gbps
            priority: Some(6),
            dscp: Some(34), // Assured Forwarding
        }),
    };

    storage_manager
        .configure_storage_network("iscsi-workflow", &storage_config)
        .await?;
    println!("✓ iSCSI storage network configured");

    // 3. Create dedicated VLAN for storage
    let vlan_config = StorageVlanConfig {
        base_interface: "eth0".to_string(),
        vlan_tag: 400,
        subnet: Some("192.168.400.0/24".to_string()),
        gateway: Some("192.168.400.1".to_string()),
        mtu: Some(9000),
        options: {
            let mut opts = HashMap::new();
            opts.insert("bandwidth_limit".to_string(), "8000".to_string());
            opts.insert("priority".to_string(), "6".to_string());
            opts
        },
    };

    let mut vlan_mgr = vlan_manager.write().await;
    let vlan_interface = vlan_mgr
        .create_storage_vlan("iscsi-workflow", &vlan_config)
        .await?;
    drop(vlan_mgr);
    println!("✓ Storage VLAN created: {}", vlan_interface);

    // 4. Configure path resolution
    let mut path_res = DefaultStoragePathResolver::new(PathBuf::from("/mnt"));
    let path_config = StoragePathConfig {
        storage_id: "iscsi-workflow".to_string(),
        backend_type: storage_config.backend_type.clone(),
        mount_point: PathBuf::from("/mnt/iscsi-workflow"),
        path_prefix: Some("volumes".to_string()),
        network_interface: Some(vlan_interface.clone()),
        options: HashMap::new(),
    };

    path_res.add_storage_config(path_config);
    println!("✓ Path resolution configured");

    // 5. Register with future integration
    let mut future_network_options = HashMap::new();
    future_network_options.insert("ip_address".to_string(), "192.168.400.10/24".to_string());
    future_network_options.insert("gateway".to_string(), "192.168.400.1".to_string());
    future_network_options.insert("dns_servers".to_string(), "192.168.400.1".to_string());
    future_network_options.insert("mtu".to_string(), "9000".to_string());
    future_network_options.insert("bandwidth_limit".to_string(), "8000".to_string());

    let future_backend = StorageBackendConfig {
        storage_id: "iscsi-workflow".to_string(),
        backend_type: storage_config.backend_type.clone(),
        network_config: StorageNetworkConfig {
            backend_type: storage_config.backend_type.clone(),
            interface: vlan_interface,
            vlan_tag: Some(400),
            network_options: future_network_options,
            qos_settings: None,
        },
        mount_options: HashMap::new(),
        performance_settings: PerformanceSettings {
            cache_mode: CacheMode::DirectSync,
            io_scheduler: Some("none".to_string()),
            read_ahead: Some(0), // Disable for block storage
            max_concurrent_operations: Some(128),
            timeout_settings: TimeoutSettings {
                connect_timeout: Some(60),
                read_timeout: Some(300),
                write_timeout: Some(300),
                retry_count: Some(5),
            },
        },
        security_settings: SecuritySettings {
            encryption_enabled: false, // iSCSI encryption handled at protocol level
            authentication_method: AuthenticationMethod::Password {
                username: "proxmox".to_string(),
                password_hash: "hashed_password".to_string(),
            },
            access_control: AccessControlSettings {
                allowed_hosts: vec!["192.168.400.0/24".to_string()],
                allowed_users: vec!["proxmox".to_string()],
                read_only: false,
                quota_enabled: false,
                quota_limit: None,
            },
            audit_logging: true,
        },
    };

    future_integration
        .register_storage_backend("iscsi-workflow", future_backend)
        .await?;
    println!("✓ Future integration registered");

    // 6. Verify complete setup
    let network_status = storage_manager
        .get_storage_network_status("iscsi-workflow")
        .await?;
    let future_status = future_integration
        .get_storage_network_status("iscsi-workflow")
        .await?;
    let resolved_path = path_res.resolve_path("iscsi-workflow", "vm-200-disk-0.raw")?;

    println!("✓ Complete workflow verification:");
    println!("  - Network active: {}", network_status.is_active);
    println!("  - Future integration active: {}", future_status.is_active);
    println!("  - Resolved path: {:?}", resolved_path);

    println!("✓ Complete storage integration workflow successful!");

    Ok(())
}

// Mock network configuration manager for examples
struct MockNetworkConfigManager {
    interfaces: HashMap<String, net_core::Interface>,
}

impl MockNetworkConfigManager {
    fn new() -> Self {
        let mut interfaces = HashMap::new();

        // Add mock interfaces
        for iface in ["eth0", "eth1", "eth2"] {
            interfaces.insert(
                iface.to_string(),
                pve_network_core::Interface {
                    name: iface.to_string(),
                    iface_type: pve_network_core::InterfaceType::Physical,
                    method: pve_network_core::AddressMethod::Static,
                    addresses: vec![],
                    gateway: None,
                    mtu: Some(1500),
                    options: HashMap::new(),
                    enabled: true,
                    comments: Vec::new(),
                },
            );
        }

        Self { interfaces }
    }
}

#[async_trait::async_trait]
impl storage_integration::storage_network::NetworkConfigTrait for MockNetworkConfigManager {
    async fn get_configuration(&self) -> anyhow::Result<pve_network_core::NetworkConfiguration> {
        Ok(pve_network_core::NetworkConfiguration {
            interfaces: self.interfaces.clone(),
            auto_interfaces: vec!["eth0".to_string(), "eth1".to_string()],
            hotplug_interfaces: vec![],
            comments: HashMap::new(),
            ordering: vec!["eth0".to_string(), "eth1".to_string()],
        })
    }

    async fn set_configuration(
        &self,
        _config: &pve_network_core::NetworkConfiguration,
    ) -> anyhow::Result<()> {
        Ok(())
    }
}
