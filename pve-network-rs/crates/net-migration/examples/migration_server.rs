//! Example migration server demonstrating the middleware in action
//!
//! This example shows how to set up a server with migration middleware
//! that can route requests between Rust and Perl implementations.

use hyper::service::{make_service_fn, service_fn};
use hyper::{Body, Request, Response, Server};
use net_migration::{
    middleware::{MockRustApiHandler, RustApiHandler},
    perl_client::{ApiRequest, ApiResponse, HttpPerlApiClient},
    MigrationConfig, MigrationMiddleware,
};
use serde_json::json;
use std::collections::HashMap;
use std::convert::Infallible;
use std::sync::Arc;
use std::time::Duration;

#[derive(Clone)]
struct ExampleRustHandler;

#[async_trait::async_trait]
impl RustApiHandler for ExampleRustHandler {
    async fn handle_request(&self, request: &ApiRequest) -> net_migration::Result<ApiResponse> {
        println!(
            "Rust handler processing: {} {}",
            request.method, request.path
        );

        // Simulate some processing time
        tokio::time::sleep(Duration::from_millis(50)).await;

        let response_data = match request.path.as_str() {
            "/api2/json/nodes/test/network" => {
                json!({
                    "data": [
                        {
                            "iface": "eth0",
                            "type": "eth",
                            "method": "static",
                            "address": "192.168.1.10/24",
                            "gateway": "192.168.1.1",
                            "active": 1
                        },
                        {
                            "iface": "vmbr0",
                            "type": "bridge",
                            "method": "static",
                            "address": "10.0.0.1/24",
                            "bridge_ports": "eth1",
                            "active": 1
                        }
                    ]
                })
            }
            "/api2/json/sdn/zones" => {
                json!({
                    "data": [
                        {
                            "zone": "localnetwork",
                            "type": "simple",
                            "bridge": "vmbr0"
                        }
                    ]
                })
            }
            _ => {
                json!({
                    "data": null,
                    "message": "Rust implementation for this endpoint"
                })
            }
        };

        Ok(ApiResponse {
            status: 200,
            headers: HashMap::new(),
            body: response_data,
        })
    }
}

async fn handle_request(
    req: Request<Body>,
    middleware: Arc<MigrationMiddleware>,
) -> Result<Response<Body>, Infallible> {
    match middleware.handle_request(req).await {
        Ok(response) => Ok(response),
        Err(e) => {
            eprintln!("Request handling error: {}", e);
            let error_response = Response::builder()
                .status(500)
                .body(Body::from(format!("Internal server error: {}", e)))
                .unwrap();
            Ok(error_response)
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    println!("Starting PVE Network Migration Server...");

    // Create migration configuration
    let mut config = MigrationConfig::default();

    // Set migration phase from environment or default to read-only
    let phase = std::env::var("MIGRATION_PHASE")
        .unwrap_or_else(|_| "rust-read-only".to_string())
        .parse()
        .unwrap_or(net_migration::MigrationPhase::RustReadOnly);

    config.phase = phase;
    config.update_endpoints_for_phase();

    println!("Migration phase: {:?}", config.phase);
    println!("Fallback enabled: {}", config.fallback_enabled);

    // Create Perl client (pointing to existing Perl implementation)
    let perl_base_url =
        std::env::var("PERL_API_URL").unwrap_or_else(|_| "http://localhost:8006".to_string());

    let perl_client = Arc::new(HttpPerlApiClient::new(
        perl_base_url,
        Duration::from_secs(30),
    ));

    // Create Rust handler
    let rust_handler = Arc::new(ExampleRustHandler);

    // Create middleware
    let middleware = Arc::new(MigrationMiddleware::new(config, rust_handler, perl_client));

    // Create service
    let middleware_clone = Arc::clone(&middleware);
    let make_svc = make_service_fn(move |_conn| {
        let middleware = Arc::clone(&middleware_clone);
        async move {
            Ok::<_, Infallible>(service_fn(move |req| {
                handle_request(req, Arc::clone(&middleware))
            }))
        }
    });

    // Start server
    let addr = ([127, 0, 0, 1], 3000).into();
    let server = Server::bind(&addr).serve(make_svc);

    println!("Migration server running on http://{}", addr);
    println!("Try these endpoints:");
    println!("  GET  http://localhost:3000/api2/json/nodes/test/network");
    println!("  GET  http://localhost:3000/api2/json/sdn/zones");
    println!("  GET  http://localhost:3000/health");

    // Add health endpoint handler
    if let Err(e) = server.await {
        eprintln!("Server error: {}", e);
    }

    Ok(())
}

// Health check endpoint example
async fn health_endpoint(middleware: Arc<MigrationMiddleware>) -> Response<Body> {
    let health = middleware.health_check().await;
    let health_json = serde_json::to_string_pretty(&health).unwrap();

    Response::builder()
        .status(200)
        .header("Content-Type", "application/json")
        .body(Body::from(health_json))
        .unwrap()
}
