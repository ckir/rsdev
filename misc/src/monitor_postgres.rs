//! # PostgreSQL Monitoring Service
//!
//! This utility performs periodic health checks on PostgreSQL instances defined in
//! the cloud configuration. It supports various connection methods (URL or structured objects),
//! SSL modes (NoTls or Rustls with custom verification), and automated alerting via webhooks.

use anyhow::{Context, Result};
use clap::Parser;
// // Statement: Ensure lib_common is compiled with --features "configs" to resolve E0433
use lib_common::configs::config_cloud::load_cloud_config;
use log::{error, info, warn};
use rustls::{ClientConfig, RootCertStore};
use serde::Deserialize;
use serde_json::Value;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;
use tokio_postgres::NoTls;
use tokio_postgres::config::SslMode;
use tokio_postgres_rustls::MakeRustlsConnect;
use url::Url;

// // Statement: Rustls trait imports for the custom certificate verifier
use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::pki_types::{CertificateDer, ServerName as PkiServerName, UnixTime};
use rustls::{DigitallySignedStruct, SignatureScheme};

/// Command-line arguments for the PostgreSQL monitor.
#[derive(Parser, Debug)]
#[command(author, version, about = "Monitors PostgreSQL instances hosted in the cloud", long_about = None)]
pub struct Args {
    /// Testing frequency in minutes.
    #[arg(short, long, default_value_t = 1)]
    pub frequency: u64,
    /// Disable sending alerts (useful for debugging).
    #[arg(long = "noalert", default_value_t = false)]
    pub noalert: bool,
}

/// Structured connection details for a PostgreSQL instance.
#[derive(Deserialize, Debug, Clone)]
pub struct PostgresConnection {
    /// The server host address.
    pub host: String,
    /// The database user.
    pub user: String,
    /// The name of the database.
    pub dbname: String,
    /// Optional port number.
    pub port: Option<u16>,
    /// Optional user password.
    pub password: Option<String>,
}

/// Represents a single PostgreSQL database entry from the cloud configuration.
#[derive(Deserialize, Debug, Clone)]
pub struct PostgresEntry {
    /// The display name of the database.
    #[serde(rename = "dbName")]
    pub db_name: String,
    /// Optional direct connection URL.
    #[serde(rename = "dbUrl")]
    pub db_url: Option<String>,
    /// Optional structured connection object.
    #[serde(rename = "dbConnection")]
    pub db_connection: Option<PostgresConnection>,
    /// The SSL mode to use for the connection (e.g., disable, require, prefer).
    #[serde(rename = "sslMode")]
    pub ssl_mode: Option<String>,
    /// Whether this specific instance should be monitored.
    #[serde(default)]
    pub monitor: bool,
}

/// Configuration for an individual alert server.
#[derive(Deserialize, Debug, Clone)]
pub struct AlertServer {
    /// The HTTP endpoint for the alert.
    pub host: String,
    /// The descriptive name of the alert server.
    pub name: String,
}

/// Container for primary and failover alert configurations.
#[derive(Deserialize, Debug, Clone)]
pub struct AlertsConfig {
    /// Primary alert destination.
    pub primary: AlertServer,
    /// Failover alert destination used if the primary fails.
    pub failover: AlertServer,
}

/// Internal structure holding the processed monitoring configuration.
pub struct MonitorConfig {
    /// Filtered list of database instances to check.
    pub postgres_instances: Vec<PostgresEntry>,
    /// Optional alerting configuration.
    pub alerts: Option<AlertsConfig>,
}

/// A custom verifier that skips server certificate validation.
#[derive(Debug)]
pub struct NoCertificateVerification;

impl ServerCertVerifier for NoCertificateVerification {
    fn verify_server_cert(
        &self,
        _end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _server_name: &PkiServerName<'_>,
        _ocsp_response: &[u8],
        _now: UnixTime,
    ) -> Result<ServerCertVerified, rustls::Error> {
        // // Statement: Always return success to bypass certificate checks
        Ok(ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        _message: &[u8],
        _cert: &CertificateDer<'_>,
        _dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        Ok(HandshakeSignatureValid::assertion())
    }

    fn verify_tls13_signature(
        &self,
        _message: &[u8],
        _cert: &CertificateDer<'_>,
        _dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        Ok(HandshakeSignatureValid::assertion())
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        rustls::crypto::ring::default_provider()
            .signature_verification_algorithms
            .supported_schemes()
    }
}

/// Initializes the logging system using `fern`.
pub fn setup_logging() -> Result<()> {
    // // Statement: Generate a timestamped log filename
    let log_filename = format!("monitor_postgres_{}.log", chrono::Local::now().format("%Y-%m-%d"));
    
    // // Statement: Configure dispatcher for dual output to console and file
    fern::Dispatch::new()
        .format(|out, message, record| {
            out.finish(format_args!(
                "{}[{}][{}] {}",
                chrono::Local::now().format("[%Y-%m-%d %H:%M:%S]"),
                record.target(),
                record.level(),
                message
            ))
        })
        .level(log::LevelFilter::Info)
        .chain(std::io::stdout())
        .chain(fern::log_file(log_filename)?)
        .apply()?;
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    // // Statement: Initialize application environment
    setup_logging().context("Failed to setup logging")?;
    let args = Args::parse();
    let interval_duration = Duration::from_secs(args.frequency * 60);

    info!("Starting PostgreSQL monitor. Frequency: {} minute(s)", args.frequency);

    loop {
        // // Statement: Load configuration from cloud using blocking task spawn
        let config_json_res: Result<Result<Value, _>, _> = tokio::task::spawn_blocking(move || {
            load_cloud_config(None, None)
        }).await;

        let config_json = match config_json_res {
            Ok(Ok(val)) => val,
            Ok(Err(e)) => {
                error!("Cloud config internal error: {}", e);
                sleep(Duration::from_secs(30)).await;
                continue;
            }
            Err(e) => {
                error!("Task join error: {}", e);
                sleep(Duration::from_secs(30)).await;
                continue;
            }
        };

        let mut postgres_instances = Vec::new();
        // // Statement: Parse PostgreSQL instances from JSON pointer
        if let Some(cloud_config) = config_json.pointer("/commonAll/db/postgres/cloud") {
            if let Ok(entries) = serde_json::from_value::<Vec<PostgresEntry>>(cloud_config.clone()) {
                // // Statement: Only include instances marked for monitoring
                postgres_instances = entries.into_iter().filter(|e| e.monitor).collect();
            }
        }

        let mut alerts = None;
        // // Statement: Parse alert configuration if available
        if let Some(alerts_json) = config_json.pointer("/commonAll/alerts") {
            if let Ok(cfg) = serde_json::from_value::<AlertsConfig>(alerts_json.clone()) {
                alerts = Some(cfg);
            }
        }

        let config = MonitorConfig { postgres_instances, alerts };

        // // Statement: Iterate through configured instances and verify connectivity
        for instance in &config.postgres_instances {
            if let Err(e) = check_postgres(instance).await {
                error!("FAILURE: Instance '{}' error: {:#}", instance.db_name, e);
                
                // // Statement: Handle alerting logic based on CLI flags and config
                if !args.noalert {
                    if let Some(ref alert_cfg) = config.alerts {
                        send_alert(alert_cfg, &instance.db_name, &e.to_string()).await;
                    }
                }
            } else {
                info!("SUCCESS: Instance '{}' is healthy.", instance.db_name);
            }
        }

        // // Statement: Wait for the next monitoring cycle
        sleep(interval_duration).await;
    }
}

/// Performs a connection and simple query test on a PostgreSQL instance.
///
/// # Arguments
/// * `entry` - The database entry containing connection details and SSL preferences.
async fn check_postgres(entry: &PostgresEntry) -> Result<()> {
    // // Statement: Construct Postgres config from URL or structured object
    let mut pg_config = if let Some(url) = &entry.db_url {
        url.parse::<tokio_postgres::Config>()?
    } else if let Some(conn) = &entry.db_connection {
        let mut cfg = tokio_postgres::Config::new();
        cfg.host(&conn.host);
        cfg.user(&conn.user);
        cfg.dbname(&conn.dbname);
        if let Some(p) = conn.port { cfg.port(p); }
        if let Some(pass) = &conn.password { cfg.password(pass); }
        cfg
    } else {
        return Err(anyhow::anyhow!("Missing connection info for {}", entry.db_name));
    };

    // // Statement: Configure SSL mode based on database entry
    let ssl_mode_str = entry.ssl_mode.as_deref().unwrap_or("prefer");
    let ssl_mode = match ssl_mode_str {
        "disable" => SslMode::Disable,
        "require" => SslMode::Require,
        _ => SslMode::Prefer,
    };
    pg_config.ssl_mode(ssl_mode);

    if ssl_mode == SslMode::Disable {
        // // Statement: Connect without TLS
        let (client, connection) = pg_config.connect(NoTls).await?;
        tokio::spawn(async move { if let Err(e) = connection.await { error!("DB connection error: {}", e); } });
        client.simple_query("SELECT 1").await?;
    } else {
        // // Statement: Fixed warning by removing 'mut' from root_store
        let root_store = RootCertStore::empty();
        let mut config = ClientConfig::builder()
            .with_root_certificates(root_store)
            .with_no_client_auth();
        
        // // Statement: Inject custom verifier to allow connections to self-signed certificates
        config.dangerous().set_certificate_verifier(Arc::new(NoCertificateVerification));
        let tls_connector = MakeRustlsConnect::new(config);

        let (client, connection) = pg_config.connect(tls_connector).await?;
        // // Statement: Spawn connection handler task
        tokio::spawn(async move { if let Err(e) = connection.await { error!("DB connection error: {}", e); } });
        // // Statement: Run health check query
        client.simple_query("SELECT 1").await?;
    }

    Ok(())
}

/// Dispatches an alert to the primary or failover alert server.
///
/// # Arguments
/// * `config` - Alerting configuration.
/// * `subject` - The database name/identifier.
/// * `message` - The specific error message.
pub async fn send_alert(config: &AlertsConfig, subject: &str, message: &str) {
    let client = reqwest::Client::new();
    let payload = serde_json::json!({
        "subject": format!("POSTGRES MONITOR: {}", subject),
        "message": message
    });

    // // Statement: Attempt primary alert with fallback logic
    if let Err(e) = client.post(&config.primary.host).json(&payload).send().await {
        warn!("Primary alert failed: {}. Trying failover...", e);
        if let Err(fe) = client.post(&config.failover.host).json(&payload).send().await {
            error!("Failover alert failed: {}", fe);
        }
    }
}

/// Helper to mask sensitive passwords in connection strings for logging.
///
/// # Arguments
/// * `url_str` - The raw PostgreSQL connection URL.
pub fn mask_url_password(url_str: &str) -> String {
    if let Ok(mut url) = Url::parse(url_str) {
        if url.password().is_some() {
            // // Statement: Redact password field
            let _ = url.set_password(Some("*****"));
        }
        url.to_string()
    } else {
        url_str.to_string()
    }
}