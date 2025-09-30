# Task 12: LXC Container Integration

This document describes the implementation of LXC container integration for pve-network, providing VNet binding, hotplug operations, and compatibility with pve-container procedures.

## Overview

The container integration functionality enables seamless network management for LXC containers, supporting both traditional bridge networking and SDN VNets. It provides compatibility with existing pve-container procedures while preparing for future Rust-based container management.

## Architecture

### Components

1. **VNet Binding Manager** (`vnet_binding.rs`)
   - Manages binding of SDN VNets to container interfaces
   - Tracks binding relationships and metadata
   - Provides cleanup and validation functionality

2. **Hotplug Manager** (`hotplug.rs`)
   - Handles dynamic addition/removal of network interfaces
   - Manages operation tracking and status
   - Coordinates with container runtime

3. **pve-container Compatibility** (`pve_container_compat.rs`)
   - Parses and generates pve-container configuration format
   - Maintains compatibility with existing container procedures
   - Handles configuration file management

4. **Network Hooks** (`hooks.rs`)
   - Provides extensible hook system for network events
   - Supports future Rust pve-container integration
   - Enables custom network event handling

5. **Container Types** (`types.rs`)
   - Defines data structures for container network configuration
   - Provides validation and serialization support
   - Supports both bridge and VNet networking

## Key Features

### VNet Binding

```rust
// Bind VNet to container interface
let vnet_binding = VNetBinding::new();
vnet_binding.bind_vnet("test-vnet", container_id, &interface).await?;

// Check binding status
let is_bound = vnet_binding.is_interface_bound(container_id, "net0").await?;

// Get bound VNet
let vnet = vnet_binding.get_interface_vnet(container_id, "net0").await?;
```

### Hotplug Operations

```rust
// Hotplug add interface
let hotplug = ContainerNetworkHotplug::new();
let operation_id = hotplug.hotplug_add(container_id, interface).await?;

// Check operation status
let status = hotplug.get_operation_status(&operation_id).await?;

// Hotplug remove interface
let remove_id = hotplug.hotplug_remove(container_id, "net1".to_string()).await?;
```

### pve-container Compatibility

```rust
// Parse container configuration
let compat = PveContainerCompat::new();
let config = compat.parse_container_config(container_id, config_content).await?;

// Generate configuration
let generated = compat.generate_container_config(&config).await?;

// Update interface
compat.update_interface(container_id, interface).await?;
```

### Network Hooks

```rust
// Register hooks
let hooks = ContainerNetworkHooks::new();
hooks.register_hook("logger".to_string(), NetworkEventLogger::new()).await?;

// Execute lifecycle hooks
hooks.execute_lifecycle_hooks(
    container_id,
    ContainerNetworkEventType::ContainerStarted,
    &config,
).await?;
```

## Configuration Format

### Container Network Interface

```rust
pub struct ContainerNetworkInterface {
    pub name: String,                    // Interface name (e.g., "net0")
    pub vnet: Option<String>,           // SDN VNet name
    pub bridge: Option<String>,         // Bridge name (traditional)
    pub hwaddr: Option<String>,         // MAC address
    pub ip: Option<String>,             // IP configuration
    pub ip6: Option<String>,            // IPv6 configuration
    pub gw: Option<String>,             // Gateway
    pub gw6: Option<String>,            // IPv6 gateway
    pub tag: Option<u16>,               // VLAN tag
    pub trunks: Option<String>,         // Trunk configuration
    pub firewall: Option<bool>,         // Firewall enabled
    pub link_down: Option<bool>,        // Link down
    pub mtu: Option<u16>,               // MTU size
    pub rate: Option<f64>,              // Rate limiting
    pub options: HashMap<String, String>, // Additional options
}
```

### pve-container Format Compatibility

The implementation supports parsing and generating pve-container configuration format:

```
# Traditional bridge networking
net0: bridge=vmbr0,hwaddr=02:00:00:00:00:01,ip=192.168.1.10/24,gw=192.168.1.1

# SDN VNet networking
net1: name=test-vnet,hwaddr=02:00:00:00:00:02,ip=10.0.0.10/24

# Advanced options
net2: bridge=vmbr0,tag=100,firewall=1,rate=100,mtu=1400
```

## API Endpoints

The container integration provides REST API endpoints for network management:

### Container Network Configuration

- `GET /api2/json/nodes/{node}/lxc/{vmid}/network` - Get container network configuration
- `POST /api2/json/nodes/{node}/lxc/{vmid}/network/{interface}` - Update network interface
- `DELETE /api2/json/nodes/{node}/lxc/{vmid}/network/{interface}` - Remove network interface

### Hotplug Operations

- `POST /api2/json/nodes/{node}/lxc/{vmid}/network/hotplug/add` - Hotplug add interface
- `POST /api2/json/nodes/{node}/lxc/{vmid}/network/hotplug/remove` - Hotplug remove interface
- `GET /api2/json/nodes/{node}/lxc/{vmid}/network/hotplug/{operation_id}` - Get operation status

### VNet Binding

- `POST /api2/json/nodes/{node}/lxc/{vmid}/network/{interface}/bind` - Bind VNet
- `DELETE /api2/json/nodes/{node}/lxc/{vmid}/network/{interface}/bind` - Unbind VNet
- `GET /api2/json/nodes/{node}/lxc/{vmid}/network/vnets` - List container VNets
- `GET /api2/json/sdn/vnets/{vnet}/containers` - List VNet containers

## Requirements Compliance

### Requirement 10.1: Container Network Interface Setup
✅ **Implemented**: The system correctly configures network interfaces for LXC containers using the same mechanisms as the Perl version through the `PveContainerCompat` layer.

### Requirement 10.2: pve-container Compatibility
✅ **Implemented**: The `PveContainerCompat` component maintains full compatibility with pve-container network setup procedures, parsing and generating identical configuration formats.

### Requirement 10.3: VNet-to-Container Binding
✅ **Implemented**: The `VNetBinding` component supports the same VNet-to-container binding mechanisms as the Perl version, with full tracking and validation.

### Requirement 10.4: Hotplug Coordination
✅ **Implemented**: The `ContainerNetworkHotplug` component properly coordinates with LXC lifecycle management for hotplug network operations.

### Requirement 11.1: Future Rust pve-container API Design
✅ **Implemented**: Network APIs are designed with interfaces that can be easily consumed by a future Rust pve-container implementation.

### Requirement 11.2: Rust-Compatible Event Handling
✅ **Implemented**: The hook system provides event handling and interfaces suitable for Rust-based container management, with the `RustContainerIntegrationHook` as a foundation.

## Testing

### Unit Tests

Each component includes comprehensive unit tests:

```bash
# Run container integration tests
cargo test -p container-integration

# Run specific test modules
cargo test -p container-integration vnet_binding::tests
cargo test -p container-integration hotplug::tests
cargo test -p container-integration pve_container_compat::tests
```

### Integration Tests

```bash
# Run container integration example
cargo run --example container_integration

# Test with real container configurations
cargo test -p container-integration --test integration_tests
```

### Example Usage

The `container_integration.rs` example demonstrates all major functionality:

```bash
cargo run --example container_integration
```

## Future Integration Points

### Rust pve-container Integration

The hook system is designed to support future Rust pve-container integration:

1. **Shared Data Structures**: Common types can be reused between network and container management
2. **Event Coordination**: Hooks provide coordination points for container lifecycle events
3. **Efficient Communication**: Rust-to-Rust communication patterns are established
4. **API Compatibility**: Network APIs are designed for easy consumption by container management

### Storage Integration

The container integration prepares for storage network coordination:

1. **Network Storage Support**: Interface types support storage network configurations
2. **VLAN Isolation**: Storage traffic can be isolated using VLAN tagging
3. **Performance Optimization**: Network configuration optimized for storage workloads

## Migration Strategy

The container integration supports gradual migration:

1. **Phase 1**: Read-only compatibility with existing pve-container
2. **Phase 2**: Hotplug operations with fallback to Perl
3. **Phase 3**: Full network configuration management
4. **Phase 4**: Native Rust container integration

## Monitoring and Debugging

### Hook Execution Tracking

```rust
// Get hook statistics
let stats = hooks.get_hook_statistics().await?;
for (hook_name, stat) in stats {
    println!("{}: {} executions ({} successful)", 
        hook_name, stat.total_executions, stat.successful_executions);
}
```

### Operation Monitoring

```rust
// Monitor hotplug operations
let operations = hotplug.list_container_operations(container_id).await?;
for op in operations {
    println!("Operation {}: {:?}", op.id, op.status);
}
```

### VNet Binding Tracking

```rust
// List all bindings
let bindings = vnet_binding.list_bindings().await?;
for binding in bindings {
    println!("VNet '{}' -> Container {} Interface '{}'", 
        binding.vnet, binding.container_id, binding.interface_name);
}
```

## Conclusion

The container integration implementation provides comprehensive support for LXC container networking while maintaining full compatibility with existing pve-container procedures. The modular design enables future integration with Rust-based container management and provides a solid foundation for advanced networking features.