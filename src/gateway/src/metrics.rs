use prometheus::{Encoder, IntCounter, IntGauge, Registry, TextEncoder};
use std::sync::Arc;
use tracing::info;

lazy_static::lazy_static! {
    pub static ref WRITE_REQUESTS: IntCounter = IntCounter::new("virtualdom_write_requests_total", "Total write requests")
        .expect("metric can be created");
    
    pub static ref WRITE_SUCCESS: IntCounter = IntCounter::new("virtualdom_write_success_total", "Successful writes")
        .expect("metric can be created");
    
    pub static ref WRITE_CONFLICTS: IntCounter = IntCounter::new("virtualdom_write_conflicts_total", "Write conflicts")
        .expect("metric can be created");
    
    pub static ref WRITE_ERRORS: IntCounter = IntCounter::new("virtualdom_write_errors_total", "Write errors")
        .expect("metric can be created");
    
    pub static ref READ_REQUESTS: IntCounter = IntCounter::new("virtualdom_read_requests_total", "Total read requests")
        .expect("metric can be created");
    
    pub static ref READ_SUCCESS: IntCounter = IntCounter::new("virtualdom_read_success_total", "Successful reads")
        .expect("metric can be created");
    
    pub static ref READ_NOT_FOUND: IntCounter = IntCounter::new("virtualdom_read_not_found_total", "Read not found")
        .expect("metric can be created");
    
    pub static ref READ_ERRORS: IntCounter = IntCounter::new("virtualdom_read_errors_total", "Read errors")
        .expect("metric can be created");
    
    pub static ref ACTIVE_SUBSCRIPTIONS: IntGauge = IntGauge::new("virtualdom_active_subscriptions", "Active change subscriptions")
        .expect("metric can be created");
}

pub fn init() -> Arc<Registry> {
    let registry = Arc::new(Registry::new());
    
    registry
        .register(Box::new(WRITE_REQUESTS.clone()))
        .expect("collector can be registered");
    
    registry
        .register(Box::new(WRITE_SUCCESS.clone()))
        .expect("collector can be registered");
    
    registry
        .register(Box::new(WRITE_CONFLICTS.clone()))
        .expect("collector can be registered");
    
    registry
        .register(Box::new(WRITE_ERRORS.clone()))
        .expect("collector can be registered");
    
    registry
        .register(Box::new(READ_REQUESTS.clone()))
        .expect("collector can be registered");
    
    registry
        .register(Box::new(READ_SUCCESS.clone()))
        .expect("collector can be registered");
    
    registry
        .register(Box::new(READ_NOT_FOUND.clone()))
        .expect("collector can be registered");
    
    registry
        .register(Box::new(READ_ERRORS.clone()))
        .expect("collector can be registered");
    
    registry
        .register(Box::new(ACTIVE_SUBSCRIPTIONS.clone()))
        .expect("collector can be registered");
    
    info!("Metrics registry initialized");
    registry
}

pub async fn serve_metrics(registry: Arc<Registry>, port: u16) {
    use axum::{routing::get, Router};
    
    let app = Router::new().route("/metrics", get(move || {
        let registry = registry.clone();
        async move {
            let encoder = TextEncoder::new();
            let metric_families = registry.gather();
            let mut buffer = Vec::new();
            encoder.encode(&metric_families, &mut buffer).unwrap();
            String::from_utf8(buffer).unwrap()
        }
    }));
    
    let addr = format!("0.0.0.0:{}", port);
    info!("Metrics server listening on {}", addr);
    
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}