//! Container Integration Example
//!
//! This example demonstrates the container integration functionality,
//! including VNet binding, hotplug operations, and pve-container compatibility.

use container_integration::hooks::{
    NetworkEventLogger, RustContainerIntegrationHook, VNetValidationHook,
};
use container_integration::types::{ContainerNetworkConfigExt, ContainerNetworkInterfaceExt};
use container_integration::{
    ContainerIntegration, ContainerNetworkEventType, ContainerNetworkInterface,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    println!("=== Container Integration Example ===\n");

    // Create container integration manager
    let integration = ContainerIntegration::new();

    // Register hooks
    println!("1. Registering network hooks...");
    integration
        .hooks()
        .register_hook("logger".to_string(), NetworkEventLogger::new())
        .await?;

    integration
        .hooks()
        .register_hook("vnet-validation".to_string(), VNetValidationHook::new())
        .await?;

    integration
        .hooks()
        .register_hook(
            "rust-integration".to_string(),
            RustContainerIntegrationHook::new(),
        )
        .await?;

    let hooks = integration.hooks().list_hooks().await?;
    println!("Registered hooks: {:?}\n", hooks);

    // Example 1: VNet binding
    println!("2. VNet Binding Example");
    let container_id = 100;
    let vnet_name = "test-vnet";

    let mut interface = ContainerNetworkInterface::new("net0".to_string());
    interface.vnet = Some(vnet_name.to_string());
    interface.ip = Some("10.0.0.10/24".to_string());
    interface.hwaddr = Some("02:00:00:00:00:01".to_string());

    // Bind VNet to container
    integration
        .vnet_binding()
        .bind_vnet(vnet_name, container_id, &interface)
        .await?;
    println!(
        "✓ Bound VNet '{}' to container {} interface '{}'",
        vnet_name, container_id, interface.name
    );

    // Check binding
    let is_bound = integration
        .vnet_binding()
        .is_interface_bound(container_id, "net0")
        .await?;
    println!("✓ Interface bound: {}", is_bound);

    let bound_vnet = integration
        .vnet_binding()
        .get_interface_vnet(container_id, "net0")
        .await?;
    println!("✓ Bound VNet: {:?}\n", bound_vnet);

    // Example 2: Hotplug operations
    println!("3. Hotplug Operations Example");

    let mut hotplug_interface = ContainerNetworkInterface::new("net1".to_string());
    hotplug_interface.bridge = Some("vmbr0".to_string());
    hotplug_interface.ip = Some("192.168.1.10/24".to_string());
    hotplug_interface.hwaddr = Some("02:00:00:00:00:02".to_string());

    // Hotplug add interface
    let operation_id = integration
        .hotplug()
        .hotplug_add(container_id, hotplug_interface)
        .await?;
    println!("✓ Started hotplug add operation: {}", operation_id);

    // Check operation status
    let status = integration
        .hotplug()
        .get_operation_status(&operation_id)
        .await?;
    if let Some(op) = status {
        println!("✓ Operation status: {:?}", op.status);
    }

    // List container operations
    let operations = integration
        .hotplug()
        .list_container_operations(container_id)
        .await?;
    println!("✓ Container operations: {}\n", operations.len());

    // Example 3: pve-container compatibility
    println!("4. pve-container Compatibility Example");

    let config_content = r#"
hostname: test-container
nameserver: 8.8.8.8
net0: name=test-vnet,hwaddr=02:00:00:00:00:01,ip=10.0.0.10/24
net1: bridge=vmbr0,hwaddr=02:00:00:00:00:02,ip=192.168.1.10/24,gw=192.168.1.1
"#;

    // Parse container configuration
    let parsed_config = integration
        .compat()
        .parse_container_config(container_id, config_content)
        .await?;
    println!("✓ Parsed container config:");
    println!("  - Hostname: {:?}", parsed_config.hostname);
    println!("  - Interfaces: {}", parsed_config.interfaces.len());

    for (name, iface) in &parsed_config.interfaces {
        println!(
            "    - {}: {:?} -> {:?}",
            name,
            iface.network_backend(),
            iface.ip
        );
    }

    // Generate configuration back
    let generated_config = integration
        .compat()
        .generate_container_config(&parsed_config)
        .await?;
    println!("✓ Generated config:\n{}\n", generated_config);

    // Example 4: Hook execution
    println!("5. Hook Execution Example");

    // Execute lifecycle hooks
    integration
        .hooks()
        .execute_lifecycle_hooks(
            container_id,
            ContainerNetworkEventType::ContainerStarted,
            &parsed_config,
        )
        .await?;

    // Execute config change hooks
    let mut updated_config = parsed_config.clone();
    let mut new_interface = ContainerNetworkInterface::new("net2".to_string());
    new_interface.vnet = Some("new-vnet".to_string());
    new_interface.ip = Some("10.1.0.10/24".to_string());
    updated_config.add_interface(new_interface)?;

    integration
        .hooks()
        .execute_config_hooks(container_id, Some(&parsed_config), &updated_config)
        .await?;

    // Get hook statistics
    let stats = integration.hooks().get_hook_statistics().await?;
    println!("✓ Hook execution statistics:");
    for (hook_name, stat) in stats {
        println!(
            "  - {}: {} executions ({} successful, {} failed)",
            hook_name, stat.total_executions, stat.successful_executions, stat.failed_executions
        );
    }

    // Example 5: Cleanup
    println!("\n6. Cleanup Example");

    // Cleanup container bindings
    integration
        .vnet_binding()
        .cleanup_container_bindings(container_id)
        .await?;
    println!("✓ Cleaned up container bindings");

    // Cleanup hotplug operations
    integration.hotplug().cleanup_completed_operations().await?;
    println!("✓ Cleaned up completed operations");

    // Clear hook execution history
    integration
        .hooks()
        .clear_execution_history(Some(container_id))
        .await?;
    println!("✓ Cleared hook execution history");

    println!("\n=== Container Integration Example Complete ===");
    Ok(())
}
