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
    /// Testing frequency in minutes
    #[arg(short, long, default_value_t = 1)]
    frequency: u64,
}

#[derive(Deserialize, Debug)]
struct RedisEntry {
    #[serde(rename = "dbName")]
    db_name: String,
    #[serde(rename = "dbUrl")]
    db_url: String,
    #[serde(default)]
    monitor: bool,
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
    redis_instances: Vec<String>,
    alerts: Option<AlertsConfig>,
}

fn setup_logging() -> Result<()> {
    let log_filename = format!("monitor_redis_{}.log", chrono::Local::now().format("%Y-%m-%d"));
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

/// Masks the password in a Redis connection string.
/// Supports redis:// and rediss:// schemes.
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

async fn check_redis(connection_string: &str) -> Result<()> {
    let client = Client::open(connection_string)?;
    let mut con = client.get_multiplexed_async_connection().await?;

    let _: String = redis::cmd("PING")
        .query_async(&mut con)
        .await
        .context("Failed to execute PING")?;

    let now = chrono::Utc::now().to_rfc3339();
    let _: () = redis::cmd("SET")
        .arg("LASTCHECKED")
        .arg(now)
        .query_async(&mut con)
        .await
        .context("Failed to set LASTCHECKED")?;

    Ok(())
}

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
async fn main() -> Result<()> {
    let args = Args::parse();
    setup_logging().context("Failed to initialize logging")?;

    info!(
        "Starting Redis Monitor. Frequency: {} minute(s)",
        args.frequency
    );

    // 1. Download Configuration
    info!("Retrieving cloud configuration...");

    // Wrap the blocking call in spawn_blocking to avoid panicking the async runtime
    let config_json = tokio::task::spawn_blocking(move || load_cloud_config(None, None)).await??;

    // Debug: Print the decoded JSON
    // info!("Decoded Configuration: {}", serde_json::to_string_pretty(&config_json).unwrap_or_else(|_| "Invalid JSON".to_string()));

    // 2. Parse Configuration
    let mut redis_instances = Vec::new();

    // Navigate to config.commonAll.db.redis.cloud
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
        redis_instances,
        alerts: alerts_config,
    };

    if config.redis_instances.is_empty() {
        warn!("No Redis instances found in configuration.");
    }

    let interval_duration = Duration::from_secs(args.frequency * 60);

    loop {
        info!(
            "Checking {} Redis instances...",
            config.redis_instances.len()
        );

        for instance_url in &config.redis_instances {
            let masked_url = mask_url_password(instance_url);
            match check_redis(instance_url).await {
                Ok(_) => {
                    info!("Instance {} is HEALTHY", masked_url);
                }
                Err(e) => {
                    error!("Instance {} FAILED: {}", masked_url, e);

                    if let Some(ref alerts) = config.alerts {
                        send_alert(alerts, &masked_url, &e.to_string()).await;
                    } else {
                        warn!("No alert servers configured. Alert suppressed.");
                    }
                }
            }
        }

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
