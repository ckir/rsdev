//! # NASDAQ Market Status Checker
//!
//! This module provides functionality to query the NASDAQ API for the current
//! status of the U.S. stock market. It determines whether the market is open,
//! closed, or in a pre-market/after-hours session.
//!
//! ## Core Components:
//! - **`MarketStatusData`**: A struct that represents the deserialized data
//!   from the NASDAQ API response. It includes market times, current status,
//!   and trading day information.
//! - **`MarketStatus`**: A client struct responsible for making the API call
//!   and parsing the response into `MarketStatusData`.
//!
//! The module is crucial for any process that needs to act based on market hours,
//! such as starting or stopping data ingestors at the correct times.

#![doc(html_logo_url = "https://example.com/logo.png")] // Placeholder
#![forbid(unsafe_code)]
#![warn(missing_docs, rust_2018_idioms, unused_qualifications)]

use crate::markets::nasdaq::apicall::ApiCall;
use crate::loggers::loggerlocal::LoggerLocal;
use chrono::{NaiveDateTime, NaiveDate, Utc, Duration};
use chrono_tz::US::Eastern;
use serde::{Deserialize, Serialize};
use serde_json::{from_value, Value};
use std::sync::Arc;

/// # NASDAQ Market Status Data
///
/// Represents the structured data returned from the NASDAQ Market Info API endpoint.
///
/// This struct holds all relevant information about the current state of the U.S. market,
/// including various session times and status indicators. It uses `serde` for robust
/// deserialization from the JSON payload.
///
/// The `rename_all = "camelCase"` attribute is important as the NASDAQ API uses camelCase
/// for its JSON keys, while Rust convention is snake_case.
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct MarketStatusData {
    /// The country code for the market (e.g., "US").
    pub country: String,
    /// A technical indicator for the market's state (e.g., "M").
    pub market_indicator: String,
    /// A human-readable market state indicator (e.g., "Market Open").
    pub ui_market_indicator: String,
    /// A string representing the countdown to the next market event.
    pub market_count_down: String,
    /// The start time of the pre-market session (e.g., "04:00").
    pub pre_market_opening_time: String,
    /// The end time of the pre-market session (e.g., "09:30").
    pub pre_market_closing_time: String,
    /// The official market opening time (e.g., "09:30").
    pub market_opening_time: String, 
    /// The official market closing time (e.g., "16:00").
    pub market_closing_time: String,
    /// The start time of the after-hours session (e.g., "16:00").
    pub after_hours_market_opening_time: String,
    /// The end time of the after-hours session (e.g., "20:00").
    pub after_hours_market_closing_time: String,
    /// The date of the previous trading day.
    pub previous_trade_date: String,
    /// The date of the next upcoming trading day.
    pub next_trade_date: String,
    /// A boolean indicating if the current day is a trading day.
    pub is_business_day: bool,
    /// A concise status string (e.g., "Open", "Closed").
    pub mrkt_status: String,
    /// A countdown string tied to `mrkt_status`.
    pub mrkt_count_down: String,
    
    /// Raw `NaiveDateTime` for the pre-market open, parsed from the API.
    /// This is used for precise time calculations.
    #[serde(rename = "pmOpenRaw")]
    pub pm_open_raw: NaiveDateTime,
    /// Raw `NaiveDateTime` for the after-hours close.
    #[serde(rename = "ahCloseRaw")]
    pub ah_close_raw: NaiveDateTime,
    /// Raw `NaiveDateTime` for the regular market open.
    #[serde(rename = "openRaw")]
    pub open_raw: NaiveDateTime,
    /// Raw `NaiveDateTime` for the regular market close.
    #[serde(rename = "closeRaw")]
    pub close_raw: NaiveDateTime,
}

/// # Market Status Client
///
/// A client for fetching and interpreting the NASDAQ market status.
///
/// This struct encapsulates the logic for making the API request. It requires
/// an `ApiCall` instance to handle the underlying HTTP communication and a
/// `LoggerLocal` for logging outcomes.
pub struct MarketStatus {
    /// An `Arc`'d `ApiCall` instance for making HTTP requests to the NASDAQ API.
    api_call: Arc<ApiCall>,
    /// A shared logger for recording status checks, successes, and failures.
    logger: Arc<LoggerLocal>,
}

impl MarketStatus {
    /// Creates a new `MarketStatus` client.
    ///
    /// # Arguments
    /// * `api_call` - A shared `ApiCall` instance for network requests.
    /// * `logger` - A shared `LoggerLocal` for structured logging.
    pub fn new(api_call: Arc<ApiCall>, logger: Arc<LoggerLocal>) -> Self {
        Self { api_call, logger }
    }

    /// # Fetch Market Status
    ///
    /// Asynchronously queries the NASDAQ API to get the latest market status.
    ///
    /// ## Logic:
    /// 1.  Constructs the API path for the market info endpoint.
    /// 2.  Uses `ApiCall` to fetch the data.
    /// 3.  On success, attempts to deserialize the JSON payload into `MarketStatusData`.
    /// 4.  Logs success or failure outcomes. A `fatal` log is generated if the
    ///     payload does not match the expected `MarketStatusData` struct, as this
    ///     indicates a breaking change in the API.
    ///
    /// # Returns
    /// A `Result` containing either the successfully parsed `MarketStatusData` or a
    /// dynamic error, which ensures that error information is propagated up the call stack.
    pub async fn get_status(&self) -> Result<MarketStatusData, Box<dyn std::error::Error + Send + Sync>> {
        let path = "api/market-info";

        let raw_json: Value = self.api_call.fetch_nasdaq(path).await
            .map_err(|e| format!("Nasdaq API Fetch Error: {}", e))?;

        // The actual data is often nested under a "data" key. This handles that case.
        let data_part = raw_json.get("data").unwrap_or(&raw_json);

        // Attempt to deserialize the JSON into our strict struct.
        match from_value::<MarketStatusData>(data_part.clone()) {
            Ok(data) => {
                self.logger.debug("Market status schema validated successfully", None).await;
                Ok(data)
            }
            Err(e) => {
                // If deserialization fails, it's a critical error. The API has likely changed.
                let error_message = format!("STRICT SCHEMA VALIDATION FAILED: {}", e);
                self.logger.fatal(&error_message, Some(serde_json::json!({"payload": raw_json}))).await;
                Err(error_message.into())
            }
        }
    }
}

impl MarketStatusData {
    /// # Get Current New York Time
    ///
    /// A helper function to get the current time as a `NaiveDateTime` in the
    /// US/Eastern timezone. This is critical for accurately comparing against
    /// market hours, which are always based on NY time.
    fn now_ny(&self) -> NaiveDateTime {
        Utc::now().with_timezone(&Eastern).naive_local()
    }

    /// # Calculate Sleep Duration
    ///
    /// Determines how long a process should sleep before the next market event.
    ///
    /// This logic is essential for an ingestor that needs to "wake up" right
    /// before the market opens to start streaming data.
    ///
    /// ## Logic:
    /// - If the market is already "Open", returns 0 duration.
    /// - It checks if the current time is before the pre-market or regular open.
    /// - If today's market times are in the past (e.g., on a weekend), it parses
    ///   `next_trade_date` and calculates the duration until the pre-market open
    ///   (04:00 AM NY) on that future date.
    /// - If a valid future target time is found, it returns the `Duration` until then.
    /// - As a fallback, it returns a default duration (5 minutes or 1 minute)
    ///   to ensure the process doesn't get stuck.
    pub fn get_sleep_duration(&self) -> std::time::Duration {
        let now = self.now_ny();
        
        // If the market is open, no need to sleep.
        if self.mrkt_status == "Open" {
            return std::time::Duration::from_secs(0);
        }

        // Determine the next event: pre-market open or regular open.
        let mut target = if now < self.pm_open_raw { 
            self.pm_open_raw 
        } else { 
            self.open_raw 
        };

        // Handle weekends/holidays where the target time is in the past.
        if target <= now {
            let fmt = "%b %d, %Y";
            if let Ok(next_date) = NaiveDate::parse_from_str(&self.next_trade_date, fmt) {
                // Anchor the next event to 4:00 AM NY time on the next business day.
                if let Some(anchored) = next_date.and_hms_opt(4, 0, 0) {
                    target = anchored;
                }
            }
        }

        // Calculate the duration if the target is in the future.
        if target > now {
            let diff = target - now;
            println!("Target NY Open: {} ({} remaining)", 
                target.format("%Y-%m-%d %H:%M:%S"), 
                Self::format_duration(diff)
            );
            // Convert from chrono::Duration to std::time::Duration.
            diff.to_std().unwrap_or(std::time::Duration::from_secs(60)) // Fallback
        } else {
            // Fallback if no future time could be determined.
            std::time::Duration::from_secs(300) 
        }
    }

    /// # Format Duration
    ///
    /// A utility function to format a `chrono::Duration` into a human-readable
    /// HH:MM:SS string.
    pub fn format_duration(dur: Duration) -> String {
        let total_secs = dur.num_seconds();
        let hours = total_secs / 3600;
        let mins = (total_secs % 3600) / 60;
        let secs = total_secs % 60;
        format!("{:02}:{:02}:{:02}", hours, mins, secs)
    }
}