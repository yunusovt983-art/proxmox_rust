//! Complete migration system example
//! 
//! This example demonstrates how to set up and use the migration system
//! for gradual transition from Perl to Rust implementation.

use net_migration::{
    MigrationConfig, MigrationMiddleware, MigrationPhase,
    HttpPerlApiClient, 
    perl_client::{ApiRequest, ApiResponse, PerlApiClient},
    middleware::RustApiHandler,
};
use pve_network_api::{NetworkAPI, SDNAPI, ContainerNetworkAPI, StorageNetworkAPI, NetApiRustHandler};
use hyper::{Body, Request, Response, Server};
use hyper::service::{make_service_fn, service_fn};
use std::convert::Infallible;
use std::sync::Arc;
use std::time::Duration;
use serde_json::json;
use std::collections::HashMap;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();
    
    println!("PVE Network Migration Example");
    println!("=============================");
    
    // 1. Load migration configuration
    let mut config = load_migration_config().await?;
    println!("Loaded migration configuration: phase={:?}", config.phase);
    
    // 2. Set up Perl API client
    let perl_client = setup_perl_client(&config)?;
    println!("Perl API client configured: {}", config.perl_api_base_url);
    
    // 3. Set up Rust API handlers
    let rust_handler = setup_rust_handlers().await?;
    println!("Rust API handlers initialized");
    
    // 4. Create migration middleware
    let middleware = Arc::new(MigrationMiddleware::new(
        config.clone(),
        rust_handler,
        perl_client,
    ));
    println!("Migration middleware created");
    
    // 5. Test the system with different phases
    println!("\nTesting migration phases...");
    test_migration_phases(Arc::clone(&middleware)).await?;
    
    // 6. Start the server
    println!("\nStarting migration server...");
    start_server(middleware).await?;
    
    Ok(())
}

async fn load_migration_config() -> Result<MigrationConfig, Box<dyn std::error::Error>> {
    // Try to load from environment or use defaults
    let phase = std::env::var("MIGRATION_PHASE")
        .unwrap_or_else(|_| "rust-read-only".to_string())
        .parse()
        .unwrap_or(MigrationPhase::RustReadOnly);
    
    let mut config = MigrationConfig::default();
    config.phase = phase;
    config.update_endpoints_for_phase();
    
    // Override with environment variables if present
    if let Ok(perl_url) = std::env::var("PERL_API_URL") {
        config.perl_api_base_url = perl_url;
    }
    
    if let Ok(fallback_str) = std::env::var("FALLBACK_ENABLED") {
        config.fallback_enabled = fallback_str.parse().unwrap_or(true);
    }
    
    Ok(config)
}

fn setup_perl_client(config: &MigrationConfig) -> Result<Arc<dyn PerlApiClient>, Box<dyn std::error::Error>> {
    let client = HttpPerlApiClient::new(
        config.perl_api_base_url.clone(),
        Duration::from_secs(config.perl_api_timeout),
    );
    
    // Add authentication if available
    if let Ok(token) = std::env::var("PERL_API_TOKEN") {
        Ok(Arc::new(client.with_auth_token(token)))
    } else {
        Ok(Arc::new(client))
    }
}

async fn setup_rust_handlers() -> Result<Arc<dyn RustApiHandler>, Box<dyn std::error::Error>> {
    // Initialize the actual API handlers
    // In a real implementation, these would be properly configured
    let network_api = NetworkAPI::new().await?;
    let sdn_api = SDNAPI::new().await?;
    let container_api = ContainerNetworkAPI::new().await?;
    let storage_api = StorageNetworkAPI::new().await?;
    
    let handler = NetApiRustHandler::new(
        network_api,
        sdn_api,
        container_api,
        storage_api,
    );
    
    Ok(Arc::new(handler))
}

async fn test_migration_phases(middleware: Arc<MigrationMiddleware>) -> Result<(), Box<dyn std::error::Error>> {
    println!("Testing different migration scenarios...");
    
    // Test 1: Read-only operation (should use Rust in read-only phase)
    println!("1. Testing read-only operation...");
    let read_request = Request::builder()
        .method("GET")
        .uri("/api2/json/nodes/test/network")
        .body(Body::empty())?;
    
    match middleware.handle_request(read_request).await {
        Ok(response) => {
            println!("   ✓ Read operation successful: status={}", response.status());
        }
        Err(e) => {
            println!("   ✗ Read operation failed: {}", e);
        }
    }
    
    // Test 2: Write operation (behavior depends on phase)
    println!("2. Testing write operation...");
    let write_body = json!({
        "iface": "test0",
        "type": "bridge",
        "method": "static",
        "address": "192.168.100.1/24"
    });
    
    let write_request = Request::builder()
        .method("POST")
        .uri("/api2/json/nodes/test/network")
        .header("Content-Type", "application/json")
        .body(Body::from(write_body.to_string()))?;
    
    match middleware.handle_request(write_request).await {
        Ok(response) => {
            println!("   ✓ Write operation successful: status={}", response.status());
        }
        Err(e) => {
            println!("   ✗ Write operation failed: {}", e);
        }
    }
    
    // Test 3: Health check
    println!("3. Testing health check...");
    let health = middleware.health_check().await;
    println!("   Health status: {}", serde_json::to_string_pretty(&health)?);
    
    // Test 4: Metrics (if available)
    if let Some(metrics) = middleware.get_metrics() {
        println!("4. Migration metrics:");
        println!("   Total requests: {}", metrics.total_requests);
        println!("   Rust success rate: {:.2}%", metrics.rust_success_rate() * 100.0);
        println!("   Fallback rate: {:.2}%", metrics.fallback_rate() * 100.0);
        println!("   Overall success rate: {:.2}%", metrics.success_rate() * 100.0);
    }
    
    Ok(())
}

async fn start_server(middleware: Arc<MigrationMiddleware>) -> Result<(), Box<dyn std::error::Error>> {
    let make_svc = make_service_fn(move |_conn| {
        let middleware = Arc::clone(&middleware);
        async move {
            Ok::<_, Infallible>(service_fn(move |req| {
                handle_request(req, Arc::clone(&middleware))
            }))
        }
    });
    
    let addr = ([127, 0, 0, 1], 3000).into();
    let server = Server::bind(&addr).serve(make_svc);
    
    println!("Migration server running on http://{}", addr);
    println!("\nAvailable endpoints:");
    println!("  GET  /api2/json/nodes/{{node}}/network");
    println!("  POST /api2/json/nodes/{{node}}/network");
    println!("  GET  /api2/json/sdn/zones");
    println!("  POST /api2/json/sdn/zones");
    println!("  GET  /health");
    println!("  GET  /metrics");
    
    println!("\nEnvironment variables:");
    println!("  MIGRATION_PHASE: Set migration phase (perl-only, rust-read-only, rust-full, etc.)");
    println!("  PERL_API_URL: Perl API base URL (default: http://localhost:8006)");
    println!("  FALLBACK_ENABLED: Enable/disable fallback (default: true)");
    println!("  PERL_API_TOKEN: Authentication token for Perl API");
    
    if let Err(e) = server.await {
        eprintln!("Server error: {}", e);
    }
    
    Ok(())
}

async fn handle_request(
    req: Request<Body>,
    middleware: Arc<MigrationMiddleware>,
) -> Result<Response<Body>, Infallible> {
    let path = req.uri().path();
    
    // Handle special endpoints
    match path {
        "/health" => {
            let health = middleware.health_check().await;
            let health_json = serde_json::to_string_pretty(&health).unwrap();
            
            let response = Response::builder()
                .status(200)
                .header("Content-Type", "application/json")
                .body(Body::from(health_json))
                .unwrap();
            
            Ok(response)
        }
        "/metrics" => {
            if let Some(metrics) = middleware.get_metrics() {
                let metrics_json = serde_json::json!({
                    "total_requests": metrics.total_requests,
                    "rust_successes": metrics.rust_successes,
                    "rust_failures": metrics.rust_failures,
                    "fallback_successes": metrics.fallback_successes,
                    "fallback_failures": metrics.fallback_failures,
                    "total_fallbacks": metrics.total_fallbacks,
                    "rust_success_rate": metrics.rust_success_rate(),
                    "fallback_rate": metrics.fallback_rate(),
                    "overall_success_rate": metrics.success_rate(),
                    "average_rust_time_ms": metrics.average_rust_time.as_millis(),
                    "average_fallback_time_ms": metrics.average_fallback_time.as_millis(),
                });
                
                let response = Response::builder()
                    .status(200)
                    .header("Content-Type", "application/json")
                    .body(Body::from(metrics_json.to_string()))
                    .unwrap();
                
                Ok(response)
            } else {
                let response = Response::builder()
                    .status(404)
                    .body(Body::from("Metrics not available"))
                    .unwrap();
                
                Ok(response)
            }
        }
        _ => {
            // Handle through migration middleware
            match middleware.handle_request(req).await {
                Ok(response) => Ok(response),
                Err(e) => {
                    eprintln!("Request handling error: {}", e);
                    let error_response = Response::builder()
                        .status(500)
                        .header("Content-Type", "application/json")
                        .body(Body::from(json!({
                            "error": e.to_string(),
                            "migration_error": true
                        }).to_string()))
                        .unwrap();
                    Ok(error_response)
                }
            }
        }
    }
}

// Mock implementations for the example
// In a real implementation, these would be the actual API structs

impl NetworkAPI {
    async fn new() -> Result<Self, Box<dyn std::error::Error>> {
        // Initialize with actual dependencies
        Ok(NetworkAPI {})
    }
    
    async fn list_interfaces(&self, _node: String) -> Result<Vec<serde_json::Value>, Box<dyn std::error::Error>> {
        Ok(vec![
            json!({
                "iface": "eth0",
                "type": "eth",
                "method": "static",
                "address": "192.168.1.10/24",
                "active": 1
            }),
            json!({
                "iface": "vmbr0",
                "type": "bridge", 
                "method": "static",
                "address": "10.0.0.1/24",
                "active": 1
            })
        ])
    }
    
    async fn get_interface(&self, _node: String, iface: String) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
        Ok(json!({
            "iface": iface,
            "type": "eth",
            "method": "static",
            "address": "192.168.1.10/24",
            "active": 1
        }))
    }
    
    async fn create_interface(&self, _node: String, _config: serde_json::Value) -> Result<(), Box<dyn std::error::Error>> {
        Ok(())
    }
    
    async fn update_interface(&self, _node: String, _iface: String, _config: serde_json::Value) -> Result<(), Box<dyn std::error::Error>> {
        Ok(())
    }
    
    async fn delete_interface(&self, _node: String, _iface: String) -> Result<(), Box<dyn std::error::Error>> {
        Ok(())
    }
    
    async fn reload_network(&self, _node: String) -> Result<String, Box<dyn std::error::Error>> {
        Ok("UPID:test:00001234:00000000:00000000:netreload:test:root@pam:".to_string())
    }
}

impl SDNAPI {
    async fn new() -> Result<Self, Box<dyn std::error::Error>> {
        Ok(SDNAPI {})
    }
    
    async fn list_zones(&self) -> Result<Vec<serde_json::Value>, Box<dyn std::error::Error>> {
        Ok(vec![
            json!({
                "zone": "localnetwork",
                "type": "simple",
                "bridge": "vmbr0"
            })
        ])
    }
    
    async fn get_zone(&self, zone: String) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
        Ok(json!({
            "zone": zone,
            "type": "simple",
            "bridge": "vmbr0"
        }))
    }
    
    async fn create_zone(&self, _config: serde_json::Value) -> Result<(), Box<dyn std::error::Error>> {
        Ok(())
    }
    
    async fn update_zone(&self, _zone: String, _config: serde_json::Value) -> Result<(), Box<dyn std::error::Error>> {
        Ok(())
    }
    
    async fn delete_zone(&self, _zone: String) -> Result<(), Box<dyn std::error::Error>> {
        Ok(())
    }
    
    async fn list_vnets(&self) -> Result<Vec<serde_json::Value>, Box<dyn std::error::Error>> {
        Ok(vec![])
    }
    
    async fn get_vnet(&self, vnet: String) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
        Ok(json!({
            "vnet": vnet,
            "zone": "localnetwork"
        }))
    }
    
    async fn create_vnet(&self, _config: serde_json::Value) -> Result<(), Box<dyn std::error::Error>> {
        Ok(())
    }
    
    async fn update_vnet(&self, _vnet: String, _config: serde_json::Value) -> Result<(), Box<dyn std::error::Error>> {
        Ok(())
    }
    
    async fn delete_vnet(&self, _vnet: String) -> Result<(), Box<dyn std::error::Error>> {
        Ok(())
    }
}

impl ContainerNetworkAPI {
    async fn new() -> Result<Self, Box<dyn std::error::Error>> {
        Ok(ContainerNetworkAPI {})
    }
    
    async fn list_container_networks(&self, _node: String, _vmid: u32) -> Result<Vec<serde_json::Value>, Box<dyn std::error::Error>> {
        Ok(vec![])
    }
    
    async fn create_container_network(&self, _node: String, _vmid: u32, _config: serde_json::Value) -> Result<(), Box<dyn std::error::Error>> {
        Ok(())
    }
}

impl StorageNetworkAPI {
    async fn new() -> Result<Self, Box<dyn std::error::Error>> {
        Ok(StorageNetworkAPI {})
    }
    
    async fn get_storage_network_config(&self, _storage: String) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
        Ok(json!({}))
    }
}