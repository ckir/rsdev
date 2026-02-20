//! # Redis Monitoring Service
//!
//! This utility performs periodic health checks on Redis instances defined in
//! the cloud configuration. It utilizes `lib_common` for encrypted configuration
//! retrieval and handles alerting via HTTP webhooks upon connection failures.

use anyhow::{Context, Result};
use clap::Parser;
// // Statement: Ensure lib_common is compiled with --features "configs" to resolve E0433
use lib_common::configs::config_cloud::load_cloud_config;
use log::{error, info, warn};
use redis::Client;
use serde::Deserialize;
use serde_json::Value;
use std::time::Duration;
use tokio::time::sleep;

/// Command-line arguments for the Redis monitor.
#[derive(Parser, Debug)]
#[command(author, version, about = "Monitors Redis instances hosted in the cloud", long_about = None)]
pub struct Args {
    /// Testing frequency in minutes.
    #[arg(short, long, default_value_t = 1)]
    pub frequency: u64,
}

/// Represents a single Redis database entry from the cloud configuration.
#[derive(Deserialize, Debug)]
pub struct RedisEntry {
    /// The display name of the database.
    #[serde(rename = "dbName")]
    pub db_name: String,
    /// The connection string (URL) for the Redis instance.
    #[serde(rename = "dbUrl")]
    pub db_url: String,
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
    /// List of Redis connection strings to check.
    pub redis_instances: Vec<(String, String)>,
    /// Optional alerting configuration.
    pub alerts: Option<AlertsConfig>,
}

/// Initializes the logging system using `fern`.
pub fn setup_logging() -> Result<()> {
    // // Statement: Generate a timestamped log filename
    let log_filename = format!("monitor_redis_{}.log", chrono::Local::now().format("%Y-%m-%d"));
    
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

    info!("Starting Redis monitor. Frequency: {} minute(s)", args.frequency);

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

        let mut redis_urls = Vec::new();
        // // Statement: Parse Redis instances from JSON pointer
        if let Some(cloud_config) = config_json.pointer("/commonAll/db/redis/cloud") {
            if let Ok(entries) = serde_json::from_value::<Vec<RedisEntry>>(cloud_config.clone()) {
                for entry in entries.into_iter().filter(|e| e.monitor) {
                    redis_urls.push((entry.db_name, entry.db_url));
                }
            }
        }

        let mut alerts = None;
        // // Statement: Parse alert configuration if available
        if let Some(alerts_json) = config_json.pointer("/commonAll/alerts") {
            if let Ok(cfg) = serde_json::from_value::<AlertsConfig>(alerts_json.clone()) {
                alerts = Some(cfg);
            }
        }

        // // Statement: Iterate through configured instances and verify connectivity
        for (name, url) in redis_urls {
            match Client::open(url.clone()) {
                Ok(client) => {
                    match client.get_connection() {
                        Ok(_) => info!("SUCCESS: Redis instance '{}' is reachable.", name),
                        Err(e) => {
                            error!("FAILURE: Redis '{}' unreachable: {}", name, e);
                            let masked_url = url.split('@').last().unwrap_or(&url);
                            if let Some(ref alert_cfg) = alerts {
                                send_alert(alert_cfg, &name, &format!("{} - {}", masked_url, e)).await;
                            }
                        }
                    }
                }
                Err(e) => error!("Configuration error for '{}': {}", name, e),
            }
        }

        sleep(interval_duration).await;
    }
}

/// Dispatches an alert to the configured primary server, with failover logic.
///
/// # Arguments
/// * `config` - The alerting configuration containing primary and failover servers.
/// * `subject` - The subject/title of the alert.
/// * `message` - The error details.
pub async fn send_alert(config: &AlertsConfig, subject: &str, message: &str) {
    let client = reqwest::Client::new();
    let payload = serde_json::json!({
        "subject": format!("REDIS MONITOR: {}", subject),
        "message": message
    });

    // // Statement: Execute HTTP POST to primary endpoint with fallback logic
    match client.post(&config.primary.host).json(&payload).send().await {
        Ok(_) => info!("Alert sent to primary: {}", config.primary.name),
        Err(e) => {
            warn!("Primary alert failed: {}. Trying failover...", e);
            if let Err(fe) = client.post(&config.failover.host).json(&payload).send().await {
                error!("Failover alert also failed: {}", fe);
            } else {
                info!("Alert sent to failover: {}", config.failover.name);
            }
        }
    }
}