//! # Dummy Test Server
//!
//! This module implements a simple `actix_web` HTTP/HTTPS server intended for
//! local development, testing, and mocking purposes. It provides basic endpoints
//! to verify server responsiveness and TLS configuration without connecting
//! to live market data feeds.
//!
//! ## Endpoints:
//! - `GET /`: Returns a simple HTML page displaying the current UTC timestamp.
//! - `GET /status`: Returns a JSON object containing the current UTC timestamp.
//!
//! ## Features:
//! - **TLS/HTTPS Support**: Configured to load TLS certificates and private keys
//!   from the user's `.letsencrypt` directory, allowing for testing secure connections.
//! - **Configurable Port**: The server port can be specified as a command-line argument.
//!
//! This server is a lightweight substitute for the main data streaming services,
//! useful for isolated component testing or front-end development where a backend
//! presence is required but live data is not.

use actix_web::{App, HttpResponse, HttpServer, Responder, get};
use chrono::Utc;
use rustls::ServerConfig;
use rustls_pki_types::PrivateKeyDer;
use serde::Serialize;
use std::env;
use std::fs::File;
use std::io::BufReader;

/// # Status Response
///
/// Represents the JSON response structure for the `/status` endpoint.
#[derive(Serialize)]
struct StatusResponse {
    /// The current UTC timestamp in RFC 3339 format.
    ts: String,
}

#[get("/")]
/// Handles requests to the root path (`/`).
///
/// Returns a simple HTML page with the current UTC timestamp.
async fn index() -> impl Responder {
    let now = Utc::now();
    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(format!("<html><body>{}</body></html>", now.to_rfc3339()))
}

#[get("/status")]
/// Handles requests to the `/status` endpoint.
///
/// Returns a JSON object containing the current UTC timestamp.
async fn status() -> impl Responder {
    let now = Utc::now();
    HttpResponse::Ok().json(StatusResponse {
        ts: now.to_rfc3339(),
    })
}

/// # Load Rustls Configuration
///
/// Loads the TLS server configuration from PEM-encoded certificate and private key files.
///
/// It expects `fullchain.pem` (certificate chain) and `privkey.pem` (private key)
/// to be located in a `.letsencrypt` directory within the user's home directory.
///
/// # Panics
/// Panics if:
/// - The user's home directory cannot be found.
/// - The certificate or key files cannot be opened or parsed.
fn load_rustls_config() -> ServerConfig {
    let home = dirs::home_dir().expect("Could not find home directory");
    let cert_path = home.join(".letsencrypt");
    let cert_file_path = cert_path.join("fullchain.pem");
    let key_file_path = cert_path.join("privkey.pem");

    /// Load TLS certificates from `fullchain.pem`.
    let cert_file = File::open(cert_file_path).expect("Could not open fullchain.pem");
    let mut cert_reader = BufReader::new(cert_file);
    let cert_chain = rustls_pemfile::certs(&mut cert_reader)
        .collect::<Result<Vec<_>, _>>()
        .expect("Failed to load certificates");

    /// Load the private key from `privkey.pem`. Tries PKCS8 format first, then RSA (PKCS1).
    let key_file = File::open(&key_file_path).expect("Could not open privkey.pem");
    let mut key_reader = BufReader::new(key_file);

    let mut keys: Vec<PrivateKeyDer> = rustls_pemfile::pkcs8_private_keys(&mut key_reader)
        .map(|k| k.map(PrivateKeyDer::Pkcs8))
        .collect::<Result<Vec<_>, _>>()
        .expect("Failed to parse PKCS8 keys");

    if keys.is_empty() {
        let key_file = File::open(&key_file_path).expect("Could not open privkey.pem");
        let mut key_reader = BufReader::new(key_file);
        keys = rustls_pemfile::rsa_private_keys(&mut key_reader)
            .map(|k| k.map(PrivateKeyDer::Pkcs1))
            .collect::<Result<Vec<_>, _>>()
            .expect("Failed to parse RSA keys");
    }

    let key = keys
        .into_iter()
        .next()
        .expect("No private keys found in privkey.pem");

    /// Build the `ServerConfig` with the loaded certificate chain and private key.
    ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(cert_chain, key)
        .expect("Invalid certificate configuration")
}

#[actix_web::main]
/// # Main Entry Point
///
/// Initializes and runs the `actix_web` dummy server.
///
/// This function performs the following steps:
/// 1.  Installs the default crypto provider for `rustls`.
/// 2.  Parses the server port from command-line arguments or defaults to 3000.
/// 3.  Loads the TLS configuration using `load_rustls_config`.
/// 4.  Sets up the `actix_web` HTTP server with `/` and `/status` routes.
/// 5.  Binds the server to the specified address and starts serving requests over HTTPS.
///
/// # Panics
/// Panics if:
/// - The `rustls` crypto provider cannot be installed.
/// - The provided port is not a valid number.
/// - The TLS configuration is invalid.
/// - The server fails to bind to the specified address.
async fn main() -> std::io::Result<()> {
    /// Explicitly installs the default `ring` crypto provider for `rustls`, required for TLS.
    let _ = rustls::crypto::ring::default_provider().install_default();

    /// Parses the server port from command-line arguments. Defaults to 3000 if not provided.
    let port = env::args()
        .nth(1)
        .unwrap_or_else(|| "3000".to_string())
        .parse::<u16>()
        .expect("Port must be a number");

    /// Loads the TLS certificate and private key configuration.
    let config = load_rustls_config();
    println!("HTTPS Server running on port {}", port);

    /// Creates and runs the `actix_web` server, serving the `index` and `status` endpoints over HTTPS.
    HttpServer::new(|| App::new().service(index).service(status))
        .bind_rustls_0_23(("0.0.0.0", port), config)?
        .run()
        .await
}
