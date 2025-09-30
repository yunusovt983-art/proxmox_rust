//! Simple HTTP server for testing the network and SDN API

use axum::{extract::State, http::StatusCode, response::Json, routing::get, Router};
use chrono::Utc;
use serde_json::json;
use std::net::SocketAddr;
use std::sync::Arc;
use tower::ServiceBuilder;
use tower_http::{cors::CorsLayer, trace::TraceLayer};

use pve_network_api::{
    context::AppContext, ContainerNetworkAPI, NetworkAPI, StorageNetworkAPI, SDNAPI,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    env_logger::init();

    // Initialize shared application context
    let context = AppContext::bootstrap().await?;

    // Build the application router
    let app = Router::new()
        .merge(NetworkAPI::router())
        .merge(StorageNetworkAPI::router())
        .merge(ContainerNetworkAPI::router())
        .merge(SDNAPI::router())
        .route("/", get(root))
        .route("/health", get(health_check))
        .route("/metrics/migration", get(migration_metrics))
        .layer(
            ServiceBuilder::new()
                .layer(TraceLayer::new_for_http())
                .layer(CorsLayer::permissive()),
        )
        .with_state(context.clone());

    // Start the server
    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    println!("Starting API server on http://{}", addr);
    println!("Available endpoints:");
    println!("  Network API:");
    println!("    GET /api2/json/nodes/{{node}}/network");
    println!("    GET /api2/json/nodes/{{node}}/network/{{iface}}");
    println!("    GET /api2/json/nodes/{{node}}/network/{{iface}}/status");
    println!("  Storage API:");
    println!("    GET /api2/json/nodes/{{node}}/storage/network");
    println!("    POST /api2/json/nodes/{{node}}/storage/network/{{storage}}");
    println!("    DELETE /api2/json/nodes/{{node}}/storage/network/{{storage}}");
    println!("    POST /api2/json/nodes/{{node}}/storage/network/{{storage}}/validate");
    println!("    GET /api2/json/nodes/{{node}}/storage/vlan");
    println!("    POST /api2/json/nodes/{{node}}/storage/vlan/{{storage}}");
    println!("  SDN API:");
    println!("    GET /sdn/zones");
    println!("    POST /sdn/zones");
    println!("    GET /sdn/zones/{{zone}}");
    println!("    PUT /sdn/zones/{{zone}}");
    println!("    DELETE /sdn/zones/{{zone}}");
    println!("    GET /sdn/vnets");
    println!("    POST /sdn/vnets");
    println!("    GET /sdn/vnets/{{vnet}}");
    println!("    PUT /sdn/vnets/{{vnet}}");
    println!("    DELETE /sdn/vnets/{{vnet}}");
    println!("    GET /sdn/subnets");
    println!("    POST /sdn/subnets");
    println!("    GET /sdn/subnets/{{subnet}}");
    println!("    PUT /sdn/subnets/{{subnet}}");
    println!("    DELETE /sdn/subnets/{{subnet}}");
    println!("    GET /sdn/config");
    println!("    PUT /sdn/config");
    println!("    POST /sdn/reload");
    println!("  Container API:");
    println!("    GET /api2/json/nodes/{{node}}/lxc/{{vmid}}/network");
    println!("    POST /api2/json/nodes/{{node}}/lxc/{{vmid}}/network");
    println!("    GET /api2/json/nodes/{{node}}/lxc/{{vmid}}/network/config");
    println!("    PUT /api2/json/nodes/{{node}}/lxc/{{vmid}}/network/{{iface}}");
    println!("    DELETE /api2/json/nodes/{{node}}/lxc/{{vmid}}/network/{{iface}}");
    println!("    POST /api2/json/nodes/{{node}}/lxc/{{vmid}}/network/{{iface}}/hotplug");
    println!("    GET /api2/json/nodes/{{node}}/lxc/{{vmid}}/network/stats");
    println!("  Other:");
    println!("    GET /health");
    println!("    GET /metrics/migration");

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

/// Root endpoint
async fn root() -> Json<serde_json::Value> {
    Json(json!({
        "name": "PVE Network API (Rust)",
        "version": "0.1.0",
        "features": ["network", "sdn", "storage", "container"],
        "endpoints": {
            "network": [
                "GET /api2/json/nodes/{node}/network",
                "GET /api2/json/nodes/{node}/network/{iface}",
                "GET /api2/json/nodes/{node}/network/{iface}/status"
            ],
            "storage": [
                "GET /api2/json/nodes/{node}/storage/network",
                "POST /api2/json/nodes/{node}/storage/network/{storage}",
                "DELETE /api2/json/nodes/{node}/storage/network/{storage}",
                "POST /api2/json/nodes/{node}/storage/network/{storage}/validate",
                "GET /api2/json/nodes/{node}/storage/vlan",
                "POST /api2/json/nodes/{node}/storage/vlan/{storage}",
                "DELETE /api2/json/nodes/{node}/storage/vlan/{storage}",
                "GET /api2/json/nodes/{node}/storage/path/{storage}?path=<path>"
            ],
            "sdn": [
                "GET /sdn/zones",
                "POST /sdn/zones",
                "GET /sdn/zones/{zone}",
                "PUT /sdn/zones/{zone}",
                "DELETE /sdn/zones/{zone}",
                "GET /sdn/vnets",
                "POST /sdn/vnets",
                "GET /sdn/vnets/{vnet}",
                "PUT /sdn/vnets/{vnet}",
                "DELETE /sdn/vnets/{vnet}",
                "GET /sdn/subnets",
                "POST /sdn/subnets",
                "GET /sdn/subnets/{subnet}",
                "PUT /sdn/subnets/{subnet}",
                "DELETE /sdn/subnets/{subnet}",
                "GET /sdn/config",
                "PUT /sdn/config",
                "POST /sdn/reload"
            ],
            "container": [
                "GET /api2/json/nodes/{node}/lxc/{vmid}/network",
                "POST /api2/json/nodes/{node}/lxc/{vmid}/network",
                "GET /api2/json/nodes/{node}/lxc/{vmid}/network/config",
                "PUT /api2/json/nodes/{node}/lxc/{vmid}/network/{iface}",
                "DELETE /api2/json/nodes/{node}/lxc/{vmid}/network/{iface}",
                "POST /api2/json/nodes/{node}/lxc/{vmid}/network/{iface}/hotplug",
                "DELETE /api2/json/nodes/{node}/lxc/{vmid}/network/{iface}/hotplug",
                "GET /api2/json/nodes/{node}/lxc/{vmid}/network/stats"
            ]
        }
    }))
}

/// Health check endpoint
async fn health_check(
    State(context): State<Arc<AppContext>>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let phase = context.migration_hooks.current_phase().await;
    let applied = context.migration_hooks.network_applied_count().await;

    Ok(Json(json!({
        "status": "healthy",
        "timestamp": Utc::now().to_rfc3339(),
        "components": {
            "network_api": "ok",
            "sdn_api": "ok",
            "storage_api": "ok",
            "container_api": "ok"
        },
        "migration": {
            "phase": phase,
            "network_applied_events": applied
        }
    })))
}

async fn migration_metrics(
    State(context): State<Arc<AppContext>>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let phase = context.migration_hooks.current_phase().await;
    let applied = context.migration_hooks.network_applied_count().await;

    Ok(Json(json!({
        "phase": phase,
        "network_applied_events": applied
    })))
}
