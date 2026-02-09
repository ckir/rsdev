use anyhow::{Context, Result};
use clap::Parser;
use lib_common::config_cloud::load_cloud_config;
use log::{error, info, warn};
use rustls::{ClientConfig, RootCertStore};
use serde::Deserialize;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;
use tokio_postgres::{NoTls, SimpleQueryMessage};
use tokio_postgres::config::SslMode;
use tokio_postgres_rustls::MakeRustlsConnect;
use postgres_native_tls::MakeTlsConnector as NativeMakeTlsConnector;
use native_tls::{TlsConnector as NativeTlsConnector, Certificate as NativeTlsCertificate};
use url::Url;

// Imports for custom verifier
use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::pki_types::{CertificateDer, ServerName as PkiServerName, UnixTime};
use rustls::{DigitallySignedStruct, SignatureScheme};

#[derive(Parser, Debug)]
#[command(author, version, about = "Monitors PostgreSQL instances hosted in the cloud", long_about = None)]
struct Args {
    /// Testing frequency in minutes
    #[arg(short, long, default_value_t = 1)]
    frequency: u64,
    /// Disable sending alerts (useful for debugging)
    #[arg(long = "noalert", default_value_t = false)]
    noalert: bool,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
struct PostgresConfig {
    active: bool,
    #[serde(rename = "dbDirectUrl")]
    #[allow(dead_code)]
    db_direct_url: Option<String>,
    #[serde(rename = "dbName")]
    db_name: String,
    #[serde(rename = "dbUrl")]
    db_url: Option<String>,
    #[serde(rename = "ssl")]
    ssl: Option<SslConfig>,
    #[serde(rename = "dbConnection")]
    db_connection: Option<PostgresConnectionObject>,
    #[serde(default)]
    monitor: bool,
}

#[derive(Deserialize, Debug, Clone)]
struct PostgresConnectionObject {
    user: String,
    password: String,
    host: String,
    port: u16,
    database: String,
    ssl: Option<SslConfig>,
}

#[derive(Deserialize, Debug, Clone)]
struct SslConfig {
    ca: Option<String>,
    #[serde(rename = "rejectUnauthorized")]
    reject_unauthorized: Option<bool>,
}

#[derive(Deserialize, Debug, Clone)]
struct AlertServer {
    host: String,
    name: String,
}

#[derive(Deserialize, Debug, Clone)]
struct AlertsConfig {
    primary: AlertServer,
    failover: AlertServer,
}

struct MonitorConfig {
    postgres_instances: Vec<PostgresConfig>,
    alerts: Option<AlertsConfig>,
}

// Enum to handle different types of TLS connectors
enum ConnectorKind {
    Rustls(MakeRustlsConnect),
    Native(NativeMakeTlsConnector),
}

fn setup_logging() -> Result<()> {
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
        .chain(std::io::stderr())
        .chain(fern::log_file(format!("monitor_postgres_{}.log", chrono::Local::now().format("%Y-%m-%d")))?)
        .apply()?;
    Ok(())
}

/// Masks the password in a PostgreSQL connection string.
fn mask_url_password(url: &str) -> String {
    if let Some(start_idx) = url.find("://") {
        let scheme_end = start_idx + 3;
        if let Some(at_idx) = url[scheme_end..].find('@') {
            let auth_part_end = scheme_end + at_idx;
            let auth_part = &url[scheme_end..auth_part_end];

            // Check if there is a password (format is :password@ or user:password@)
            if let Some(colon_idx) = auth_part.find(':') {
                // Reconstruct the URL with masked password
                let user = &auth_part[..colon_idx];
                let rest = &url[auth_part_end..];
                return format!("{}{}:*****{}", &url[..scheme_end], user, rest);
            }
        }
    }
    // Return original if no password pattern found or parsing fails
    url.to_string()
}

// Custom verifier that accepts everything (for Supabase/debugging)
#[derive(Debug)]
struct NoVerifier;

impl ServerCertVerifier for NoVerifier {
    fn verify_server_cert(
        &self,
        _end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _server_name: &PkiServerName<'_>,
        _ocsp_response: &[u8],
        _now: UnixTime,
    ) -> Result<ServerCertVerified, rustls::Error> {
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
        vec![
            SignatureScheme::RSA_PKCS1_SHA1,
            SignatureScheme::ECDSA_SHA1_Legacy,
            SignatureScheme::RSA_PKCS1_SHA256,
            SignatureScheme::ECDSA_NISTP256_SHA256,
            SignatureScheme::RSA_PKCS1_SHA384,
            SignatureScheme::ECDSA_NISTP384_SHA384,
            SignatureScheme::RSA_PKCS1_SHA512,
            SignatureScheme::ECDSA_NISTP521_SHA512,
            SignatureScheme::RSA_PSS_SHA256,
            SignatureScheme::RSA_PSS_SHA384,
            SignatureScheme::RSA_PSS_SHA512,
            SignatureScheme::ED25519,
            SignatureScheme::ED448,
        ]
    }
}

// Helper to create a Rustls connector
fn create_rustls_connector(
    ca_pem: Option<&str>,
    reject_unauthorized: Option<bool>,
) -> Result<MakeRustlsConnect> {
    let mut root_store = RootCertStore::empty();

    // Add system roots from rustls-native-certs
    let native_certs = rustls_native_certs::load_native_certs();

    for error in native_certs.errors {
        warn!("Error loading a native certificate: {}", error);
    }

    for cert in native_certs.certs {
        root_store.add(cert)?;
    }

    // Also add webpki roots as a fallback/supplement
    root_store.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());

    // Add custom CA if provided
    if let Some(ca) = ca_pem {
        let mut reader = std::io::BufReader::new(ca.as_bytes());
        for cert in rustls_pemfile::certs(&mut reader) {
            root_store.add(cert?)?;
        }
    }

    let builder = ClientConfig::builder();

    let mut config = if let Some(false) = reject_unauthorized {
        warn!("Disabling SSL verification (using NoVerifier).");
        builder
            .dangerous()
            .with_custom_certificate_verifier(Arc::new(NoVerifier))
            .with_no_client_auth()
    } else {
        builder
            .with_root_certificates(root_store)
            .with_no_client_auth()
    };

    // Add ALPN protocol for PostgreSQL, which is required by some providers like Supabase
    config.alpn_protocols = vec![b"postgresql".to_vec()];

    Ok(MakeRustlsConnect::new(config))
}

// Helper to create a Native-TLS connector
fn create_native_connector(
    ca_pem: Option<&str>,
    reject_unauthorized: Option<bool>,
) -> Result<NativeMakeTlsConnector> {
    let mut builder = NativeTlsConnector::builder();

    // If reject_unauthorized is explicitly false, OR if we are forcing it (handled by caller passing Some(false))
    if let Some(false) = reject_unauthorized {
        builder.danger_accept_invalid_certs(true);
    }

    // Add custom CA if provided
    if let Some(ca) = ca_pem {
        let cert = NativeTlsCertificate::from_pem(ca.as_bytes())?;
        builder.add_root_certificate(cert);
    }

    let connector = builder.build().context("Failed to build native-tls connector")?;
    Ok(NativeMakeTlsConnector::new(connector))
}

async fn check_postgres(entry: &PostgresConfig) -> Result<(chrono::DateTime<chrono::Utc>, String)> {
    // Prepare configuration and TLS connector based on entry type
    let (config, tls_connector) = if let Some(url_str) = entry.db_url.as_ref().filter(|s| !s.is_empty())
     {
         let mut config = url_str.parse::<tokio_postgres::Config>()?;

        // Choose connector based on ssl_mode from the connection string.
        let connector = if config.get_ssl_mode() != SslMode::Disable {
            // Check if it's a Supabase instance by parsing the URL
            let is_supabase = if let Ok(parsed_url) = Url::parse(url_str) {
                if let Some(host) = parsed_url.host_str() {
                    host.contains("supabase.co") || host.contains("supabase.com")
                } else {
                    false
                }
            } else {
                // Fallback to simple string check if parsing fails
                url_str.contains("supabase.co") || url_str.contains("supabase.com")
            };

            if is_supabase {
                info!("Using native-tls for Supabase instance: {} (Verification Disabled)", entry.db_name);
                // Force Require for Supabase
                if config.get_ssl_mode() != SslMode::Disable {
                    config.ssl_mode(SslMode::Require);
                }
                // Force disable verification for Supabase to bypass trust store issues
                let ca_pem = entry.ssl.as_ref().and_then(|s| s.ca.as_deref());
                Some(ConnectorKind::Native(create_native_connector(ca_pem, Some(false))?))
            } else {
                // Use rustls for everything else
                let ca_pem = entry.ssl.as_ref().and_then(|s| s.ca.as_deref());
                let reject_unauthorized = entry.ssl.as_ref().and_then(|s| s.reject_unauthorized);
                Some(ConnectorKind::Rustls(create_rustls_connector(ca_pem, reject_unauthorized)?))
            }
        } else {
            None
        };
         (config, connector)
     } else if let Some(obj) = &entry.db_connection {
         let mut config = tokio_postgres::Config::new();
         config.user(&obj.user);
         config.password(&obj.password);
         config.host(&obj.host);
         config.port(obj.port);
         config.dbname(&obj.database);

         let connector = if let Some(ssl) = &obj.ssl {
             config.ssl_mode(SslMode::Require);

             if obj.host.contains("supabase.co") || obj.host.contains("supabase.com") {
                 info!("Using native-tls for Supabase instance: {} (Verification Disabled)", entry.db_name);
                 Some(ConnectorKind::Native(create_native_connector(ssl.ca.as_deref(), Some(false))?))
             } else {
                 // Use rustls for everything else
                 Some(ConnectorKind::Rustls(create_rustls_connector(
                     ssl.ca.as_deref(),
                     ssl.reject_unauthorized,
                 )?))
             }
         } else {
             config.ssl_mode(SslMode::Disable);
             None
         };
         (config, connector)
     } else {
         return Err(anyhow::anyhow!(
             "No dbUrl or dbConnection found for {}",
             entry.db_name
         ));
     };

    // Connect using the appropriate transport (TLS or NoTls)
    let client = if let Some(connector_kind) = tls_connector {
        match connector_kind {
            ConnectorKind::Rustls(connector) => {
                let (client, connection) = config.connect(connector).await.with_context(|| {
                    let mut msg = format!("Failed to connect to PostgreSQL (Rustls) for {}", entry.db_name);
                    if let Some(url) = entry.db_url.as_ref() {
                        msg.push_str(&format!(". URL: {}", mask_url_password(url)));
                    } else if let Some(obj) = entry.db_connection.as_ref() {
                        msg.push_str(&format!(". Host: {}", obj.host));
                    }
                    msg
                })?;
                tokio::spawn(async move { if let Err(e) = connection.await { error!("PostgreSQL connection error: {}", e); } });
                client
            },
            ConnectorKind::Native(connector) => {
                let (client, connection) = config.connect(connector).await.with_context(|| {
                    let mut msg = format!("Failed to connect to PostgreSQL (Native-TLS) for {}", entry.db_name);
                    if let Some(url) = entry.db_url.as_ref() {
                        msg.push_str(&format!(". URL: {}", mask_url_password(url)));
                    } else if let Some(obj) = entry.db_connection.as_ref() {
                        msg.push_str(&format!(". Host: {}", obj.host));
                    }
                    msg
                })?;
                tokio::spawn(async move { if let Err(e) = connection.await { error!("PostgreSQL connection error: {}", e); } });
                client
            }
        }
    } else {
        let (client, connection) = config.connect(NoTls).await.with_context(|| {
            let mut msg = format!("Failed to connect to PostgreSQL (NoTls) for {}", entry.db_name);
            if let Some(url) = entry.db_url.as_ref() {
                msg.push_str(&format!(". URL: {}", mask_url_password(url)));
            } else if let Some(obj) = entry.db_connection.as_ref() {
                msg.push_str(&format!(". Host: {}", obj.host));
            }
            msg
        })?;
        tokio::spawn(async move { if let Err(e) = connection.await { error!("PostgreSQL connection error: {}", e); } });
        client
    };

    // Use simple_query to avoid prepared statements, which are not supported by Supabase transaction poolers (port 6543)
    let messages = client
        .simple_query("SELECT NOW(), version()")
        .await
        .context("Failed to execute test query on PostgreSQL")?;

    let mut server_time: Option<chrono::DateTime<chrono::Utc>> = None;
    let mut server_version: Option<String> = None;

    for message in messages {
        if let SimpleQueryMessage::Row(row) = message {
            let time_str = row.get(0).context("Failed to get server time from row")?;
            let version_str = row.get(1).context("Failed to get server version from row")?;

            // Parse the timestamp string returned by Postgres
            // Example: 2025-01-20 12:34:56.789+00
            let parsed_time = chrono::DateTime::parse_from_str(time_str, "%Y-%m-%d %H:%M:%S%.f%#z")
                .or_else(|_| chrono::DateTime::parse_from_str(time_str, "%Y-%m-%d %H:%M:%S%.f")) // Try without timezone if needed
                .context(format!("Failed to parse server time: {}", time_str))?
                .with_timezone(&chrono::Utc);

            server_time = Some(parsed_time);
            server_version = Some(version_str.to_string());
            break; // We only need the first row
        }
    }

    if let (Some(time), Some(version)) = (server_time, server_version) {
        Ok((time, version))
    } else {
        Err(anyhow::anyhow!("Query returned no data rows"))
    }
}

async fn send_alert(alerts: &AlertsConfig, instance: &str, error_msg: &str) {
    let client = reqwest::Client::new();

    let now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
    let message = format!(
        "PostgreSQL instance {} failed. Error: {}",
        instance, error_msg
    );

    let payload = serde_json::json!({
        "Date": now,
        "Info": "PostgreSQL Monitor Alert",
        "Message": message
    });

    // Try Primary
    info!("Sending alert to Primary server: {}", alerts.primary.name);
    let primary_result = client
        .post(&alerts.primary.host)
        .json(&payload)
        .send()
        .await;

    let mut success = false;
    match primary_result {
        Ok(res) => {
            if res.status().is_success() {
                info!("Alert sent successfully to Primary server.");
                success = true;
            } else {
                error!(
                    "Failed to send alert to Primary server. Status: {}",
                    res.status()
                );
            }
        }
        Err(e) => {
            error!("Network error sending alert to Primary server: {}", e);
        }
    }

    // If Primary failed, try Failover
    if !success {
        warn!(
            "Primary alert failed. Attempting to send alert to Failover server: {}",
            alerts.failover.name
        );
        let failover_result = client
            .post(&alerts.failover.host)
            .json(&payload)
            .send()
            .await;

        match failover_result {
            Ok(res) => {
                if res.status().is_success() {
                    info!("Alert sent successfully to Failover server.");
                } else {
                    error!(
                        "Failed to send alert to Failover server. Status: {}",
                        res.status()
                    );
                }
            }
            Err(e) => {
                error!("Network error sending alert to Failover server: {}", e);
            }
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    // Explicitly install the default crypto provider for rustls
    let _ = rustls::crypto::ring::default_provider().install_default();

    let args = Args::parse();
    setup_logging().context("Failed to initialize logging")?;

    info!(
        "Starting PostgreSQL Monitor. Frequency: {} minute(s)",
        args.frequency
    );

    // 1. Download Configuration
    info!("Retrieving cloud configuration...");

    // Wrap the blocking call in spawn_blocking to avoid panicking the async runtime
    let config_json = tokio::task::spawn_blocking(move || load_cloud_config(None, None)).await??;

    // 2. Parse Configuration
    let mut postgres_instances = Vec::new();

    // Navigate to config.commonAll.db.postgres.cloud
    if let Some(postgres_cloud_config) = config_json.pointer("/commonAll/db/postgres/cloud") {
        match serde_json::from_value::<Vec<PostgresConfig>>(postgres_cloud_config.clone()) {
            Ok(entries) => {
                postgres_instances = entries
                    .into_iter()
                    .filter(|e| e.active && e.monitor)
                    .collect();
                info!(
                    "Found {} PostgreSQL cloud instances to monitor.",
                    postgres_instances.len()
                );
            }
            Err(e) => warn!("Failed to parse postgres cloud config: {}", e),
        }
    } else {
        warn!("Configuration path /commonAll/db/postgres/cloud not found");
    }

    // Parse Alerts Configuration
    let mut alerts_config: Option<AlertsConfig> = None;
    if let Some(alerts_json) = config_json.pointer("/commonAll/alerts") {
        match serde_json::from_value::<AlertsConfig>(alerts_json.clone()) {
            Ok(cfg) => {
                info!(
                    "Found alert servers: Primary='{}', Failover='{}'",
                    cfg.primary.name, cfg.failover.name
                );
                alerts_config = Some(cfg);
            }
            Err(e) => warn!("Failed to parse alerts config: {}", e),
        }
    } else {
        warn!("Configuration path /commonAll/alerts not found");
    }

    let config = MonitorConfig {
        postgres_instances,
        alerts: alerts_config,
    };

    if config.postgres_instances.is_empty() {
        warn!("No PostgreSQL instances found in configuration to monitor.");
    }

    let interval_duration = Duration::from_secs(args.frequency * 60);

    loop {
        info!(
            "Checking {} PostgreSQL instances...",
            config.postgres_instances.len()
        );

        for instance_entry in &config.postgres_instances {
            let display_name = &instance_entry.db_name;

            match check_postgres(instance_entry).await {
                Ok((server_time, server_version)) => {
                    info!(
                        "Instance {} is HEALTHY. Time: {}, Version: {}",
                        display_name, server_time, server_version
                    );
                }
                Err(e) => {
                    let connection_info = if let Some(url) = &instance_entry.db_url {
                        mask_url_password(url)
                    } else if let Some(obj) = &instance_entry.db_connection {
                        format!("Host: {}", obj.host)
                    } else {
                        "Unknown connection info".to_string()
                    };

                    error!("Instance {} FAILED. Connection: {}. Error: {:#}", display_name, connection_info, e);

                    if args.noalert {
                        warn!("Alert suppressed by --noalert flag.");
                    } else if let Some(ref alerts) = config.alerts {
                        send_alert(alerts, display_name, &e.to_string()).await;
                    } else {
                        warn!("No alert servers configured. Alert suppressed.");
                    }
                }
            }
        }

        sleep(interval_duration).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mask_url_password_hides_password() {
        let url = "postgresql://user:secret@host:5432/db";
        let masked = mask_url_password(url);
        assert!(masked.contains(":*****@"));
        assert!(!masked.contains("secret"));
    }

    #[test]
    fn test_create_rustls_connector_ok() {
        // Should return Ok with default system roots
        let res = create_rustls_connector(None, None);
        assert!(res.is_ok());
    }
}
