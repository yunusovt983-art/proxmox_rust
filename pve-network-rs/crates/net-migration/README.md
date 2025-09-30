# PVE Network Migration System

This crate provides a comprehensive migration system for transitioning the Proxmox VE network management from Perl to Rust implementation. It enables gradual, safe migration with fallback capabilities and comprehensive monitoring.

## Features

- **Phased Migration**: Gradual transition through defined migration phases
- **Automatic Fallback**: Falls back to Perl implementation on Rust failures
- **Configuration Management**: Flexible configuration with environment overrides
- **Metrics Collection**: Detailed metrics on migration performance
- **Health Monitoring**: System health checks and status reporting
- **CLI Management**: Command-line tools for migration control

## Migration Phases

The migration system supports several phases for gradual rollout:

1. **PerlOnly**: All requests handled by Perl (default/fallback)
2. **RustReadOnly**: Read-only operations handled by Rust, writes by Perl
3. **RustBasicWrite**: Basic write operations handled by Rust
4. **RustAdvanced**: Advanced network functions handled by Rust
5. **RustSdn**: SDN operations handled by Rust
6. **RustFull**: Full Rust implementation

## Quick Start

### 1. Configuration

Create a configuration file (e.g., `/etc/pve/network-migration.conf`):

```toml
# Migration phase
phase = "rust-read-only"

# Fallback settings
fallback_enabled = true
fallback_timeout = 30

# Perl API configuration
perl_api_base_url = "http://localhost:8006"
perl_api_timeout = 60

# Logging
log_migration_decisions = true
metrics_enabled = true
```

### 2. Basic Usage

```rust
use net_migration::{
    MigrationConfig, MigrationMiddleware,
    HttpPerlApiClient, middleware::RustApiHandler,
};
use std::sync::Arc;
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Load configuration
    let config = MigrationConfig::load_with_defaults()?;
    
    // Set up Perl client
    let perl_client = Arc::new(HttpPerlApiClient::new(
        config.perl_api_base_url.clone(),
        Duration::from_secs(config.perl_api_timeout),
    ));
    
    // Set up Rust handler (your implementation)
    let rust_handler: Arc<dyn RustApiHandler> = Arc::new(YourRustHandler::new());
    
    // Create middleware
    let middleware = MigrationMiddleware::new(config, rust_handler, perl_client);
    
    // Use middleware to handle requests
    let response = middleware.handle_request(request).await?;
    
    Ok(())
}
```

### 3. CLI Management

Use the `migration-ctl` tool to manage the migration:

```bash
# Check current status
migration-ctl status

# Set migration phase
migration-ctl set-phase rust-read-only

# Enable/disable fallback
migration-ctl fallback true

# Test Perl connectivity
migration-ctl test-perl

# Generate example configuration
migration-ctl generate-config --output migration.conf
```

## Configuration

### Environment Variables

- `PVE_NETWORK_MIGRATION_PHASE`: Override migration phase
- `PVE_NETWORK_MIGRATION_FALLBACK`: Enable/disable fallback
- `PERL_API_URL`: Perl API base URL
- `PERL_API_TOKEN`: Authentication token for Perl API

### Configuration File Format

The configuration file uses TOML format:

```toml
# Global settings
phase = "rust-read-only"
fallback_enabled = true
fallback_timeout = 30
perl_api_base_url = "http://localhost:8006"
perl_api_timeout = 60
log_migration_decisions = true
metrics_enabled = true

# Feature flags
[features]
enable_sdn_migration = false
enable_advanced_networking = true

# Per-endpoint configuration
[endpoints."/api2/json/nodes/{node}/network"]
use_rust = true
fallback_on_error = true
rust_timeout = 30
rust_methods = ["GET"]
```

## Architecture

### Components

1. **MigrationConfig**: Configuration management and phase control
2. **MigrationMiddleware**: HTTP request routing and handling
3. **PerlApiClient**: Communication with existing Perl implementation
4. **FallbackHandler**: Error handling and fallback logic
5. **RustApiHandler**: Interface for Rust API implementations

### Request Flow

```
HTTP Request
     ↓
MigrationMiddleware
     ↓
Should use Rust? ──No──→ PerlApiClient
     ↓ Yes
RustApiHandler
     ↓
Success? ──No──→ FallbackHandler ──→ PerlApiClient
     ↓ Yes
HTTP Response
```

## Monitoring and Metrics

The migration system provides comprehensive metrics:

- **Total requests**: Number of requests processed
- **Rust success rate**: Percentage of successful Rust operations
- **Fallback rate**: Percentage of requests that fell back to Perl
- **Execution times**: Average response times for Rust and Perl
- **Error rates**: Failure rates for both implementations

Access metrics via:

```rust
if let Some(metrics) = middleware.get_metrics() {
    println!("Rust success rate: {:.2}%", metrics.rust_success_rate() * 100.0);
    println!("Fallback rate: {:.2}%", metrics.fallback_rate() * 100.0);
}
```

## Health Checks

Monitor system health:

```rust
let health = middleware.health_check().await;
println!("Perl API healthy: {}", health["perl_api"]);
println!("Migration phase: {}", health["migration_phase"]);
```

## Error Handling

The system provides robust error handling:

- **Automatic fallback**: On Rust failures, automatically tries Perl
- **Timeout handling**: Configurable timeouts for operations
- **Error logging**: Detailed error logging for debugging
- **Graceful degradation**: System remains functional even with partial failures

## Testing

### Unit Tests

```bash
cargo test -p net-migration
```

### Integration Tests

```bash
# Start test server
cargo run --example migration_example

# Test endpoints
curl http://localhost:3000/api2/json/nodes/test/network
curl http://localhost:3000/health
curl http://localhost:3000/metrics
```

## Migration Strategy

### Phase 1: Read-Only Migration
1. Set phase to `rust-read-only`
2. Monitor metrics for Rust success rate
3. Verify API compatibility with existing clients

### Phase 2: Basic Write Operations
1. Set phase to `rust-basic-write`
2. Test basic network interface operations
3. Monitor for any regressions

### Phase 3: Advanced Features
1. Set phase to `rust-advanced`
2. Enable advanced networking features
3. Test bridge, VLAN, and bonding operations

### Phase 4: SDN Migration
1. Set phase to `rust-sdn`
2. Enable SDN functionality
3. Test zones, vnets, and IPAM operations

### Phase 5: Full Migration
1. Set phase to `rust-full`
2. All operations handled by Rust
3. Monitor for stability

### Rollback Procedure
1. Set phase to `perl-only`
2. All requests immediately route to Perl
3. System returns to original state

## Security Considerations

- **Authentication**: Supports token-based authentication for Perl API
- **Input validation**: All inputs validated before processing
- **Error information**: Sensitive error details not exposed to clients
- **Audit logging**: All migration decisions logged for audit

## Performance

The migration system is designed for minimal overhead:

- **Zero-copy routing**: Efficient request routing
- **Async operations**: Non-blocking I/O throughout
- **Connection pooling**: Reused connections to Perl API
- **Metrics collection**: Optional with minimal impact

## Troubleshooting

### Common Issues

1. **Perl API unreachable**
   - Check `perl_api_base_url` configuration
   - Verify network connectivity
   - Check authentication credentials

2. **High fallback rate**
   - Review Rust implementation logs
   - Check for configuration issues
   - Verify API compatibility

3. **Timeout errors**
   - Increase `rust_timeout` values
   - Check system resource usage
   - Review operation complexity

### Debug Logging

Enable debug logging:

```bash
RUST_LOG=debug cargo run --example migration_example
```

### Health Check Endpoint

Monitor system health:

```bash
curl http://localhost:3000/health
```

## Contributing

1. Follow the existing code style
2. Add tests for new functionality
3. Update documentation
4. Ensure backward compatibility

## License

This project is licensed under the AGPL-3.0 license.