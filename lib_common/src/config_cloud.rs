use aes::Aes256;
use base64::{Engine as _, engine::general_purpose};
use cbc::Decryptor;
use cipher::{BlockDecryptMut, KeyIvInit, block_padding::Pkcs7};
use reqwest::blocking::Client;
use serde_json::Value;
use static_init::dynamic;
use std::env;
use thiserror::Error;

#[derive(Debug, Error, Clone)]
/// # Cloud Configuration Error
///
/// Defines custom error types that can occur during the retrieval and decryption
/// of cloud-based configuration.
pub enum CloudConfigError {
    /// An error occurred while accessing environment variables.
    #[error("Environment variable error: {0}")]
    VarError(#[from] env::VarError),

    /// A required environment variable was not found.
    #[error("Missing environment variable: {0}")]
    MissingEnvVar(String),

    /// An error occurred during network communication (HTTP request).
    #[error("Network error: {0}")]
    NetworkError(String),

    /// An error occurred during the decryption process (e.g., invalid key, IV, or ciphertext).
    #[error("Decryption error: {0}")]
    DecryptionError(String),

    /// An error occurred while parsing the decrypted content as JSON.
    #[error("JSON parse error: {0}")]
    JsonError(String),

    /// The format of the retrieved encrypted data was invalid.
    #[error("Invalid data format: {0}")]
    InvalidData(String),
}

/// Statically initialized `CLOUD_CONFIG` that holds the decrypted cloud configuration.
///
/// This ensures that the configuration is loaded and decrypted only once
/// at application startup and then reused across all parts of the application.
/// It wraps the `load_cloud_config` function, providing a thread-safe,
/// lazy-initialized way to access the cloud configuration.
#[dynamic]
static CLOUD_CONFIG: Result<Value, CloudConfigError> = load_cloud_config(None, None);

/// # Load Cloud Configuration
///
/// Retrieves, decrypts, and parses the application's cloud-based configuration.
///
/// This function performs the following steps:
/// 1.  **Get Key & URL**: Obtains the AES decryption key (password) and the URL
///     of the encrypted configuration file from environment variables (`WEBLIB_AES_PASSWORD`,
///     `WEBLIB_CLOUD_CONFIG_URL`) or optional arguments.
/// 2.  **Fetch Encrypted Content**: Makes an HTTP GET request to the specified URL
///     to fetch the encrypted configuration file.
/// 3.  **Parse Encrypted Data**: The fetched content is expected to be two Base64-encoded
///     lines: the Initialization Vector (IV) and the ciphertext. These are decoded.
/// 4.  **Decrypt**: The ciphertext is decrypted using AES-256 CBC mode with the
///     retrieved key and IV. PKCS7 padding is removed.
/// 5.  **Parse JSON**: The decrypted plaintext is parsed as a JSON `serde_json::Value`.
///
/// All steps include robust error handling, returning a `CloudConfigError` on failure.
///
/// # Arguments
/// * `url` - An optional `String` to explicitly provide the URL for the encrypted config.
///   If `None`, it defaults to the `WEBLIB_CLOUD_CONFIG_URL` environment variable.
/// * `password` - An optional `String` to explicitly provide the AES decryption password.
///   If `None`, it defaults to the `WEBLIB_AES_PASSWORD` environment variable.
///
/// # Returns
/// A `Result<Value, CloudConfigError>` containing the decrypted and parsed JSON configuration
/// on success, or a `CloudConfigError` on failure.
pub fn load_cloud_config(
    url: Option<String>,
    password: Option<String>,
) -> Result<Value, CloudConfigError> {
    /// 1. Retrieve the AES decryption password from arguments or environment variables.
    let password = password
        .or_else(|| env::var("WEBLIB_AES_PASSWORD").ok())
        .ok_or_else(|| CloudConfigError::MissingEnvVar("WEBLIB_AES_PASSWORD".to_string()))?;

    // Ensure password is clean (remove whitespace/newlines)
    let password = password.trim();

    /// 1. Retrieve the URL for the encrypted configuration file from arguments or environment variables.
    let url = url
        .or_else(|| env::var("WEBLIB_CLOUD_CONFIG_URL").ok())
        .ok_or_else(|| CloudConfigError::MissingEnvVar("WEBLIB_CLOUD_CONFIG_URL".to_string()))?;
    // println!("URL: {}", url);

    /// 2. Fetch the encrypted file content via an HTTP GET request.
    let client = Client::new();
    let response = client
        .get(&url)
        .send()
        .map_err(|e| CloudConfigError::NetworkError(e.to_string()))?;

    if !response.status().is_success() {
        return Err(CloudConfigError::NetworkError(format!(
            "HTTP request failed with status: {}",
            response.status()
        )));
    }

    let content = response
        .text()
        .map_err(|e| CloudConfigError::NetworkError(e.to_string()))?;

    /// 3. Parse the fetched content, which is expected to be Base64-encoded IV and ciphertext on separate lines.
    let lines: Vec<&str> = content
        .lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty())
        .collect();

    if lines.len() < 2 {
        return Err(CloudConfigError::InvalidData(format!(
            "File format error: expected at least 2 lines (IV and Ciphertext), found {}",
            lines.len()
        )));
    }

    let iv_base64 = lines[0];
    let ciphertext_base64 = lines[1];

    /// Decode the Base64-encoded IV.
    let iv = general_purpose::STANDARD
        .decode(iv_base64)
        .map_err(|e| CloudConfigError::InvalidData(format!("Invalid Base64 IV: {}", e)))?;

    /// Decode the Base64-encoded ciphertext.
    let ciphertext = general_purpose::STANDARD
        .decode(ciphertext_base64)
        .map_err(|e| CloudConfigError::InvalidData(format!("Invalid Base64 Ciphertext: {}", e)))?;

    /// 4. Decrypt the content using AES-256 CBC mode.
    /// Hex-decodes the password to get the 32-byte AES key.
    let key_vec = hex::decode(password)
        .map_err(|e| CloudConfigError::DecryptionError(format!("Invalid Key Hex: {}", e)))?;

    if key_vec.len() != 32 {
        return Err(CloudConfigError::DecryptionError(format!(
            "Key must be 32 bytes (256 bits), found {}",
            key_vec.len()
        )));
    }

    let key_arr: [u8; 32] = key_vec.try_into().expect("Length checked above");
    let iv_arr: [u8; 16] = iv
        .as_slice()
        .try_into()
        .map_err(|_| CloudConfigError::InvalidData(format!("Invalid IV length: {}", iv.len())))?;

    /// Initializes the AES-256 CBC decryptor with the key and IV.
    let decryptor = Decryptor::<Aes256>::new(&key_arr.into(), &iv_arr.into());

    /// Performs the decryption with PKCS7 padding removal.
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
                "Decryption failed (UnpadError): {:?}. Check if the Key is correct.",
                e
            ))
        })?;

    /// 5. Parses the decrypted data as JSON.
    let json: Value = serde_json::from_slice(decrypted_data)
        .map_err(|e| CloudConfigError::JsonError(e.to_string()))?;

    Ok(json)
}

/// # Get Cloud Configuration
///
/// Returns a clone of the globally cached, decrypted cloud configuration.
///
/// This function provides a convenient way to access the cloud configuration
/// without re-fetching and re-decrypting it, as it leverages the `CLOUD_CONFIG`
/// static initializer.
///
/// # Returns
/// A `Result<Value, CloudConfigError>` containing a clone of the parsed JSON
/// configuration on success, or a `CloudConfigError` if the initial loading/decryption
/// failed.
pub fn get_cloud_config() -> Result<Value, CloudConfigError> {
    match &*CLOUD_CONFIG {
        Ok(val) => Ok(val.clone()),
        Err(e) => Err(e.clone()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use aes::Aes256;
    use base64::engine::general_purpose;
    use cipher::{BlockEncryptMut, KeyIvInit, block_padding::Pkcs7};
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::thread;

    #[test]
    fn test_load_cloud_config_success() {
        // 1. Prepare the expected data
        // Use a valid 32-byte hex key
        let key_hex = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
        let key = hex::decode(key_hex).unwrap();

        let expected_json = serde_json::json!({
            "service_name": "test_service",
            "retries": 3,
            "enabled": true
        });
        let plain_text = serde_json::to_vec(&expected_json).unwrap();

        // 2. Encrypt the data (Simulate the cloud file generation)

        // IV generation (Fixed for test)
        let iv = [0x11u8; 16];

        // Encrypt using AES-256-CBC with PKCS7 padding
        let encryptor = cbc::Encryptor::<Aes256>::new(key.as_slice().into(), &iv.into());
        let ciphertext = encryptor.encrypt_padded_vec_mut::<Pkcs7>(&plain_text);

        // Construct final payload: Base64 IV \n Base64 Ciphertext
        let iv_base64 = general_purpose::STANDARD.encode(&iv);
        let ciphertext_base64 = general_purpose::STANDARD.encode(&ciphertext);

        let payload = format!("{}\n{}", iv_base64, ciphertext_base64);

        // 3. Start a mock HTTP server on a random local port
        let listener = TcpListener::bind("127.0.0.1:0").expect("Failed to bind to random port");
        let port = listener.local_addr().unwrap().port();
        let url = format!("http://127.0.0.1:{}/config", port);

        let handle = thread::spawn(move || {
            // Accept one connection
            if let Ok((mut stream, _)) = listener.accept() {
                // Read the request (consume buffer)
                let mut buf = [0u8; 1024];
                let _ = stream.read(&mut buf);

                // Send HTTP response
                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nContent-Type: text/plain\r\n\r\n",
                    payload.len()
                );
                stream.write_all(response.as_bytes()).unwrap();
                stream.write_all(payload.as_bytes()).unwrap();
                stream.flush().unwrap();
            }
        });

        // 4. Call the function under test
        let result = load_cloud_config(Some(url), Some(key_hex.to_string()));

        // Ensure server thread finishes
        handle.join().unwrap();

        // 5. Assertions
        assert!(result.is_ok(), "Expected Ok, got Err: {:?}", result.err());
        assert_eq!(result.unwrap(), expected_json);
    }
}
