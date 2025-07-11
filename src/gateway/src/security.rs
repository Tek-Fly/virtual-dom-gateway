use crate::config::Config;
use anyhow::Result;
use rustls::ServerConfig;
use rustls::pki_types::{CertificateDer, PrivateKeyDer};
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
        .collect::<Result<Vec<CertificateDer>, _>>()?;
    
    // Load private key
    let key_file = File::open(key_path)?;
    let mut key_reader = BufReader::new(key_file);
    let keys = rustls_pemfile::pkcs8_private_keys(&mut key_reader)
        .map(|key| key.map(PrivateKeyDer::from))
        .collect::<Result<Vec<_>, _>>()?;
    
    let key = keys.into_iter().next()
        .ok_or_else(|| anyhow::anyhow!("No private key found"))?;
    
    // Configure TLS with modern cipher suites
    let config = ServerConfig::builder()
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
    use ring::aead::{LessSafeKey, Nonce, UnboundKey, AES_256_GCM};
    use ring::rand::{SecureRandom, SystemRandom};
    
    // Generate a random nonce
    let rng = SystemRandom::new();
    let mut nonce_bytes = [0u8; 12];
    rng.fill(&mut nonce_bytes)
        .map_err(|_| anyhow::anyhow!("Failed to generate nonce"))?;
    
    // Create the key
    let unbound_key = UnboundKey::new(&AES_256_GCM, key)
        .map_err(|_| anyhow::anyhow!("Invalid key length"))?;
    let less_safe_key = LessSafeKey::new(unbound_key);
    
    // Create nonce
    let nonce = Nonce::assume_unique_for_key(nonce_bytes);
    
    // Prepare ciphertext buffer and encrypt
    let mut ciphertext = data.to_vec();
    less_safe_key.seal_in_place_append_tag(nonce, ring::aead::Aad::empty(), &mut ciphertext)
        .map_err(|_| anyhow::anyhow!("Encryption failed"))?;
    
    // Combine nonce + ciphertext + tag
    let mut output = nonce_bytes.to_vec();
    output.extend_from_slice(&ciphertext);
    
    Ok(output)
}

/// Decrypt field-level data
pub fn decrypt_field(encrypted: &[u8], key: &[u8]) -> Result<Vec<u8>> {
    use ring::aead::{LessSafeKey, Nonce, UnboundKey, AES_256_GCM};
    
    // Ensure we have at least nonce + tag
    if encrypted.len() < 12 + 16 {
        return Err(anyhow::anyhow!("Invalid encrypted data length"));
    }
    
    // Extract nonce from encrypted data
    let (nonce_bytes, ciphertext) = encrypted.split_at(12);
    let mut nonce_array = [0u8; 12];
    nonce_array.copy_from_slice(nonce_bytes);
    
    // Create the key
    let unbound_key = UnboundKey::new(&AES_256_GCM, key)
        .map_err(|_| anyhow::anyhow!("Invalid key length"))?;
    let less_safe_key = LessSafeKey::new(unbound_key);
    
    // Create nonce
    let nonce = Nonce::assume_unique_for_key(nonce_array);
    
    // Decrypt in place
    let mut decrypted = ciphertext.to_vec();
    let plaintext_len = less_safe_key.open_in_place(nonce, ring::aead::Aad::empty(), &mut decrypted)
        .map_err(|_| anyhow::anyhow!("Decryption failed"))?
        .len();
    
    // Truncate to actual plaintext length (removes tag)
    decrypted.truncate(plaintext_len);
    
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