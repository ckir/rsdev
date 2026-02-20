//! # Cloud Configuration Module
//!
//! This module provides utilities to retrieve, decrypt, and parse configuration
//! files stored in cloud environments. It supports AES-256-CBC decryption and
//! utilizes static initialization to cache the configuration in memory.

use aes::Aes256;
use base64::{Engine as _, engine::general_purpose};
use cbc::Decryptor;
use cipher::{BlockDecryptMut, KeyIvInit, block_padding::Pkcs7};
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
    // // Statement: Resolve decryption password from arguments or environment.
    let password = password
        .or_else(|| env::var("WEBLIB_AES_PASSWORD").ok())
        .ok_or_else(|| CloudConfigError::MissingEnvVar("WEBLIB_AES_PASSWORD".to_string()))?;

    let password = password.trim();

    // // Statement: Resolve target URL for the configuration file.
    let url = url
        .or_else(|| env::var("WEBLIB_CLOUD_CONFIG_URL").ok())
        .ok_or_else(|| CloudConfigError::MissingEnvVar("WEBLIB_CLOUD_CONFIG_URL".to_string()))?;

    // // Statement: Initialize blocking HTTP client.
    let client = Client::new();
    
    // // Statement: Execute GET request; explicit type reqwest::Error added for stability.
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

    // // Statement: Extract raw text content.
    let content = response
        .text()
        .map_err(|e: reqwest::Error| CloudConfigError::NetworkError(e.to_string()))?;

    // // Statement: Parse file lines (Expected: Line 1 = IV, Line 2 = Ciphertext).
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

    // // Statement: Decode Base64 initialization vector and ciphertext.
    let iv = general_purpose::STANDARD
        .decode(iv_base64)
        .map_err(|e| CloudConfigError::InvalidData(format!("Invalid Base64 IV: {}", e)))?;

    let ciphertext = general_purpose::STANDARD
        .decode(ciphertext_base64)
        .map_err(|e| CloudConfigError::InvalidData(format!("Invalid Base64 Ciphertext: {}", e)))?;

    // // Statement: Hex-decode password and validate 32-byte length for AES-256.
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

    // // Statement: Decrypt buffer and remove PKCS7 padding.
    let decryptor = Decryptor::<Aes256>::new(&key_arr.into(), &iv_arr.into());
    let mut buf = ciphertext.to_vec();
    if buf.is_empty() {
        return Err(CloudConfigError::DecryptionError("Ciphertext is empty".to_string()));
    }

    let decrypted_data = decryptor
        .decrypt_padded_mut::<Pkcs7>(&mut buf)
        .map_err(|e| {
            CloudConfigError::DecryptionError(format!(
                "Decryption failed: {:?}. Verify the decryption key.",
                e
            ))
        })?;

    // // Statement: Parse final decrypted bytes as JSON.
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
    // // Statement: Access static result via dereference.
    match &*CLOUD_CONFIG {
        Ok(val) => Ok(val.clone()),
        Err(e) => Err(e.clone()),
    }
}