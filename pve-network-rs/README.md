# pve-network-rs

Rust implementation of Proxmox VE Network Management, including traditional networking (bridges, bonds, VLANs) and Software Defined Networking (SDN) capabilities.

## Overview

This project provides a complete Rust rewrite of the pve-network package, maintaining 100% API compatibility while modernizing the codebase. The implementation follows the architectural patterns established in Proxmox Datacenter Manager.

## Features

- **Traditional Networking**: Support for bridges, bonds, VLANs, and physical interfaces
- **Software Defined Networking (SDN)**: Complete SDN stack with zones, vnets, subnets, and IPAM
- **API Compatibility**: 100% compatible with existing Perl implementation
- **Incremental Migration**: Gradual migration strategy with feature flags
- **Performance**: Improved performance and resource efficiency
- **Safety**: Memory safety and type safety through Rust

## Architecture

The project is organized as a Cargo workspace with the following crates:

- `pve-network-core`: Core types and business logic
- `pve-network-api`: REST API endpoints
- `pve-network-config`: Configuration parsing and generation
- `pve-network-validate`: Configuration validation
- `pve-network-apply`: Configuration application
- `pve-sdn-core`: SDN core abstractions
- `pve-sdn-drivers`: SDN driver implementations
- `pvenet`: CLI utilities
- `pve-network-test`: Integration tests

## Building

```bash
# Build all crates
cargo build --release

# Run tests
cargo test

# Build Debian packages
dpkg-buildpackage -us -uc -b
```

## Installation

```bash
# Install main package
dpkg -i pve-network-rs_*.deb

# Install CLI utilities
dpkg -i pvenet_*.deb
```

## Usage

### CLI

```bash
# Validate configuration
pvenet validate

# Apply configuration (dry-run)
pvenet apply --dry-run

# Apply configuration
pvenet apply

# Show status
pvenet status --verbose

# Rollback configuration
pvenet rollback
```

### API

The REST API is fully compatible with the existing Perl implementation:

```bash
# List interfaces
curl -X GET /api2/json/nodes/{node}/network

# Create interface
curl -X POST /api2/json/nodes/{node}/network \
  -d '{"name": "br0", "type": "bridge", "method": "static", "address": "192.168.1.1/24"}'
```

## Migration Strategy

The migration follows a phased approach:

1. **Phase 1**: Read-only operations
2. **Phase 2**: Basic write operations
3. **Phase 3**: Advanced networking features
4. **Phase 4**: SDN functionality
5. **Phase 5**: Full feature parity

Feature flags allow gradual migration without service interruption.

## Development

### Requirements

- Rust 1.70+
- Debian build tools
- ifupdown2 (for testing)

### Testing

```bash
# Unit tests
cargo test

# Integration tests
cargo test --test integration

# Contract tests (requires Perl version)
cargo test --test contract
```

## License

AGPL-3.0

## Contributing

Please follow the Proxmox development guidelines and ensure all tests pass before submitting changes.