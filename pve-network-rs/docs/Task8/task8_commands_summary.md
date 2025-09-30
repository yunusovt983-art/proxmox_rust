# Task 8 Commands Summary

## Build and Test Commands

### Project Build
```bash
# Build the entire project
cargo build

# Build with warnings as errors (for CI)
cargo build --all-features --all-targets
```

### Testing
```bash
# Run all tests
cargo test

# Run specific package tests
cargo test -p pve-network-core

# Run tests with output
cargo test -p pve-network-core -- --nocapture

# Run specific test
cargo test -p pve-network-core bridge::tests::test_bridge_config_creation
```

### Example Execution
```bash
# Run the advanced network functions example
cargo run --example advanced_network_functions

# Run example from examples directory
cd examples
cargo run --example advanced_network_functions
```

### Code Quality
```bash
# Check code formatting
cargo fmt --check

# Apply code formatting
cargo fmt

# Run clippy lints
cargo clippy --all-targets --all-features

# Fix clippy suggestions
cargo clippy --fix --all-targets --all-features
```

## Development Commands

### Documentation Generation
```bash
# Generate documentation
cargo doc --no-deps --open

# Generate documentation for specific package
cargo doc -p pve-network-core --no-deps --open
```

### Dependency Management
```bash
# Check for outdated dependencies
cargo outdated

# Update dependencies
cargo update

# Add new dependency
cargo add <dependency> --package pve-network-core
```

### Workspace Management
```bash
# Check workspace structure
cargo tree

# Clean build artifacts
cargo clean

# Check compilation without building
cargo check
```

## Testing Specific Features

### Bridge Testing
```bash
# Test bridge functionality
cargo test -p pve-network-core bridge

# Test VLAN-aware bridge features
cargo test -p pve-network-core bridge::tests::test_bridge_config_creation
cargo test -p pve-network-core bridge::tests::test_bridge_validation
```

### Bond Testing
```bash
# Test bonding functionality
cargo test -p pve-network-core bond

# Test specific bond modes
cargo test -p pve-network-core bond::tests::test_bond_validation
cargo test -p pve-network-core bond::tests::test_bond_slave_management
```

### VLAN Testing
```bash
# Test VLAN functionality
cargo test -p pve-network-core vlan

# Test QinQ functionality
cargo test -p pve-network-core vlan::tests::test_qinq_configuration
cargo test -p pve-network-core vlan::tests::test_vlan_config_with_options
```

## Debugging Commands

### Verbose Output
```bash
# Build with verbose output
cargo build --verbose

# Test with verbose output
cargo test --verbose

# Run example with debug output
RUST_LOG=debug cargo run --example advanced_network_functions
```

### Memory and Performance
```bash
# Build with debug symbols
cargo build --profile dev

# Build optimized for debugging
cargo build --profile dev-opt

# Profile memory usage (requires additional tools)
cargo build --release
valgrind --tool=massif target/release/advanced_network_functions
```

## Integration Commands

### API Server Testing
```bash
# Run API server (when implemented)
cargo run --bin api-server

# Test API endpoints
curl -X GET http://localhost:8080/api2/json/nodes/test/network
```

### Configuration Testing
```bash
# Test configuration parsing
cargo test -p pve-network-config

# Test configuration validation
cargo test -p pve-network-validate

# Test configuration application
cargo test -p pve-network-apply
```

## Continuous Integration Commands

### Full CI Pipeline
```bash
# Format check
cargo fmt --check

# Lint check
cargo clippy --all-targets --all-features -- -D warnings

# Build all targets
cargo build --all-targets --all-features

# Run all tests
cargo test --all-features

# Generate documentation
cargo doc --no-deps --all-features

# Check for security vulnerabilities
cargo audit
```

### Release Preparation
```bash
# Check package for publishing
cargo package --package pve-network-core

# Dry run publish
cargo publish --dry-run --package pve-network-core

# Build release version
cargo build --release --all-features
```

## Useful Development Aliases

Add these to your shell configuration for convenience:

```bash
# Build aliases
alias cb='cargo build'
alias ct='cargo test'
alias cc='cargo check'
alias cf='cargo fmt'
alias ccl='cargo clippy'

# Project specific aliases
alias pve-test='cargo test -p pve-network-core'
alias pve-example='cargo run --example advanced_network_functions'
alias pve-doc='cargo doc -p pve-network-core --no-deps --open'
```

## Performance Benchmarking

```bash
# Run benchmarks (when implemented)
cargo bench

# Profile with perf (Linux)
cargo build --release
perf record --call-graph=dwarf target/release/advanced_network_functions
perf report

# Memory profiling with heaptrack (Linux)
heaptrack target/release/advanced_network_functions
```

These commands provide comprehensive coverage for building, testing, and maintaining the advanced network functions implementation.