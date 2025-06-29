use anyhow::Result;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub host: String,
    pub port: u16,
    pub mongodb_uri: String,
    pub mongodb_database: String,
    pub jwt_secret: String,
    pub tls_cert_path: Option<String>,
    pub tls_key_path: Option<String>,
    pub enable_metrics: bool,
    pub metrics_port: u16,
    pub log_level: String,
}

impl Config {
    /// Load configuration from environment variables
    pub fn from_env() -> Result<Self> {
        Ok(Self {
            host: std::env::var("GATEWAY_HOST").unwrap_or_else(|_| "0.0.0.0".to_string()),
            port: std::env::var("GATEWAY_PORT")
                .unwrap_or_else(|_| "50051".to_string())
                .parse()?,
            mongodb_uri: std::env::var("MONGODB_URI")
                .unwrap_or_else(|_| "mongodb://localhost:27017".to_string()),
            mongodb_database: std::env::var("MONGODB_DATABASE")
                .unwrap_or_else(|_| "virtual_dom".to_string()),
            jwt_secret: std::env::var("JWT_SECRET")
                .unwrap_or_else(|_| "development-secret-change-in-production".to_string()),
            tls_cert_path: std::env::var("TLS_CERT_PATH").ok(),
            tls_key_path: std::env::var("TLS_KEY_PATH").ok(),
            enable_metrics: std::env::var("ENABLE_METRICS")
                .unwrap_or_else(|_| "true".to_string())
                .parse()?,
            metrics_port: std::env::var("METRICS_PORT")
                .unwrap_or_else(|_| "9090".to_string())
                .parse()?,
            log_level: std::env::var("LOG_LEVEL").unwrap_or_else(|_| "info".to_string()),
        })
    }

    /// Validate configuration
    pub fn validate(&self) -> Result<()> {
        if self.jwt_secret == "development-secret-change-in-production" {
            tracing::warn!("Using default JWT secret - change in production!");
        }

        if self.tls_cert_path.is_some() != self.tls_key_path.is_some() {
            anyhow::bail!("Both TLS cert and key must be provided");
        }

        Ok(())
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            host: "0.0.0.0".to_string(),
            port: 50051,
            mongodb_uri: "mongodb://localhost:27017".to_string(),
            mongodb_database: "virtual_dom".to_string(),
            jwt_secret: "development-secret-change-in-production".to_string(),
            tls_cert_path: None,
            tls_key_path: None,
            enable_metrics: true,
            metrics_port: 9090,
            log_level: "info".to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = Config::default();
        assert_eq!(config.host, "0.0.0.0");
        assert_eq!(config.port, 50051);
        assert_eq!(config.mongodb_database, "virtual_dom");
    }

    #[test]
    fn test_config_validation() {
        let mut config = Config::default();
        assert!(config.validate().is_ok());

        // Test invalid TLS config
        config.tls_cert_path = Some("cert.pem".to_string());
        config.tls_key_path = None;
        assert!(config.validate().is_err());
    }
}