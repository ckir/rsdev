//! # Cloud Configuration Module
//!
//! This module provides utilities to retrieve, decrypt, and parse configuration
//! files stored in cloud environments. It supports AES-256-CBC decryption and
//! utilizes static initialization to cache the configuration in memory.

use aes::Aes256;
use base64::{engine::general_purpose, Engine as _};
use cbc::Decryptor;
use cipher::{block_padding::Pkcs7, BlockDecryptMut, KeyIvInit};
use reqwest::blocking::Client;
use serde_json::Value;
use static_init::dynamic;
use std::env;
use thiserror::Error;

/// Errors that can occur during the cloud configuration lifecycle.
#[derive(Debug, Error, Clone)]
pub enum CloudConfigError {
    /// Errors related to standard environment variable access.
    #[error("Environment variable error: {0}")]
    VarError(#[from] env::VarError),

    /// Errors specifically for when a required environment variable is not set.
    #[error("Missing environment variable: {0}")]
    MissingEnvVar(String),

    /// Errors occurring during HTTP requests or network connectivity.
    #[error("Network error: {0}")]
    NetworkError(String),

    /// Errors occurring during the AES decryption process or key validation.
    #[error("Decryption error: {0}")]
    DecryptionError(String),

    /// Errors resulting from malformed JSON content within the decrypted data.
    #[error("JSON parse error: {0}")]
    JsonError(String),

    /// Errors related to the structure of the retrieved file (e.g., missing IV or ciphertext).
    #[error("Invalid data format: {0}")]
    InvalidData(String),
}

/// Global static storage for the cloud configuration.
///
/// This is initialized exactly once upon first access or program start.
#[dynamic]
static CLOUD_CONFIG: Result<Value, CloudConfigError> = load_cloud_config(None, None);

/// Retrieves and decrypts the configuration from a remote URL.
///
/// # Arguments
///
/// * `url` - An optional URL override. Defaults to `WEBLIB_CLOUD_CONFIG_URL`.
/// * `password` - An optional hex-encoded 32-byte key. Defaults to `WEBLIB_AES_PASSWORD`.
///
/// # Returns
///
/// Returns a `Result` containing the parsed `serde_json::Value` or a `CloudConfigError`.
pub fn load_cloud_config(
    url: Option<String>,
    password: Option<String>,
) -> Result<Value, CloudConfigError> {
    // Resolve decryption password from arguments or environment.
    let password = password
        .or_else(|| env::var("WEBLIB_AES_PASSWORD").ok())
        .ok_or_else(|| CloudConfigError::MissingEnvVar("WEBLIB_AES_PASSWORD".to_string()))?;

    let password = password.trim();

    // Resolve target URL for the configuration file.
    let url = url
        .or_else(|| env::var("WEBLIB_CLOUD_CONFIG_URL").ok())
        .ok_or_else(|| CloudConfigError::MissingEnvVar("WEBLIB_CLOUD_CONFIG_URL".to_string()))?;

    // Initialize blocking HTTP client.
    let client = Client::new();

    // Execute GET request; explicit type reqwest::Error added for stability.
    let response = client
        .get(&url)
        .send()
        .map_err(|e: reqwest::Error| CloudConfigError::NetworkError(e.to_string()))?;

    if !response.status().is_success() {
        return Err(CloudConfigError::NetworkError(format!(
            "HTTP request failed with status: {}",
            response.status()
        )));
    }

    // Extract raw text content.
    let content = response
        .text()
        .map_err(|e: reqwest::Error| CloudConfigError::NetworkError(e.to_string()))?;

    // Delegate decryption and parsing to the pure logic function.
    decrypt_and_parse(&content, password)
}

/// Decrypts and parses the configuration content.
///
/// This function encapsulates the core logic of splitting the content into IV and ciphertext,
/// decoding from Base64, decrypting using AES-256-CBC, and parsing the result as JSON.
///
/// # Arguments
///
/// * `content` - The raw text content retrieved from the cloud source (IV \n Ciphertext).
/// * `password` - The hex-encoded 32-byte decryption key.
///
/// # Returns
///
/// Returns a `Result` containing the parsed `serde_json::Value` or a `CloudConfigError`.
pub fn decrypt_and_parse(content: &str, password: &str) -> Result<Value, CloudConfigError> {
    // Parse file lines (Expected: Line 1 = IV, Line 2 = Ciphertext).
    let lines: Vec<&str> = content
        .lines()
        .map(|l: &str| l.trim())
        .filter(|l: &&str| !l.is_empty())
        .collect();

    if lines.len() < 2 {
        return Err(CloudConfigError::InvalidData(format!(
            "File format error: expected at least 2 lines, found {}",
            lines.len()
        )));
    }

    let iv_base64 = lines[0];
    let ciphertext_base64 = lines[1];

    // Decode Base64 initialization vector and ciphertext.
    let iv = general_purpose::STANDARD
        .decode(iv_base64)
        .map_err(|e| CloudConfigError::InvalidData(format!("Invalid Base64 IV: {}", e)))?;

    let ciphertext = general_purpose::STANDARD
        .decode(ciphertext_base64)
        .map_err(|e| CloudConfigError::InvalidData(format!("Invalid Base64 Ciphertext: {}", e)))?;

    // Hex-decode password and validate 32-byte length for AES-256.
    let key_vec = hex::decode(password)
        .map_err(|e| CloudConfigError::DecryptionError(format!("Invalid Key Hex: {}", e)))?;

    if key_vec.len() != 32 {
        return Err(CloudConfigError::DecryptionError(format!(
            "Key must be 32 bytes, found {}",
            key_vec.len()
        )));
    }

    let key_arr: [u8; 32] = key_vec.try_into().expect("Length checked above");
    let iv_arr: [u8; 16] = iv
        .as_slice()
        .try_into()
        .map_err(|_| CloudConfigError::InvalidData(format!("Invalid IV length: {}", iv.len())))?;

    // Decrypt buffer and remove PKCS7 padding.
    let decryptor = Decryptor::<Aes256>::new(&key_arr.into(), &iv_arr.into());
    let mut buf = ciphertext.to_vec();
    if buf.is_empty() {
        return Err(CloudConfigError::DecryptionError(
            "Ciphertext is empty".to_string(),
        ));
    }

    let decrypted_data = decryptor
        .decrypt_padded_mut::<Pkcs7>(&mut buf)
        .map_err(|e| {
            CloudConfigError::DecryptionError(format!(
                "Decryption failed: {:?}. Verify the decryption key.",
                e
            ))
        })?;

    // Parse final decrypted bytes as JSON.
    let json: Value = serde_json::from_slice(decrypted_data)
        .map_err(|e| CloudConfigError::JsonError(e.to_string()))?;

    Ok(json)
}

/// Provides access to the globally cached cloud configuration.
///
/// # Returns
///
/// Returns a clone of the `serde_json::Value` or a `CloudConfigError`.
pub fn get_cloud_config() -> Result<Value, CloudConfigError> {
    // Access static result via dereference.
    match &*CLOUD_CONFIG {
        Ok(val) => Ok(val.clone()),
        Err(e) => Err(e.clone()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use aes::cipher::{BlockEncryptMut, KeyIvInit};
    use cbc::Encryptor;

    /// Helper to encrypt data for testing.
    fn encrypt_data(data: &[u8], key_hex: &str, iv_bytes: &[u8; 16]) -> String {
        let key = hex::decode(key_hex).unwrap();
        let encryptor = Encryptor::<Aes256>::new(key.as_slice().into(), iv_bytes.into());
        let mut buf = vec![0u8; data.len() + 16]; // Enough space for padding
        let len = data.len();
        buf[..len].copy_from_slice(data);
        let ciphertext = encryptor
            .encrypt_padded_mut::<Pkcs7>(&mut buf, len)
            .unwrap();
        general_purpose::STANDARD.encode(ciphertext)
    }

    #[test]
    fn test_decrypt_and_parse_success() {
        let key_hex = "000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f";
        let iv = [0u8; 16];
        let iv_base64 = general_purpose::STANDARD.encode(iv);
        let json_data = r#"{"test": "value"}"#;
        let ciphertext_base64 = encrypt_data(json_data.as_bytes(), key_hex, &iv);
        let content = format!("{}\n{}", iv_base64, ciphertext_base64);

        let result = decrypt_and_parse(&content, key_hex);
        assert!(result.is_ok());
        let val = result.unwrap();
        assert_eq!(val["test"], "value");
    }

    #[test]
    fn test_decrypt_and_parse_invalid_lines() {
        let content = "only_one_line";
        let result = decrypt_and_parse(content, "dummy_key");
        match result {
            Err(CloudConfigError::InvalidData(msg)) => {
                assert!(msg.contains("expected at least 2 lines"))
            }
            _ => panic!("Expected InvalidData error"),
        }
    }

    #[test]
    fn test_decrypt_and_parse_invalid_key_len() {
        let iv_base64 = "AAAAAAAAAAAAAAAAAAAAAA=="; // Valid Base64 16 bytes
        let cipher_base64 = "AAAAAAAAAAAAAAAAAAAAAA==";
        let content = format!("{}\n{}", iv_base64, cipher_base64);
        let short_key = "001122"; // Valid hex, but only 3 bytes
        let result = decrypt_and_parse(&content, short_key);
        match result {
            Err(CloudConfigError::DecryptionError(msg)) => {
                assert!(msg.contains("Key must be 32 bytes"))
            }
            _ => panic!("Expected DecryptionError error with 'Key must be 32 bytes'"),
        }
    }

    #[test]
    fn test_load_cloud_config_missing_env() {
        // Unset variables to ensure failure
        env::remove_var("WEBLIB_AES_PASSWORD");
        env::remove_var("WEBLIB_CLOUD_CONFIG_URL");

        let result = load_cloud_config(None, None);
        match result {
            Err(CloudConfigError::MissingEnvVar(var)) => {
                assert!(var.contains("WEBLIB_AES_PASSWORD"))
            }
            _ => panic!("Expected MissingEnvVar error"),
        }
    }
}
