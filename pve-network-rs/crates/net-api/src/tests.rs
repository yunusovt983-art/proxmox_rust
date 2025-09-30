//! Unit tests for the network API

#[cfg(test)]
mod tests {
    use crate::network::{NetworkGetQuery, NetworkListQuery};
    use crate::NetworkAPI;
    use pve_network_core::NetworkError;

    #[tokio::test]
    async fn test_network_api_creation() {
        let api = NetworkAPI::new();
        // Just verify we can create the API instance without panicking
        assert!(true);
    }

    #[tokio::test]
    async fn test_list_interfaces_with_mock_data() {
        let api = NetworkAPI::new();
        let query = NetworkListQuery {
            interface_type: None,
            enabled: None,
        };

        // This will use the default mock configuration
        let result = api.list_interfaces("test-node", query).await;
        assert!(result.is_ok());

        let interfaces = result.unwrap();
        assert!(!interfaces.is_empty());

        // Should have at least loopback interface
        let has_loopback = interfaces.iter().any(|iface| iface.iface == "lo");
        assert!(has_loopback);
    }

    #[tokio::test]
    async fn test_list_interfaces_with_type_filter() {
        let api = NetworkAPI::new();
        let query = NetworkListQuery {
            interface_type: Some("bridge".to_string()),
            enabled: None,
        };

        let result = api.list_interfaces("test-node", query).await;
        assert!(result.is_ok());

        let interfaces = result.unwrap();
        // All returned interfaces should be bridges
        for interface in interfaces {
            assert_eq!(interface.interface_type, "bridge");
        }
    }

    #[tokio::test]
    async fn test_get_interface() {
        let api = NetworkAPI::new();
        let query = NetworkGetQuery {
            detailed: Some(false),
        };

        let result = api.get_interface("test-node", "lo", query).await;
        assert!(result.is_ok());

        let interface_value = result.unwrap();
        assert!(interface_value.is_object());

        let interface_obj = interface_value.as_object().unwrap();
        assert_eq!(interface_obj.get("iface").unwrap().as_str().unwrap(), "lo");
    }

    #[tokio::test]
    async fn test_get_interface_detailed() {
        let api = NetworkAPI::new();
        let query = NetworkGetQuery {
            detailed: Some(true),
        };

        let result = api.get_interface("test-node", "lo", query).await;
        assert!(result.is_ok());

        let interface_value = result.unwrap();
        assert!(interface_value.is_object());

        let interface_obj = interface_value.as_object().unwrap();
        assert_eq!(interface_obj.get("iface").unwrap().as_str().unwrap(), "lo");
        assert!(interface_obj.contains_key("config"));
        assert!(interface_obj.contains_key("addresses"));
    }

    #[tokio::test]
    async fn test_get_interface_not_found() {
        let api = NetworkAPI::new();
        let query = NetworkGetQuery {
            detailed: Some(false),
        };

        let result = api.get_interface("test-node", "nonexistent", query).await;
        assert!(result.is_err());

        match result.unwrap_err() {
            NetworkError::Api(pve_network_core::error::ApiError::NotFound { resource }) => {
                assert!(resource.contains("nonexistent"));
            }
            _ => panic!("Expected NotFound error"),
        }
    }

    #[tokio::test]
    async fn test_get_interface_status() {
        let api = NetworkAPI::new();

        let result = api.get_interface_status("test-node", "lo").await;
        assert!(result.is_ok());

        let status_value = result.unwrap();
        assert!(status_value.is_object());

        let status_obj = status_value.as_object().unwrap();
        assert_eq!(status_obj.get("iface").unwrap().as_str().unwrap(), "lo");
        assert!(status_obj.contains_key("active"));
        assert!(status_obj.contains_key("link"));
    }

    #[tokio::test]
    async fn test_create_interface() {
        use crate::network::NetworkInterfaceRequest;
        use std::collections::HashMap;

        let api = NetworkAPI::new();
        let request = NetworkInterfaceRequest {
            iface: "test0".to_string(),
            interface_type: "eth".to_string(),
            method: "static".to_string(),
            address: Some("192.168.1.100".to_string()),
            netmask: Some("24".to_string()),
            gateway: Some("192.168.1.1".to_string()),
            mtu: Some(1500),
            autostart: Some(1),
            bridge_ports: None,
            bridge_vlan_aware: None,
            slaves: None,
            bond_mode: None,
            vlan_id: None,
            vlan_raw_device: None,
            options: HashMap::new(),
            comments: None,
        };

        let result = api.create_interface("test-node", request).await;
        assert!(result.is_ok());

        let response = result.unwrap();
        assert!(response.success);
        assert!(response.message.contains("test0"));
        assert!(response.message.contains("created"));
    }

    #[tokio::test]
    async fn test_create_interface_invalid_name() {
        use crate::network::NetworkInterfaceRequest;
        use std::collections::HashMap;

        let api = NetworkAPI::new();
        let request = NetworkInterfaceRequest {
            iface: "".to_string(), // Invalid empty name
            interface_type: "eth".to_string(),
            method: "static".to_string(),
            address: None,
            netmask: None,
            gateway: None,
            mtu: None,
            autostart: None,
            bridge_ports: None,
            bridge_vlan_aware: None,
            slaves: None,
            bond_mode: None,
            vlan_id: None,
            vlan_raw_device: None,
            options: HashMap::new(),
            comments: None,
        };

        let result = api.create_interface("test-node", request).await;
        assert!(result.is_err());

        match result.unwrap_err() {
            NetworkError::Api(pve_network_core::error::ApiError::BadRequest { message }) => {
                assert!(message.contains("empty"));
            }
            _ => panic!("Expected BadRequest error"),
        }
    }

    #[tokio::test]
    async fn test_create_interface_invalid_type() {
        use crate::network::NetworkInterfaceRequest;
        use std::collections::HashMap;

        let api = NetworkAPI::new();
        let request = NetworkInterfaceRequest {
            iface: "test0".to_string(),
            interface_type: "invalid".to_string(), // Invalid type
            method: "static".to_string(),
            address: None,
            netmask: None,
            gateway: None,
            mtu: None,
            autostart: None,
            bridge_ports: None,
            bridge_vlan_aware: None,
            slaves: None,
            bond_mode: None,
            vlan_id: None,
            vlan_raw_device: None,
            options: HashMap::new(),
            comments: None,
        };

        let result = api.create_interface("test-node", request).await;
        assert!(result.is_err());

        match result.unwrap_err() {
            NetworkError::Api(pve_network_core::error::ApiError::BadRequest { message }) => {
                assert!(message.contains("Invalid interface type"));
            }
            _ => panic!("Expected BadRequest error"),
        }
    }

    #[tokio::test]
    async fn test_update_interface_name_mismatch() {
        use crate::network::NetworkInterfaceRequest;
        use std::collections::HashMap;

        let api = NetworkAPI::new();
        let request = NetworkInterfaceRequest {
            iface: "different_name".to_string(), // Different from URL parameter
            interface_type: "eth".to_string(),
            method: "static".to_string(),
            address: None,
            netmask: None,
            gateway: None,
            mtu: None,
            autostart: None,
            bridge_ports: None,
            bridge_vlan_aware: None,
            slaves: None,
            bond_mode: None,
            vlan_id: None,
            vlan_raw_device: None,
            options: HashMap::new(),
            comments: None,
        };

        let result = api.update_interface("test-node", "test0", request).await;
        assert!(result.is_err());

        match result.unwrap_err() {
            NetworkError::Api(pve_network_core::error::ApiError::BadRequest { message }) => {
                assert!(message.contains("must match"));
            }
            _ => panic!("Expected BadRequest error"),
        }
    }

    #[tokio::test]
    async fn test_delete_interface_loopback_protection() {
        let api = NetworkAPI::new();

        let result = api.delete_interface("test-node", "lo").await;
        assert!(result.is_err());

        match result.unwrap_err() {
            NetworkError::Api(pve_network_core::error::ApiError::BadRequest { message }) => {
                assert!(message.contains("loopback"));
            }
            _ => panic!("Expected BadRequest error"),
        }
    }

    #[tokio::test]
    async fn test_reload_network() {
        let api = NetworkAPI::new();

        let result = api.reload_network("test-node").await;
        assert!(result.is_ok());

        let response = result.unwrap();
        assert!(response.success);
        assert!(response.message.contains("reload"));
        assert!(response.task_id.is_some());
    }
}
