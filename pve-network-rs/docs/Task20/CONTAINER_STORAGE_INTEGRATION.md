# Container Integration Overview

## Architecture
- `ContainerIntegration` bundles:
  - `VNetBinding` (bind/unbind networks to LXC interfaces).
  - `ContainerNetworkHotplug` (add/remove/update while running).
  - `PveContainerCompat` for parsing/generating legacy `/etc/pve/lxc/<vmid>.conf`.
  - `ContainerNetworkHooks` with async hook trait for future Rust `pve-container`.
- Communication: hooks emit `ContainerNetworkEvent` for lifecycle/state changes.

## Key Workflows
### Binding VNets
```rust
let iface = ContainerNetworkInterface::new("net0".into());
iface.vnet = Some("zone1-vnet1".into());
vnet_binding.bind_vnet("zone1-vnet1", vmid, &iface).await?;
```
- Validates VNet existence (via SDN core).
- Records binding in `container_bindings` maps, emits VNetBound event.

### Hotplug Operations
```rust
hotplug.hotplug_add(vmid, iface).await?;
```
- Checks container running state (TODO: integrate with future runtime API).
- Executes registered hooks (logger, validation, future Rust integration).
- Maintains operation history and cleans up completed entries.

### Compatibility Layer
```rust
let config = compat.parse_container_config(vmid, &conf_str).await?;
let generated = compat.generate_container_config(&config).await?;
```
- Parses legacy key/value format (`net0: bridge=vmbr0,...`).
- Ensures Rust components can round-trip configuration before full migration.

## Hook Integration
- Register custom hook:
```rust
hooks.register_hook("my-hook".into(), MyHook::new()).await?;
```
- Methods executed on lifecycle/per-interface events:
  - `on_lifecycle_event`, `on_network_change`, `on_validation` (future extension).
- Execution history accessible for debugging.

## Future Rust `pve-container`
- `RustContainerIntegrationHook` placeholder processes container start/stop/migrate.
- Shared data structures (once `pve-shared-types` introduced) will allow direct struct reuse.

---
# Storage Integration Overview

## Architecture
- `StorageNetworkConfig` (interface, VLAN, QoS).
- `StorageVlanManager` manages VLAN interfaces and isolation rules (iptables).
- `StoragePlugin` trait for backend-specific operations (NFS, CIFS, iSCSI).
- `DefaultStorageNetworkManager` orchestrates network updates + pmxcfs config.
- Future integration (`DefaultFutureStorageIntegration`) exposes Rust-to-Rust APIs for upcoming `pve-storage`.

## Workflows
### Configuring Storage Network
```rust
storage_manager.configure_network("mystorage", &config).await?;
```
- Validates interface exists, VLAN range, QoS parameters.
- Creates VLAN interface via `StorageVlanManager`.
- Applies backend-specific configuration through plugin registry.

### Plugin Implementations
- **NFS** (`NfsStoragePlugin`): validates server/export, handles mount options.
- **CIFS**: ensures credentials, domain/workgroup settings.
- **iSCSI**: configures interface binding, initiator settings, firewall rules.
- Registry (`StoragePluginRegistry`) chooses plugin by backend type; dynamic registration possible for custom plugins.

### VLAN Isolation & QoS
```rust
let vlan_config = StorageVlanConfig { .. };
let interface = vlan_manager.create_storage_vlan("mystorage", &vlan_config).await?;
```
- Applies iptables rules to isolate traffic.
- Supports selective policies (allowed VLANs/protocols) and management access.

### Path Resolution Utilities
- `StoragePathResolver` and caches resolve exports, mount points, and track state.

## Future Rust `pve-storage`
- `FutureStorageIntegration` trait defines API for registering storage backends, configuring networks, handling events.
- Builder pattern allows plugging event handlers (e.g., metrics, audit).

---
# Operational Notes

- **Documentation location:** container integration (`crates/container-integration`), storage integration (`crates/storage-integration`).
- **Hooks & events:** coordinate via forthcoming `EventBus` for cross-component notifications.
- **Testing:** unit tests cover binding/hotplug/statistics; storage plugins have validation tests (ensure to mock external dependencies).
- **Migration impact:** both subsystems rely on shared types (`pve-shared-types`) for future Rust components.
