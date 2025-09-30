# Shared Types Crate Overview

`pve-shared-types` centralizes data models used across pve-network services.

## Modules
- `network`: interfaces, IP addresses, bond/bridge/VLAN definitions.
- `sdn`: zone/vnet/subnet/controller configurations.
- `ipam`: IPAM configuration and allocation records.
- `container`: container network interfaces, state, events.
- `storage`: storage network & VLAN settings.
- `migration`: migration phase/config primitives.
- `events`: generic system events and interface-change records.

## Usage
Add to Cargo dependencies via workspace:
```toml
pve-shared-types = { path = "../pve-shared-types" }
```

Import shared definitions, for example:
```rust
use pve_shared_types::{NetworkConfiguration, Interface, ZoneConfig};
```

All structures derive `serde::{Serialize, Deserialize}` for JSON transport.
