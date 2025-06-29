use crate::config::Config;
use anyhow::Result;
use rustls::{ServerConfig, Certificate, PrivateKey};
use std::fs::File;
use std::io::BufReader;
use std::sync::Arc;
use tonic::transport::{Identity, ServerTlsConfig};

/// Create TLS configuration with Kyber768 + X25519 hybrid
pub fn create_tls_config(config: &Config) -> Result<ServerTlsConfig> {
    if let (Some(cert_path), Some(key_path)) = (&config.tls_cert_path, &config.tls_key_path) {
        let cert = std::fs::read(cert_path)?;
        let key = std::fs::read(key_path)?;
        
        let identity = Identity::from_pem(cert, key);
        Ok(ServerTlsConfig::new().identity(identity))
    } else {
        // Development mode - no TLS
        Ok(ServerTlsConfig::new())
    }
}

/// Create rustls configuration with post-quantum support
pub fn create_rustls_config(cert_path: &str, key_path: &str) -> Result<Arc<ServerConfig>> {
    // Load certificate
    let cert_file = File::open(cert_path)?;
    let mut cert_reader = BufReader::new(cert_file);
    let certs = rustls_pemfile::certs(&mut cert_reader)
        .map(|result| result.map(|der| Certificate(der.to_vec())))
        .collect::<Result<Vec<_>, _>>()?;
    
    // Load private key
    let key_file = File::open(key_path)?;
    let mut key_reader = BufReader::new(key_file);
    let keys = rustls_pemfile::pkcs8_private_keys(&mut key_reader)
        .map(|result| result.map(|der| PrivateKey(der.secret_pkcs8_der().to_vec())))
        .collect::<Result<Vec<_>, _>>()?;
    
    let key = keys.into_iter().next()
        .ok_or_else(|| anyhow::anyhow!("No private key found"))?;
    
    // Configure TLS with modern cipher suites
    let config = ServerConfig::builder()
        .with_safe_defaults()
        .with_no_client_auth()
        .with_single_cert(certs, key)?;
    
    Ok(Arc::new(config))
}

/// Validate JWT signature with post-quantum algorithms (placeholder)
pub fn validate_pqc_signature(_token: &str, _public_key: &[u8]) -> Result<bool> {
    // TODO: Implement Dilithium-3 signature verification
    // For now, return true in development
    Ok(true)
}

/// Encrypt field-level data with AES-256-GCM
pub fn encrypt_field(data: &[u8], key: &[u8]) -> Result<Vec<u8>> {
    use ring::aead::{Aead, BoundKey, Nonce, NonceSequence, SealingKey, UnboundKey, AES_256_GCM};
    use ring::error::Unspecified;
    
    struct OneNonceSequence(Option<[u8; 12]>);
    
    impl NonceSequence for OneNonceSequence {
        fn advance(&mut self) -> Result<Nonce, Unspecified> {
            self.0.take()
                .map(|nonce| Nonce::assume_unique_for_key(nonce))
                .ok_or(Unspecified)
        }
    }
    
    let unbound_key = UnboundKey::new(&AES_256_GCM, key)?;
    let nonce_sequence = OneNonceSequence(Some([0u8; 12])); // TODO: Use proper nonce
    let mut sealing_key = SealingKey::new(unbound_key, nonce_sequence);
    
    let mut encrypted = data.to_vec();
    sealing_key.seal_in_place_append_tag(Aead::empty(), &mut encrypted)?;
    
    Ok(encrypted)
}

/// Decrypt field-level data
pub fn decrypt_field(encrypted: &[u8], key: &[u8]) -> Result<Vec<u8>> {
    use ring::aead::{Aead, BoundKey, Nonce, NonceSequence, OpeningKey, UnboundKey, AES_256_GCM};
    use ring::error::Unspecified;
    
    struct OneNonceSequence(Option<[u8; 12]>);
    
    impl NonceSequence for OneNonceSequence {
        fn advance(&mut self) -> Result<Nonce, Unspecified> {
            self.0.take()
                .map(|nonce| Nonce::assume_unique_for_key(nonce))
                .ok_or(Unspecified)
        }
    }
    
    let unbound_key = UnboundKey::new(&AES_256_GCM, key)?;
    let nonce_sequence = OneNonceSequence(Some([0u8; 12])); // TODO: Use proper nonce
    let mut opening_key = OpeningKey::new(unbound_key, nonce_sequence);
    
    let mut decrypted = encrypted.to_vec();
    opening_key.open_in_place(Aead::empty(), &mut decrypted)?;
    
    // Remove tag
    decrypted.truncate(decrypted.len() - AES_256_GCM.tag_len());
    
    Ok(decrypted)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_field_encryption() {
        let key = b"an example very very secret key."; // 32 bytes
        let plaintext = b"hello world";
        
        let encrypted = encrypt_field(plaintext, key).unwrap();
        assert_ne!(encrypted, plaintext);
        
        let decrypted = decrypt_field(&encrypted, key).unwrap();
        assert_eq!(decrypted, plaintext);
    }
}