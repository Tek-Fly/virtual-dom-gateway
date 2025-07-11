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
                .unwrap_or_else(|_| "memory_gateway=debug,tower_http=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer().json())
        .init();

    info!("Starting Tekfly Virtual-DOM Gateway");

    // Load configuration
    let config = Config::from_env()?;
    info!("Configuration loaded");

    // Initialize MongoDB connection
    let db_client = db::connect(&config.mongodb_uri).await?;
    info!("Connected to MongoDB");

    // Initialize metrics
    let _metrics_registry = metrics::init();
    
    // Create service
    let service = MemoryGatewayService::new(db_client, config.clone());
    
    // Create gRPC server
    let addr: SocketAddr = format!("{}:{}", config.host, config.port).parse()?;
    
    info!("Starting gRPC server on {}", addr);

    // Configure TLS with Kyber768 + X25519
    let tls_config = security::create_tls_config(&config)?;

    Server::builder()
        .tls_config(tls_config)?
        .add_service(MemoryGatewayServer::new(service))
        .serve(addr)
        .await?;

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