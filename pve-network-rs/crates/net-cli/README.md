# pvenet - Proxmox VE Network CLI

A command-line interface for managing Proxmox VE network configurations, providing validation, application, rollback, and monitoring capabilities while maintaining full compatibility with existing Proxmox network tools.

## Features

- **Configuration Validation**: Comprehensive syntax and semantic validation of network configurations
- **Safe Application**: Transactional application of network changes with automatic rollback on failure
- **Version Management**: Backup and rollback capabilities for network configurations
- **Status Monitoring**: Detailed network interface status and statistics
- **Compatibility**: Full compatibility with existing Proxmox network tools and output formats
- **Performance**: Enhanced performance through Rust implementation

## Installation

### From Source

```bash
cd pve-network-rs
cargo build --release --bin pvenet
sudo cp target/release/pvenet /usr/bin/
sudo cp docs/pvenet.1 /usr/share/man/man1/
sudo mandb
```

### From Package

```bash
# Install the Debian package (when available)
sudo apt install pve-network-rs
```

## Usage

### Basic Commands

```bash
# Validate network configuration
pvenet validate

# Apply configuration changes (dry-run first)
pvenet apply --dry-run
pvenet apply

# Show network status
pvenet status
pvenet status --verbose

# Rollback to previous version
pvenet rollback
```

### Advanced Usage

```bash
# Validate specific configuration file
pvenet validate --config /path/to/interfaces

# Validate specific interface
pvenet validate --interface eth0

# Apply changes to specific interface
pvenet apply --interface vmbr0 --dry-run

# List available rollback versions
pvenet rollback --list

# Rollback to specific version
pvenet rollback --version 20231201-120000

# Show interface statistics
pvenet status --stats

# List interfaces in JSON format (pvesh compatible)
pvenet list --format json

# Show configuration for specific interface
pvenet show --interface eth0

# Reload network configuration
pvenet reload
```

### Compatibility Mode

The tool provides full compatibility with existing Proxmox network utilities:

```bash
# pvesh-compatible commands
pvenet list --node pve-node1 --format json
pvenet show --node pve-node1 --interface vmbr0
pvenet reload --node pve-node1

# Use the compatibility wrapper
./scripts/pvenet-compat.sh validate
./scripts/pvenet-compat.sh apply --dry-run
./scripts/pvenet-compat.sh status --verbose
```

## Configuration

### Network Configuration Files

The tool works with standard Proxmox VE network configuration files:

- `/etc/network/interfaces` - Main network configuration
- `/etc/network/interfaces.d/*` - Additional configuration files
- `/etc/pve/sdn/` - SDN configuration directory

### Backup and Rollback

Network configuration backups are stored in:
- `/etc/pve/network-backups/` - Configuration backups with timestamps

### Logging

Operation logs are written to:
- `/var/log/pve-network-rollback.log` - Rollback operations
- System journal for general operations

## Examples

### Validation Examples

```bash
# Basic validation
pvenet validate
# Output:
# Validating network configuration: /etc/network/interfaces
# ✓ Syntax validation passed
# ✓ Semantic validation passed
# ✓ ifupdown2 dry-run validation passed
# Configuration is valid

# Validate with verbose output
pvenet --verbose validate
# Shows detailed validation steps and any warnings

# Validate specific interface
pvenet validate --interface vmbr0
# Validates only the vmbr0 interface configuration
```

### Application Examples

```bash
# Dry-run application
pvenet apply --dry-run
# Output:
# Performing dry-run apply of network configuration
# ✓ Configuration validation passed
# ✓ ifupdown2 dry-run passed
# Dry-run completed successfully - configuration would be applied

# Apply configuration
pvenet apply
# Output:
# Applying network configuration: /etc/network/interfaces
# Validating configuration before apply...
# ✓ Configuration validation passed
# Applying network configuration...
# ✓ Network configuration applied successfully
# Reloaded interfaces: eth0, vmbr0
```

### Status Examples

```bash
# Basic status
pvenet status
# Output:
# Network status:
# Interface       Type       Method          Address              Status
# --------------------------------------------------------------------------------
# lo              physical   loopback        127.0.0.1/8         UP
# eth0            physical   static          192.168.1.10/24     UP
# vmbr0           bridge     static          10.0.0.1/24         UP

# Detailed status
pvenet status --verbose
# Shows comprehensive interface information, system status, and configuration details

# Interface statistics
pvenet status --stats
# Shows packet counts, error rates, and throughput statistics
```

### Rollback Examples

```bash
# List available versions
pvenet rollback --list
# Output:
# Available backup versions:
# Version              Date                 Time
# ------------------------------------------------------------
# 20231201-143022      2023-12-01          14:30:22
# 20231201-120000      2023-12-01          12:00:00
# 20231130-180000      2023-11-30          18:00:00

# Rollback to previous version
pvenet rollback
# Output:
# Rolling back network configuration to previous version
# Found latest backup: 20231201-143022
# ✓ Successfully rolled back to version: 20231201-143022
# Restored interfaces: eth0, vmbr0

# Rollback to specific version
pvenet rollback --version 20231201-120000
# Rolls back to the specified backup version
```

## Error Handling

The tool provides comprehensive error handling with clear messages:

```bash
# Configuration errors
pvenet validate
# Error: Configuration error: Parse error at line 15: Invalid interface definition

# System errors
pvenet apply
# Error: System error: Failed to execute ifupdown2: Permission denied

# Validation errors
pvenet apply
# Error: Validation error: Interface eth0 validation failed: IP address conflict with eth1
```

## Integration

### With Existing Tools

The CLI integrates seamlessly with existing Proxmox tools:

```bash
# Can be used in place of existing network utilities
alias pve-network-validate="pvenet validate"
alias pve-network-apply="pvenet apply"
alias pve-network-status="pvenet status"

# Works with existing scripts and automation
pvenet validate && pvenet apply --dry-run && pvenet apply
```

### With Monitoring Systems

```bash
# JSON output for monitoring integration
pvenet status --format json | jq '.interfaces[] | select(.status != "UP")'

# Statistics for performance monitoring
pvenet status --stats --interface eth0 | grep "errors"
```

## Development

### Building

```bash
# Build the CLI
cargo build --bin pvenet

# Run tests
cargo test --bin pvenet

# Build with optimizations
cargo build --release --bin pvenet
```

### Testing

```bash
# Run unit tests
cargo test -p pvenet

# Run integration tests
cargo test -p pvenet --test integration

# Test CLI argument parsing
cargo test -p pvenet test_cli_parsing
```

### Contributing

1. Follow the existing code style and patterns
2. Add tests for new functionality
3. Update documentation and man pages
4. Ensure compatibility with existing tools
5. Test on actual Proxmox VE systems

## Compatibility

This tool maintains 100% compatibility with existing Proxmox VE network management tools:

- **API Compatibility**: Same REST API endpoints and responses
- **Configuration Compatibility**: Supports all existing configuration formats
- **Output Compatibility**: Identical output formats and error messages
- **Behavior Compatibility**: Same validation rules and application logic

## License

This project is licensed under the same terms as Proxmox VE.

## Support

For support and bug reports, please contact the Proxmox VE development team or file issues in the project repository.