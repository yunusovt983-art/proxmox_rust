//! Tests for storage integration functionality

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        AccessControlSettings, AuthenticationMethod, CacheMode, DefaultFutureStorageIntegration,
        DefaultStorageNetworkManager, DefaultStoragePathResolver, PerformanceSettings, QosSettings,
        SecuritySettings, StorageBackendConfig, StorageBackendType, StorageNetworkConfig,
        StoragePathConfig, StorageVlanConfig, StorageVlanManager, TimeoutSettings,
    };
    use std::collections::HashMap;
    use std::path::PathBuf;
    use tokio_test;

    #[tokio::test]
    async fn test_storage_network_config_validation() {
        let network_config = Arc::new(MockNetworkConfigManager::new());
        let manager = DefaultStorageNetworkManager::new(network_config);

        let config = StorageNetworkConfig {
            backend_type: StorageBackendType::Nfs {
                server: "192.168.1.100".to_string(),
                export: "/export/data".to_string(),
                version: Some("4".to_string()),
                options: HashMap::new(),
            },
            interface: "eth0".to_string(),
            vlan_tag: Some(100),
            network_options: HashMap::new(),
            qos_settings: Some(QosSettings {
                bandwidth_limit: Some(1000),
                priority: Some(5),
                dscp: Some(46),
            }),
        };

        let result = manager.validate_storage_network(&config).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_storage_network_config_invalid_vlan() {
        let network_config = Arc::new(MockNetworkConfigManager::new());
        let manager = DefaultStorageNetworkManager::new(network_config);

        let config = StorageNetworkConfig {
            backend_type: StorageBackendType::Nfs {
                server: "192.168.1.100".to_string(),
                export: "/export/data".to_string(),
                version: Some("4".to_string()),
                options: HashMap::new(),
            },
            interface: "eth0".to_string(),
            vlan_tag: Some(5000), // Invalid VLAN tag
            network_options: HashMap::new(),
            qos_settings: None,
        };

        let result = manager.validate_storage_network(&config).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_vlan_manager_create_storage_vlan() {
        let mut vlan_manager = StorageVlanManager::new();

        let config = StorageVlanConfig {
            base_interface: "eth0".to_string(),
            vlan_tag: 100,
            subnet: Some("192.168.100.0/24".to_string()),
            gateway: Some("192.168.100.1".to_string()),
            mtu: Some(1500),
            options: HashMap::new(),
        };

        let result = vlan_manager.create_storage_vlan("storage1", &config).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "eth0.100");

        let vlans = vlan_manager.list_storage_vlans();
        assert_eq!(vlans.len(), 1);
        assert_eq!(vlans[0].storage_id, "storage1");
        assert_eq!(vlans[0].vlan_tag, 100);
    }

    #[tokio::test]
    async fn test_vlan_manager_invalid_vlan_tag() {
        let mut vlan_manager = StorageVlanManager::new();

        let config = StorageVlanConfig {
            base_interface: "eth0".to_string(),
            vlan_tag: 5000, // Invalid VLAN tag
            subnet: None,
            gateway: None,
            mtu: None,
            options: HashMap::new(),
        };

        let result = vlan_manager.create_storage_vlan("storage1", &config).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_path_resolver_resolve_path() {
        let mut resolver = DefaultStoragePathResolver::new(PathBuf::from("/mnt"));

        let config = StoragePathConfig {
            storage_id: "nfs1".to_string(),
            backend_type: StorageBackendType::Nfs {
                server: "192.168.1.100".to_string(),
                export: "/export/data".to_string(),
                version: None,
                options: HashMap::new(),
            },
            mount_point: PathBuf::from("/mnt/nfs-nfs1"),
            path_prefix: Some("proxmox".to_string()),
            network_interface: Some("eth0".to_string()),
            options: HashMap::new(),
        };

        resolver.add_storage_config(config);

        let result = resolver.resolve_path("nfs1", "images/vm-100-disk-0.qcow2");
        assert!(result.is_ok());

        let resolved = result.unwrap();
        assert_eq!(
            resolved,
            PathBuf::from("/mnt/nfs-nfs1/proxmox/images/vm-100-disk-0.qcow2")
        );
    }

    #[tokio::test]
    async fn test_path_resolver_path_traversal_protection() {
        let mut resolver = DefaultStoragePathResolver::new(PathBuf::from("/mnt"));

        let config = StoragePathConfig {
            storage_id: "nfs1".to_string(),
            backend_type: StorageBackendType::Nfs {
                server: "192.168.1.100".to_string(),
                export: "/export/data".to_string(),
                version: None,
                options: HashMap::new(),
            },
            mount_point: PathBuf::from("/mnt/nfs-nfs1"),
            path_prefix: None,
            network_interface: None,
            options: HashMap::new(),
        };

        resolver.add_storage_config(config);

        // Test path traversal attempt
        let result = resolver.resolve_path("nfs1", "../../../etc/passwd");
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_future_integration_register_backend() {
        let integration = DefaultFutureStorageIntegration::new();
        integration.initialize().await.unwrap();

        let backend_type = StorageBackendType::Nfs {
            server: "192.168.1.100".to_string(),
            export: "/export/data".to_string(),
            version: Some("4".to_string()),
            options: HashMap::new(),
        };

        let mut network_options = HashMap::new();
        network_options.insert("ip_address".to_string(), "192.168.100.10/24".to_string());
        network_options.insert("gateway".to_string(), "192.168.100.1".to_string());
        network_options.insert("dns_servers".to_string(), "8.8.8.8".to_string());

        let backend_config = StorageBackendConfig {
            storage_id: "nfs1".to_string(),
            backend_type: backend_type.clone(),
            network_config: StorageNetworkConfig {
                backend_type,
                interface: "eth0".to_string(),
                vlan_tag: Some(100),
                network_options,
                qos_settings: None,
            },
            mount_options: HashMap::new(),
            performance_settings: PerformanceSettings {
                cache_mode: CacheMode::WriteThrough,
                io_scheduler: Some("deadline".to_string()),
                read_ahead: Some(128),
                max_concurrent_operations: Some(32),
                timeout_settings: TimeoutSettings {
                    connect_timeout: Some(30),
                    read_timeout: Some(60),
                    write_timeout: Some(60),
                    retry_count: Some(3),
                },
            },
            security_settings: SecuritySettings {
                encryption_enabled: false,
                authentication_method: AuthenticationMethod::None,
                access_control: AccessControlSettings {
                    allowed_hosts: vec!["192.168.1.0/24".to_string()],
                    allowed_users: vec!["root".to_string()],
                    read_only: false,
                    quota_enabled: false,
                    quota_limit: None,
                },
                audit_logging: true,
            },
        };

        let result = integration
            .register_storage_backend("nfs1", backend_config)
            .await;
        assert!(result.is_ok());

        let backends = integration.list_storage_backends().await.unwrap();
        assert_eq!(backends.len(), 1);
        assert_eq!(backends[0].storage_id, "nfs1");
    }

    #[tokio::test]
    async fn test_future_integration_invalid_backend() {
        let integration = DefaultFutureStorageIntegration::new();
        integration.initialize().await.unwrap();

        let backend_type = StorageBackendType::Nfs {
            server: "".to_string(), // Empty server - should fail validation
            export: "/export/data".to_string(),
            version: None,
            options: HashMap::new(),
        };

        let backend_config = StorageBackendConfig {
            storage_id: "nfs1".to_string(),
            backend_type: backend_type.clone(),
            network_config: StorageNetworkConfig {
                backend_type,
                interface: "eth0".to_string(),
                vlan_tag: None,
                network_options: HashMap::new(),
                qos_settings: None,
            },
            mount_options: HashMap::new(),
            performance_settings: PerformanceSettings {
                cache_mode: CacheMode::None,
                io_scheduler: None,
                read_ahead: None,
                max_concurrent_operations: None,
                timeout_settings: TimeoutSettings {
                    connect_timeout: None,
                    read_timeout: None,
                    write_timeout: None,
                    retry_count: None,
                },
            },
            security_settings: SecuritySettings {
                encryption_enabled: false,
                authentication_method: AuthenticationMethod::None,
                access_control: AccessControlSettings {
                    allowed_hosts: Vec::new(),
                    allowed_users: Vec::new(),
                    read_only: false,
                    quota_enabled: false,
                    quota_limit: None,
                },
                audit_logging: false,
            },
        };

        let result = integration
            .register_storage_backend("nfs1", backend_config)
            .await;
        assert!(result.is_err());
    }

    // Mock network configuration manager for testing
    struct MockNetworkConfigManager {
        interfaces: HashMap<String, net_core::Interface>,
    }

    impl MockNetworkConfigManager {
        fn new() -> Self {
            let mut interfaces = HashMap::new();

            // Add a mock eth0 interface
            interfaces.insert(
                "eth0".to_string(),
                pve_network_core::Interface {
                    name: "eth0".to_string(),
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

            Self { interfaces }
        }
    }

    #[async_trait::async_trait]
    impl crate::storage_network::NetworkConfigTrait for MockNetworkConfigManager {
        async fn get_configuration(
            &self,
        ) -> anyhow::Result<pve_network_core::NetworkConfiguration> {
            Ok(pve_network_core::NetworkConfiguration {
                interfaces: self.interfaces.clone(),
                auto_interfaces: vec!["eth0".to_string()],
                hotplug_interfaces: vec![],
                comments: HashMap::new(),
                ordering: vec!["eth0".to_string()],
            })
        }

        async fn set_configuration(
            &self,
            _config: &pve_network_core::NetworkConfiguration,
        ) -> anyhow::Result<()> {
            Ok(())
        }
    }
}
