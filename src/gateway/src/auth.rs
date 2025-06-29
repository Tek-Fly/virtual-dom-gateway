use jsonwebtoken::{decode, decode_header, Algorithm, DecodingKey, Validation};
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AuthError {
    #[error("Invalid token format")]
    InvalidFormat,
    
    #[error("Token validation failed: {0}")]
    ValidationFailed(String),
    
    #[error("Missing required scope: {0}")]
    MissingScope(String),
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Claims {
    pub sub: String,           // Subject (user ID)
    pub exp: usize,           // Expiration time
    pub iat: usize,           // Issued at
    pub scopes: Vec<String>,  // Permission scopes
    pub email: Option<String>,
    pub org: Option<String>,  // Organization
}

/// Validate JWT token and extract claims
pub fn validate_token(token: &str, secret: &str) -> Result<Claims, AuthError> {
    let validation = Validation::new(Algorithm::HS512);
    
    let token_data = decode::<Claims>(
        token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &validation,
    )
    .map_err(|e| AuthError::ValidationFailed(e.to_string()))?;
    
    Ok(token_data.claims)
}

/// Generate a development token for testing
#[cfg(debug_assertions)]
pub fn generate_dev_token(secret: &str) -> Result<String, AuthError> {
    use jsonwebtoken::{encode, EncodingKey, Header};
    
    let claims = Claims {
        sub: "dev-user".to_string(),
        exp: (chrono::Utc::now() + chrono::Duration::hours(24)).timestamp() as usize,
        iat: chrono::Utc::now().timestamp() as usize,
        scopes: vec!["dom.read".to_string(), "dom.write".to_string()],
        email: Some("dev@tekfly.io".to_string()),
        org: Some("tekfly".to_string()),
    };
    
    let header = Header::new(Algorithm::HS512);
    encode(&header, &claims, &EncodingKey::from_secret(secret.as_bytes()))
        .map_err(|e| AuthError::ValidationFailed(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_token_validation() {
        let secret = "test-secret";
        let token = generate_dev_token(secret).unwrap();
        let claims = validate_token(&token, secret).unwrap();
        
        assert_eq!(claims.sub, "dev-user");
        assert!(claims.scopes.contains(&"dom.read".to_string()));
        assert!(claims.scopes.contains(&"dom.write".to_string()));
    }

    #[test]
    fn test_invalid_token() {
        let result = validate_token("invalid-token", "secret");
        assert!(result.is_err());
    }
}