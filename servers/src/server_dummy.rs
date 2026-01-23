use actix_web::{App, HttpResponse, HttpServer, Responder, get};
use chrono::Utc;
use rustls::ServerConfig;
use rustls_pki_types::PrivateKeyDer;
use serde::Serialize;
use std::env;
use std::fs::File;
use std::io::BufReader;

#[derive(Serialize)]
struct StatusResponse {
    ts: String,
}

#[get("/")]
async fn index() -> impl Responder {
    let now = Utc::now();
    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(format!("<html><body>{}</body></html>", now.to_rfc3339()))
}

#[get("/status")]
async fn status() -> impl Responder {
    let now = Utc::now();
    HttpResponse::Ok().json(StatusResponse {
        ts: now.to_rfc3339(),
    })
}

fn load_rustls_config() -> ServerConfig {
    let home = dirs::home_dir().expect("Could not find home directory");
    let cert_path = home.join(".letsencrypt");
    let cert_file_path = cert_path.join("fullchain.pem");
    let key_file_path = cert_path.join("privkey.pem");

    // Load certificates
    let cert_file = File::open(cert_file_path).expect("Could not open fullchain.pem");
    let mut cert_reader = BufReader::new(cert_file);
    let cert_chain = rustls_pemfile::certs(&mut cert_reader)
        .collect::<Result<Vec<_>, _>>()
        .expect("Failed to load certificates");

    // Load private key
    // Try PKCS8 first, then fallback to RSA (PKCS1)
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

    ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(cert_chain, key)
        .expect("Invalid certificate configuration")
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    // Explicitly install the default crypto provider for rustls
    let _ = rustls::crypto::ring::default_provider().install_default();

    let port = env::args()
        .nth(1)
        .unwrap_or_else(|| "3000".to_string())
        .parse::<u16>()
        .expect("Port must be a number");

    let config = load_rustls_config();
    println!("HTTPS Server running on port {}", port);

    HttpServer::new(|| App::new().service(index).service(status))
        .bind_rustls_0_23(("0.0.0.0", port), config)?
        .run()
        .await
}
