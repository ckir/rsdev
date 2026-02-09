use anyhow::{Context, Result};
use clap::Parser;
use lib_common::config_cloud::load_cloud_config;
use log::{error, info, warn};
use redis::Client;
use serde::Deserialize;
use std::time::Duration;
use tokio::time::sleep;

#[derive(Parser, Debug)]
#[command(author, version, about = "Monitors Redis instances hosted in the cloud", long_about = None)]
struct Args {
    /// Testing frequency in minutes. The monitor will check instances at this interval.
    #[arg(short, long, default_value_t = 1)]
    frequency: u64,
}

#[derive(Deserialize, Debug)]
/// # Redis Entry
///
/// Represents a single Redis instance configured for monitoring.
struct RedisEntry {
    /// The human-readable name of the Redis database.
    #[serde(rename = "dbName")]
    db_name: String,
    /// The connection URL for the Redis instance (e.g., `redis://:password@host:port/db`).
    #[serde(rename = "dbUrl")]
    db_url: String,
    /// Whether this Redis instance should be monitored. Defaults to `false` if not specified.
    #[serde(default)]
    monitor: bool,
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
/// Top-level configuration for the Redis monitor, containing a list of instance URLs to check
/// and details for sending alerts.
struct MonitorConfig {
    /// A list of Redis connection URLs for instances to be monitored.
    redis_instances: Vec<String>,
    /// Optional configuration for primary and failover alert servers.
    alerts: Option<AlertsConfig>,
}

fn setup_logging() -> Result<()> {
    /// Constructs the log filename using the current date.
    let log_filename = format!("monitor_redis_{}.log", chrono::Local::now().format("%Y-%m-%d"));
    /// Configures the `fern` logger to output to both standard error and a daily rotating file.
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
        .chain(fern::log_file(&log_filename)?)
        .apply()?;
    Ok(())
}

/// # Mask URL Password
///
/// Masks the password component within a Redis connection URL string
/// for safe logging or display.
///
/// It supports `redis://` and `rediss://` schemes. The password, typically
/// found between `user:` (optional) and `@host:port`, is replaced with `*****`.
/// If no password pattern is found or the URL format is unexpected, the original
/// URL is returned.
///
/// # Arguments
/// * `url` - The Redis connection URL string.
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

/// # Check Redis Instance
///
/// Connects to a Redis instance using the provided connection string and
/// performs two basic checks to verify its health:
/// 1.  Executes a `PING` command to ensure connectivity and responsiveness.
/// 2.  Executes a `SET LASTCHECKED` command to verify write functionality.
///
/// # Arguments
/// * `connection_string` - The Redis connection URL (e.g., `redis://127.0.0.1/`).
///
/// # Returns
/// A `Result<()>` indicating success or an `anyhow::Error` on connection
/// or command execution failure.
async fn check_redis(connection_string: &str) -> Result<()> {
    /// Opens a new Redis client connection using the provided connection string.
    let client = Client::open(connection_string)?;
    /// Obtains a multiplexed asynchronous connection from the client.
    let mut con = client.get_multiplexed_async_connection().await?;

    /// Executes a `PING` command to check basic connectivity and responsiveness.
    let _: String = redis::cmd("PING")
        .query_async(&mut con)
        .await
        .context("Failed to execute PING")?;

    /// Gets the current UTC time to record the last successful check.
    let now = chrono::Utc::now().to_rfc3339();
    /// Executes a `SET` command to write a timestamp, verifying write operations.
    let _: () = redis::cmd("SET")
        .arg("LASTCHECKED")
        .arg(now)
        .query_async(&mut con)
        .await
        .context("Failed to set LASTCHECKED")?;

    Ok(())
}

/// # Send Alert
///
/// Sends an alert notification to configured alert servers (primary and failover)
/// when a Redis instance check fails.
///
/// It constructs a JSON payload containing the current timestamp, alert information,
/// and the error message, then attempts to `POST` this payload to the primary
/// alert server. If the primary fails, it attempts to send the alert to the
/// failover server.
///
/// # Arguments
/// * `alerts` - A reference to `AlertsConfig` containing the primary and failover server details.
/// * `instance` - The identifier (e.g., masked URL) of the Redis instance that failed.
/// * `error_msg` - A string containing the error message from the failed check.
async fn send_alert(alerts: &AlertsConfig, instance: &str, error_msg: &str) {
    let client = reqwest::Client::new();

    let now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
    let message = format!("Redis instance {} failed. Error: {}", instance, error_msg);

    let payload = serde_json::json!({
        "Date": now,
        "Info": "Redis Monitor Alert",
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
/// This is the main function for the Redis monitoring utility.
/// It continuously checks the health of configured Redis instances
/// and sends alerts if any instance is found to be unhealthy.
///
/// ## Workflow:
/// 1.  Parses command-line arguments for monitoring frequency.
/// 2.  Sets up structured logging.
/// 3.  Retrieves and parses the cloud-based monitoring configuration, including
///     Redis instances and alert server details.
/// 4.  Enters an infinite loop:
///     -   Iterates through each configured Redis instance URL.
///     -   Calls `check_redis` to verify instance health (PING and SET operations).
///     -   Logs the health status or detailed error.
///     -   If an instance fails, it calls `send_alert` (if configured).
///     -   Pauses for the specified monitoring interval before the next cycle.
///
/// # Returns
/// A `Result<()>` indicating the overall success or failure of the monitoring process.
async fn main() -> Result<()> {
    /// Parses command-line arguments into an `Args` struct.
    let args = Args::parse();
    /// Initializes logging for the application.
    setup_logging().context("Failed to initialize logging")?;

    info!(
        "Starting Redis Monitor. Frequency: {} minute(s)",
        args.frequency
    );

    // 1. Download Configuration
    info!("Retrieving cloud configuration...");

    /// Loads the cloud-based configuration. `spawn_blocking` is used because `load_cloud_config`
    /// might be a blocking I/O operation and should not block the Tokio runtime.
    let config_json = tokio::task::spawn_blocking(move || load_cloud_config(None, None)).await??;

    // Debug: Print the decoded JSON
    // info!("Decoded Configuration: {}", serde_json::to_string_pretty(&config_json).unwrap_or_else(|_| "Invalid JSON".to_string()));

    // 2. Parse Configuration
    let mut redis_instances = Vec::new();

    /// Parses the Redis instance configurations from the cloud configuration.
    /// Filters for active and monitorable instances and collects their URLs.
    if let Some(cloud_config) = config_json.pointer("/commonAll/db/redis/cloud") {
        match serde_json::from_value::<Vec<RedisEntry>>(cloud_config.clone()) {
            Ok(entries) => {
                for entry in entries {
                    if entry.monitor {
                        info!(
                            "Found Redis instance to monitor: {} ({})",
                            entry.db_name,
                            mask_url_password(&entry.db_url)
                        );
                        redis_instances.push(entry.db_url);
                    }
                }
            }
            Err(e) => warn!("Failed to parse redis cloud config: {}", e),
        }
    } else {
        warn!("Configuration path /commonAll/db/redis/cloud not found");
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
        redis_instances,
        alerts: alerts_config,
    };

    if config.redis_instances.is_empty() {
        warn!("No Redis instances found in configuration.");
    }

    /// Calculates the monitoring interval duration from minutes to seconds.
    let interval_duration = Duration::from_secs(args.frequency * 60);

    /// Main monitoring loop.
    loop {
        info!(
            "Checking {} Redis instances...",
            config.redis_instances.len()
        );

        /// Iterates through each configured Redis instance and performs a health check.
        for instance_url in &config.redis_instances {
            let masked_url = mask_url_password(instance_url);
            match check_redis(instance_url).await {
                Ok(_) => {
                    info!("Instance {} is HEALTHY", masked_url);
                }
                Err(e) => {
                    error!("Instance {} FAILED: {}", masked_url, e);

                    /// Sends an alert if alert servers are configured.
                    if let Some(ref alerts) = config.alerts {
                        send_alert(alerts, &masked_url, &e.to_string()).await;
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

/*
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_send_real_alert() {
        // Initialize logging for the test to see output
        let _ = fern::Dispatch::new()
            .level(log::LevelFilter::Info)
            .chain(std::io::stdout())
            .apply();

        let alerts_config = AlertsConfig {
            primary: AlertServer {
                host: "https://script.google.com/macros/s/AKfycbyU8lkjhTH4oJOAif6C6BXabJP7J_lJ9VfL34I8Gd7NrXlW3MU73IEzLuYREly5uDlUSg/exec".to_string(),
                name: "Primary Test Server".to_string(),
            },
            failover: AlertServer {
                host: "https://script.google.com/macros/s/AKfycbyU8lkjhTH4oJOAif6C6BXabJP7J_lJ9VfL34I8Gd7NrXlW3MU73IEzLuYREly5uDlUSg/exec".to_string(),
                name: "Failover Test Server".to_string(),
            },
        };

        info!("Starting test_send_real_alert...");
        send_alert(
            &alerts_config,
            "test-redis-instance",
            "This is a DEBUG test alert triggered from the Rust test suite."
        ).await;
        info!("Finished test_send_real_alert.");
    }
}

*/
