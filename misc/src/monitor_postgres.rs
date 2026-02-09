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
    /// Testing frequency in minutes. The monitor will check instances at this interval.
    #[arg(short, long, default_value_t = 1)]
    frequency: u64,
    /// Disables sending alerts for detected issues (useful for debugging or maintenance).
    #[arg(long = "noalert", default_value_t = false)]
    noalert: bool,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
struct PostgresConfig {
    /// Whether this PostgreSQL instance should be actively monitored.
    active: bool,
    /// Direct connection URL for the database (e.g., for specific tools).
    #[serde(rename = "dbDirectUrl")]
    #[allow(dead_code)]
    db_direct_url: Option<String>,
    /// The name of the database instance.
    #[serde(rename = "dbName")]
    db_name: String,
    /// Standard connection URL for the database (e.g., `postgresql://user:pass@host:port/dbname`).
    #[serde(rename = "dbUrl")]
    db_url: Option<String>,
    /// Optional SSL configuration for the database connection.
    #[serde(rename = "ssl")]
    ssl: Option<SslConfig>,
    /// Alternative way to specify connection details as an object.
    #[serde(rename = "dbConnection")]
    db_connection: Option<PostgresConnectionObject>,
    /// Whether to monitor this instance. Defaults to true if not specified in config.
    #[serde(default)]
    monitor: bool,
}

#[derive(Deserialize, Debug, Clone)]
/// # PostgreSQL Connection Object
///
/// Provides detailed connection parameters for a PostgreSQL instance as a structured object,
/// as an alternative to a single `dbUrl` string.
struct PostgresConnectionObject {
    /// The username for connecting to the database.
    user: String,
    /// The password for the specified user.
    password: String,
    /// The host address or domain name of the PostgreSQL server.
    host: String,
    /// The port number on which the PostgreSQL server is listening.
    port: u16,
    /// The name of the database to connect to.
    database: String,
    /// Optional SSL configuration specific to this connection object.
    ssl: Option<SslConfig>,
}

#[derive(Deserialize, Debug, Clone)]
/// # SSL Configuration
///
/// Defines parameters for SSL/TLS connections to PostgreSQL.
struct SslConfig {
    /// Optional path to a custom CA certificate (PEM format) to trust.
    ca: Option<String>,
    /// Whether to reject unauthorized (self-signed or untrusted) certificates.
    /// `true` for strict validation, `false` to accept self-signed certificates.
    #[serde(rename = "rejectUnauthorized")]
    reject_unauthorized: Option<bool>,
}

#[derive(Deserialize, Debug, Clone)]
/// # Alert Server
///
/// Defines the endpoint and name for an alert notification server.
struct AlertServer {
    /// The URL or host of the alert server endpoint.
    host: String,
    /// A human-readable name for the alert server.
    name: String,
}

#[derive(Deserialize, Debug, Clone)]
/// # Alerts Configuration
///
/// Groups configuration for primary and failover alert notification servers.
struct AlertsConfig {
    /// Configuration for the primary alert server.
    primary: AlertServer,
    /// Configuration for the failover (backup) alert server.
    failover: AlertServer,
}

/// # Monitor Configuration
///
/// Top-level configuration for the PostgreSQL monitor, containing a list of instances to check
/// and details for sending alerts.
struct MonitorConfig {
    /// A list of PostgreSQL instances to be monitored, filtered for active and monitorable ones.
    postgres_instances: Vec<PostgresConfig>,
    /// Optional configuration for primary and failover alert servers.
    alerts: Option<AlertsConfig>,
}

// Enum to handle different types of TLS connectors
/// # Connector Kind
///
/// Enum to abstract over different types of TLS connectors (`rustls` or `native-tls`)
/// used for PostgreSQL connections.
enum ConnectorKind {
    /// Represents a `rustls`-based TLS connector.
    Rustls(MakeRustlsConnect),
    /// Represents a `native-tls`-based TLS connector.
    Native(NativeMakeTlsConnector),
}

/// # Setup Logging
///
/// Configures the `fern` logger to output to both standard error and a daily rotating file.
///
/// Log messages are formatted with a timestamp, target, level, and the message itself.
/// The log level is set to `Info` by default. Log files are named `monitor_postgres_YYYY-MM-DD.log`.
///
/// # Returns
/// A `Result<()>` indicating success or failure of the logging setup.
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

/// # Mask URL Password
///
/// Masks the password component within a PostgreSQL connection URL string
/// for safe logging or display.
///
/// It identifies the password between the `user:` and `@host` parts of the URL
/// and replaces it with `*****`. If no password is found or the URL format
/// is unexpected, the original URL is returned.
///
/// # Arguments
/// * `url` - The PostgreSQL connection URL string.
///
/// # Returns
/// A `String` with the password masked.
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
/// # NoVerifier
///
/// A custom `rustls::client::danger::ServerCertVerifier` implementation that
/// bypasses all server certificate validation.
///
/// **WARNING:** This verifier should only be used in specific scenarios (e.g.,
/// connecting to certain cloud providers like Supabase that use non-standard
/// certificate setups or for debugging purposes) where you fully understand
/// and accept the security implications. Using this in production with untrusted
/// servers can expose you to man-in-the-middle attacks.
#[derive(Debug)]
struct NoVerifier;

impl ServerCertVerifier for NoVerifier {
    /// Always returns `ServerCertVerified::assertion()`, effectively trusting any server certificate.
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

    /// Always returns `HandshakeSignatureValid::assertion()`, bypassing signature validation for TLS 1.2.
    fn verify_tls12_signature(
        &self,
        _message: &[u8],
        _cert: &CertificateDer<'_>,
        _dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        Ok(HandshakeSignatureValid::assertion())
    }

    /// Always returns `HandshakeSignatureValid::assertion()`, bypassing signature validation for TLS 1.3.
    fn verify_tls13_signature(
        &self,
        _message: &[u8],
        _cert: &CertificateDer<'_>,
        _dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        Ok(HandshakeSignatureValid::assertion())
    }

    /// Lists the supported signature schemes, which are standard for common TLS setups.
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
/// # Create `rustls` Connector
///
/// Helper function to create a `MakeRustlsConnect` for `tokio-postgres`
/// with a customized `rustls::ClientConfig`.
///
/// This function sets up root certificates from system stores (`rustls-native-certs`)
/// and common web PKI roots (`webpki-roots`). It can optionally add a custom CA
/// certificate and disable server certificate verification if `reject_unauthorized` is `false`.
/// It also adds the `postgresql` ALPN protocol, which is often required.
///
/// # Arguments
/// * `ca_pem` - An optional PEM-encoded CA certificate string to add to the trust store.
/// * `reject_unauthorized` - An `Option<bool>` to explicitly control server certificate validation.
///   If `Some(false)`, a `NoVerifier` is used to disable validation.
///
/// # Returns
/// A `Result<MakeRustlsConnect>` on success, or an `anyhow::Error` if root certificates
/// cannot be loaded or the `ClientConfig` cannot be built.
fn create_rustls_connector(
    ca_pem: Option<&str>,
    reject_unauthorized: Option<bool>,
) -> Result<MakeRustlsConnect> {
    let mut root_store = RootCertStore::empty();

    /// Add system root certificates from `rustls-native-certs`.
    let native_certs = rustls_native_certs::load_native_certs();

    for error in native_certs.errors {
        warn!("Error loading a native certificate: {}", error);
    }

    for cert in native_certs.certs {
        root_store.add(cert)?;
    }

    /// Also add `webpki` roots as a fallback/supplement for common CAs.
    root_store.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());

    /// Add custom CA if provided in PEM format.
    if let Some(ca) = ca_pem {
        let mut reader = std::io::BufReader::new(ca.as_bytes());
        for cert in rustls_pemfile::certs(&mut reader) {
            root_store.add(cert?)?;
        }
    }

    let builder = ClientConfig::builder();

    /// Builds the `rustls::ClientConfig`, optionally disabling certificate verification.
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

    /// Add ALPN protocol for PostgreSQL, which is required by some providers like Supabase.
    config.alpn_protocols = vec![b"postgresql".to_vec()];

    Ok(MakeRustlsConnect::new(config))
}

// Helper to create a Native-TLS connector
/// # Create `native-tls` Connector
///
/// Helper function to create a `NativeMakeTlsConnector` for `tokio-postgres`
/// using the `native-tls` library.
///
/// This connector is primarily used for specific scenarios, such as connecting
/// to Supabase instances where `rustls` might encounter trust store issues,
/// or when `reject_unauthorized` is explicitly set to `false`.
///
/// # Arguments
/// * `ca_pem` - An optional PEM-encoded CA certificate string to add to the trust store.
/// * `reject_unauthorized` - An `Option<bool>` to explicitly control server certificate validation.
///   If `Some(false)`, certificate validation is disabled.
///
/// # Returns
/// A `Result<NativeMakeTlsConnector>` on success, or an `anyhow::Error` if the
/// `native-tls` connector cannot be built.
fn create_native_connector(
    ca_pem: Option<&str>,
    reject_unauthorized: Option<bool>,
) -> Result<NativeMakeTlsConnector> {
    let mut builder = NativeTlsConnector::builder();

    /// If `reject_unauthorized` is `false`, invalid certificates will be accepted.
    if let Some(false) = reject_unauthorized {
        builder.danger_accept_invalid_certs(true);
    }

    /// Add custom CA if provided in PEM format.
    if let Some(ca) = ca_pem {
        let cert = NativeTlsCertificate::from_pem(ca.as_bytes())?;
        builder.add_root_certificate(cert);
    }

    let connector = builder.build().context("Failed to build native-tls connector")?;
    Ok(NativeMakeTlsConnector::new(connector))
}

/// # Check PostgreSQL Instance
///
/// Connects to a PostgreSQL instance defined by `PostgresConfig` and executes a simple
/// query (`SELECT NOW(), version()`) to verify its connectivity and health.
///
/// It dynamically configures the appropriate TLS connector (`rustls` or `native-tls`)
/// based on the connection URL (e.g., special handling for Supabase) and SSL settings.
///
/// # Arguments
/// * `entry` - A reference to the `PostgresConfig` defining the instance to check.
///
/// # Returns
/// A `Result<(chrono::DateTime<chrono::Utc>, String)>` containing the server's current time
/// and version string on success, or an `anyhow::Error` on failure to connect or query.
async fn check_postgres(entry: &PostgresConfig) -> Result<(chrono::DateTime<chrono::Utc>, String)> {
    // Prepare configuration and TLS connector based on entry type
    /// Determines the PostgreSQL connection configuration and appropriate TLS connector
    /// based on whether a `dbUrl` or `dbConnection` object is provided.
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

    /// Connects to the PostgreSQL database using the configured `tokio_postgres::Config`
    /// and the appropriate TLS connector (or `NoTls`).
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

    /// Executes a simple query to get the server's current time and version.
    /// `simple_query` is used to avoid prepared statements, which may not be supported
    /// by certain PostgreSQL proxy configurations (e.g., Supabase transaction poolers).
    let messages = client
        .simple_query("SELECT NOW(), version()")
        .await
        .context("Failed to execute test query on PostgreSQL")?;

    let mut server_time: Option<chrono::DateTime<chrono::Utc>> = None;
    let mut server_version: Option<String> = None;

    /// Parses the results from the query, extracting the server time and version.
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

    /// Returns the parsed server time and version, or an error if the query returned no data.
    if let (Some(time), Some(version)) = (server_time, server_version) {
        Ok((time, version))
    } else {
        Err(anyhow::anyhow!("Query returned no data rows"))
    }
}

/// # Send Alert
///
/// Sends an alert notification to configured alert servers (primary and failover)
/// when a PostgreSQL instance check fails.
///
/// It constructs a JSON payload containing the current timestamp, alert information,
/// and the error message, then attempts to `POST` this payload to the primary
/// alert server. If the primary fails, it attempts to send the alert to the
/// failover server.
///
/// # Arguments
/// * `alerts` - A reference to `AlertsConfig` containing the primary and failover server details.
/// * `instance` - The name of the PostgreSQL instance that failed.
/// * `error_msg` - A string containing the error message from the failed check.
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
/// # Main Entry Point
///
/// This is the main function for the PostgreSQL monitoring utility.
/// It continuously checks the health of configured PostgreSQL instances
/// and sends alerts if any instance is found to be unhealthy.
///
/// ## Workflow:
/// 1.  Installs the default `rustls` crypto provider.
/// 2.  Parses command-line arguments for monitoring frequency and alert suppression.
/// 3.  Sets up structured logging.
/// 4.  Retrieves and parses the cloud-based monitoring configuration, including
///     PostgreSQL instances and alert server details.
/// 5.  Enters an infinite loop:
///     -   Iterates through each configured PostgreSQL instance.
///     -   Calls `check_postgres` to verify instance health.
///     -   Logs the health status or detailed error.
///     -   If an instance fails and alerts are not suppressed, it calls `send_alert`.
///     -   Pauses for the specified monitoring interval before the next cycle.
///
/// # Returns
/// An `anyhow::Result<()>` indicating the overall success or failure of the monitoring process.
async fn main() -> Result<()> {
    /// Explicitly installs the default `ring` crypto provider for `rustls`, required for TLS.
    let _ = rustls::crypto::ring::default_provider().install_default();

    /// Parses command-line arguments into an `Args` struct.
    let args = Args::parse();
    /// Initializes logging for the application.
    setup_logging().context("Failed to initialize logging")?;

    info!(
        "Starting PostgreSQL Monitor. Frequency: {} minute(s)",
        args.frequency
    );

    // 1. Download Configuration
    info!("Retrieving cloud configuration...");

    /// Loads the cloud-based configuration. `spawn_blocking` is used because `load_cloud_config`
    /// might be a blocking I/O operation and should not block the Tokio runtime.
    let config_json = tokio::task::spawn_blocking(move || load_cloud_config(None, None)).await??;

    // 2. Parse Configuration
    let mut postgres_instances = Vec::new();

    /// Parses the PostgreSQL instance configurations from the cloud configuration.
    /// Filters for active and monitorable instances.
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

    /// Parses the alert servers configuration from the cloud configuration.
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

    /// Combines parsed configurations into a `MonitorConfig` struct.
    let config = MonitorConfig {
        postgres_instances,
        alerts: alerts_config,
    };

    if config.postgres_instances.is_empty() {
        warn!("No PostgreSQL instances found in configuration to monitor.");
    }

    /// Calculates the monitoring interval duration from minutes to seconds.
    let interval_duration = Duration::from_secs(args.frequency * 60);

    /// Main monitoring loop.
    loop {
        info!(
            "Checking {} PostgreSQL instances...",
            config.postgres_instances.len()
        );

        /// Iterates through each configured PostgreSQL instance and performs a health check.
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
                    /// Masks sensitive information from the connection URL for logging error details.
                    let connection_info = if let Some(url) = &instance_entry.db_url {
                        mask_url_password(url)
                    } else if let Some(obj) = &instance_entry.db_connection {
                        format!("Host: {}", obj.host)
                    } else {
                        "Unknown connection info".to_string()
                    };

                    error!("Instance {} FAILED. Connection: {}. Error: {:#}", display_name, connection_info, e);

                    /// Sends an alert if alerts are not suppressed and alert servers are configured.
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

        /// Pauses for the configured interval before the next monitoring cycle.
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
