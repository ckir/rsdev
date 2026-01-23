#![allow(dead_code)]
#![allow(unused_variables)]
#![allow(unused_imports)]
#![allow(unreachable_code)]

use serde_derive::Deserialize;
use serde_derive::Serialize;
use serde_json::Value;

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MarketStatus {
    pub data: Data,
    pub message: Value,
    pub status: Status,
    pub ts: Option<String>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Data {
    pub country: String,
    pub market_indicator: String,
    pub ui_market_indicator: String,
    pub market_count_down: String,
    pub pre_market_opening_time: String,
    pub pre_market_closing_time: String,
    pub market_opening_time: String,
    pub market_closing_time: String,
    pub after_hours_market_opening_time: String,
    pub after_hours_market_closing_time: String,
    pub previous_trade_date: String,
    pub next_trade_date: String,
    pub is_business_day: bool,
    pub mrkt_status: String,
    pub mrkt_count_down: String,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    pub r_code: i64,
    pub b_code_message: Value,
    pub developer_message: Value,
}

use anyhow::Result;
use std::env;

use tracing::{debug, error, info, trace, warn};
use tracing_appender::{non_blocking, rolling};
use tracing_subscriber::{EnvFilter, fmt, layer::SubscriberExt, prelude::*};

use tokio::io;
use tokio::time::{Duration, Instant, sleep_until};
use tokio_cron_scheduler::{Job, JobBuilder, JobScheduler, JobSchedulerError};

use chrono::TimeZone;
use chrono_tz::America::New_York;
use chrono_tz::Europe::London;

use lib_common::sys_info::{ProcessInfo, ProcessInfoError, get_process_info};
#[path = "./libnasdaq.rs"]
mod libnasdaq;
use libnasdaq::*;

// load .env files before anything else
use static_init::dynamic;

#[dynamic]
static DOTENV_INIT: () = {
    // Set up environment variables
    dotenvy::dotenv().ok();
};

#[dynamic]
pub static PROCESSINFO: Result<ProcessInfo, ProcessInfoError> = get_process_info();

fn setup_logging() -> io::Result<()> {
    // Get log level from environment variable or use default
    let log_level: String = env::var("RUST_LOG").unwrap_or_else(|_| "info".to_string());

    // Get log directory from environment variable or use default
    let log_dir: String = env::var("LOG_DIR").unwrap_or_else(|_| "logs".to_string());

    // Create log directory if it doesn't exist
    std::fs::create_dir_all(&log_dir)?;

    // Configure file appender for rotating log files daily
    let process_basename: &String = match &*PROCESSINFO {
        Ok(process_info) => &process_info.process_basename,
        Err(e) => {
            eprintln!("Failed to retrieve process info: {}", e);
            std::process::exit(1);
        }
    };
    let file_appender = rolling::daily(&log_dir, process_basename.as_str());
    let (non_blocking_appender, _guard) = non_blocking(file_appender);

    // Store the guard in a static to keep it alive for the duration of the program
    // This prevents the non-blocking writer from being dropped prematurely
    static mut GUARD: Option<tracing_appender::non_blocking::WorkerGuard> = None;
    unsafe {
        GUARD = Some(_guard);
    }

    // Create console layer for stdout
    let console_layer = fmt::layer().with_target(true).with_ansi(true);

    // Create JSON-formatted file layer
    let file_layer = fmt::layer()
        .with_ansi(false)
        .with_writer(non_blocking_appender)
        .json();

    // Create environment filter from log level
    let env_filter: EnvFilter = EnvFilter::try_from_default_env()
        .or_else(|_| EnvFilter::try_new(&log_level))
        .unwrap();

    // Combine all layers
    tracing_subscriber::registry()
        .with(env_filter)
        .with(console_layer)
        .with(file_layer)
        .init();

    info!("Logging initialized with level: {}", log_level);
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    // Set up logging
    if let Err(e) = setup_logging() {
        eprintln!("Failed to initialize logging: {}", e);
        std::process::exit(1);
    }

    match &*PROCESSINFO {
        Ok(process_info) => {
            info!("{}", process_info);
        }
        Err(e) => {
            error!("Failed to retrieve process info: {}", e);
            std::process::exit(1);
        }
    }

    let scheduler = JobScheduler::new().await.unwrap();

    // Your main logic here
    let job = JobBuilder::new()
        .with_timezone(New_York)
        .with_cron_job_type()
        .with_schedule("0 * * * * *")
        .unwrap()
        .with_run_async(Box::new(|uuid, mut l| {
            Box::pin(async move {
                // info!("JHB run async every 2 seconds id {:?}", uuid);
                let url = "https://api.nasdaq.com/api/market-info";
                match fetch_text(url).await {
                    Ok(text) => println!("{}", text),
                    Err(e) => {
                        eprintln!("Error fetching [{}]: {}", url, e);
                        std::process::exit(1);
                    }
                }
                let next_tick = l.next_tick_for_job(uuid).await;
                match next_tick {
                    Ok(Some(ts)) => info!("Next time for JHB 2s is {:?}", ts),
                    _ => warn!("Could not get next tick for 2s job"),
                }
            })
        }))
        .build()
        .unwrap();

    scheduler.add(job).await.unwrap();

    // Start the scheduler
    scheduler.start().await.unwrap();
    // Keep the program running
    loop {
        sleep_until(Instant::now() + Duration::from_secs(60)).await;
    }

    Ok(())
}
