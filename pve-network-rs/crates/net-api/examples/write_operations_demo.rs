//! Demonstration of basic write operations for network interfaces
//!
//! This example shows how to use the network API to create, update, and delete interfaces.

use pve_network_api::network::{NetworkAPI, NetworkInterfaceRequest};
use serde_json::Value;
use std::collections::HashMap;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize the network API
    let api = NetworkAPI::new();

    println!("=== Network Interface Write Operations Demo ===\n");

    // Example 1: Create a simple static interface
    println!("1. Creating a static interface 'test0'...");
    let create_request = NetworkInterfaceRequest {
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
        comments: Some("Test interface created by demo".to_string()),
    };

    match api.create_interface("demo-node", create_request).await {
        Ok(response) => {
            println!("✓ Interface created successfully: {}", response.message);
        }
        Err(e) => {
            println!("✗ Failed to create interface: {}", e);
        }
    }

    // Example 2: Update the interface
    println!("\n2. Updating interface 'test0' with new IP address...");
    let update_request = NetworkInterfaceRequest {
        iface: "test0".to_string(),
        interface_type: "eth".to_string(),
        method: "static".to_string(),
        address: Some("192.168.1.200".to_string()),
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
        comments: Some("Test interface updated by demo".to_string()),
    };

    match api
        .update_interface("demo-node", "test0", update_request)
        .await
    {
        Ok(response) => {
            println!("✓ Interface updated successfully: {}", response.message);
        }
        Err(e) => {
            println!("✗ Failed to update interface: {}", e);
        }
    }

    // Example 3: Create a bridge interface
    println!("\n3. Creating a bridge interface 'vmbr0'...");
    let mut bridge_options = HashMap::new();
    bridge_options.insert("bridge-stp".to_string(), Value::String("off".to_string()));
    bridge_options.insert("bridge-fd".to_string(), Value::String("0".to_string()));

    let bridge_request = NetworkInterfaceRequest {
        iface: "vmbr0".to_string(),
        interface_type: "bridge".to_string(),
        method: "static".to_string(),
        address: Some("192.168.2.1".to_string()),
        netmask: Some("24".to_string()),
        gateway: None,
        mtu: Some(1500),
        autostart: Some(1),
        bridge_ports: Some("eth1".to_string()),
        bridge_vlan_aware: Some(1),
        slaves: None,
        bond_mode: None,
        vlan_id: None,
        vlan_raw_device: None,
        options: bridge_options,
        comments: Some("Bridge interface for VMs".to_string()),
    };

    match api.create_interface("demo-node", bridge_request).await {
        Ok(response) => {
            println!(
                "✓ Bridge interface created successfully: {}",
                response.message
            );
        }
        Err(e) => {
            println!("✗ Failed to create bridge interface: {}", e);
        }
    }

    // Example 4: Create a VLAN interface
    println!("\n4. Creating a VLAN interface 'vmbr0.100'...");
    let vlan_request = NetworkInterfaceRequest {
        iface: "vmbr0.100".to_string(),
        interface_type: "vlan".to_string(),
        method: "static".to_string(),
        address: Some("192.168.100.1".to_string()),
        netmask: Some("24".to_string()),
        gateway: None,
        mtu: Some(1500),
        autostart: Some(1),
        bridge_ports: None,
        bridge_vlan_aware: None,
        slaves: None,
        bond_mode: None,
        vlan_id: Some(100),
        vlan_raw_device: Some("vmbr0".to_string()),
        options: HashMap::new(),
        comments: Some("VLAN 100 interface".to_string()),
    };

    match api.create_interface("demo-node", vlan_request).await {
        Ok(response) => {
            println!(
                "✓ VLAN interface created successfully: {}",
                response.message
            );
        }
        Err(e) => {
            println!("✗ Failed to create VLAN interface: {}", e);
        }
    }

    // Example 5: Try to delete the loopback interface (should fail)
    println!("\n5. Attempting to delete loopback interface (should fail)...");
    match api.delete_interface("demo-node", "lo").await {
        Ok(response) => {
            println!("✗ Unexpected success: {}", response.message);
        }
        Err(e) => {
            println!("✓ Correctly prevented deletion of loopback: {}", e);
        }
    }

    // Example 6: Delete a test interface
    println!("\n6. Deleting test interface 'test0'...");
    match api.delete_interface("demo-node", "test0").await {
        Ok(response) => {
            println!("✓ Interface deleted successfully: {}", response.message);
        }
        Err(e) => {
            println!("✗ Failed to delete interface: {}", e);
        }
    }

    // Example 7: Reload network configuration
    println!("\n7. Reloading network configuration...");
    match api.reload_network("demo-node").await {
        Ok(response) => {
            println!("✓ Network reload initiated: {}", response.message);
            if let Some(task_id) = response.task_id {
                println!("  Task ID: {}", task_id);
            }
        }
        Err(e) => {
            println!("✗ Failed to reload network: {}", e);
        }
    }

    println!("\n=== Demo completed ===");
    println!("\nNote: This demo shows the API functionality. In a real environment,");
    println!("the operations would interact with the actual network configuration files");
    println!("and system interfaces through pmxcfs and ifupdown2.");

    Ok(())
}
