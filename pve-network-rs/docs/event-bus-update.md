# Event Bus Integration Overview

- Added shared `SystemEvent`, `ChangeType`, and `ConfigChange` definitions under `crates/pve-shared-types/src/events.rs` so all services publish and consume a common event payload.
- Introduced `pve-event-bus` crate providing an async `EventBus` with listener registration (`EventListener` trait) and warnings for failing hooks.
- Container, storage, and migration hooks can now subscribe to the bus:
  - `ContainerNetworkHooks` exposes `bind_event_bus`/`notify_system_event` and forwards incoming `SystemEvent`s to registered hook implementations.
  - `StorageHooks` and `MigrationHooks` mirror the same pattern with simple logging hooks that illustrate how services react to events.
- Exposed helper on `ContainerIntegration::bind_event_bus` so higher-level services can wire the hooks during bootstrap.
- Workspace `Cargo.toml` and per-crate manifests updated to include the new bus dependency.
- Verified changes with `cargo fmt --all` and `cargo check --workspace` (no new errors).

Use the new bus by creating one instance during API startup, calling `bind_event_bus` on the integration managers, then publishing `SystemEvent` values whenever a cross-service action occurs.
