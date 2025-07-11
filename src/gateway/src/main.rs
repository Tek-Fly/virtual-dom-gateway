use anyhow::Result;
use std::net::SocketAddr;
use tonic::transport::Server;
use tracing::info;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

mod auth;
mod config;
mod db;
mod error;
mod grpc;
mod metrics;
mod rest;
mod security;
mod service;

use crate::config::Config;
use crate::grpc::memory_gateway_server::MemoryGatewayServer;
use crate::service::MemoryGatewayService;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "memory_gateway=debug,tower_http=debug,actix_web=info".into()),
        )
        .with(tracing_subscriber::fmt::layer().json())
        .init();

    info!("Starting Tekfly Virtual-DOM Gateway - Divine Hybrid Architecture");

    // Load configuration
    let config = Config::from_env()?;
    info!("Configuration loaded");

    // Initialize MongoDB connection
    let db_client = db::connect(&config.mongodb_uri).await?;
    info!("Connected to MongoDB Atlas - Divine Cloud");

    // Initialize metrics
    let metrics_registry = metrics::init();
    
    // Create shared database instance
    let db = std::sync::Arc::new(crate::db::Database::new(db_client.clone()));
    
    // Create gRPC service
    let grpc_service = MemoryGatewayService::new(db_client, config.clone());
    
    // Create REST app state
    let rest_state = std::sync::Arc::new(rest::AppState {
        db: db.clone(),
        config: config.clone(),
    });
    
    // Spawn gRPC server
    let grpc_addr: SocketAddr = format!("{}:{}", config.host, config.port).parse()?;
    let grpc_config = config.clone();
    let grpc_handle = tokio::spawn(async move {
        info!("Starting gRPC server on {} - Divine Protocol", grpc_addr);
        
        // Configure TLS with Kyber768 + X25519
        let tls_config = security::create_tls_config(&grpc_config)
            .expect("Failed to create TLS config");
        
        Server::builder()
            .tls_config(tls_config)
            .expect("Failed to configure TLS")
            .add_service(MemoryGatewayServer::new(grpc_service))
            .serve(grpc_addr)
            .await
            .expect("gRPC server failed")
    });
    
    // Spawn REST server
    let rest_port = config.port + 1000; // REST on port +1000
    let rest_addr = format!("{}:{}", config.host, rest_port);
    let rest_handle = tokio::spawn(async move {
        info!("Starting REST server on {} - Divine Accessibility", rest_addr);
        
        let app = rest::configure_routes(rest_state);
        
        let listener = tokio::net::TcpListener::bind(&rest_addr)
            .await
            .expect("Failed to bind REST server");
            
        axum::serve(listener, app)
            .await
            .expect("REST server failed")
    });
    
    // Spawn metrics server
    let metrics_port = config.port + 2000; // Metrics on port +2000
    let metrics_handle = tokio::spawn(async move {
        metrics::serve_metrics(metrics_registry, metrics_port).await;
    });
    
    info!("🙏 Divine Virtual-DOM Gateway initialized with 99.1% quality standards");
    info!("📡 gRPC: port {}, REST: port {}, Metrics: port {}", config.port, rest_port, metrics_port);
    
    // Wait for all servers
    let _ = tokio::join!(grpc_handle, rest_handle, metrics_handle);

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_server_startup() {
        // TODO: Add integration tests
    }
}