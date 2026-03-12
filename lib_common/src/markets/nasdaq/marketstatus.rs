//! # Nasdaq Market Status
//!
//! This component handles Nasdaq API calls to determine market operational status.
//! Refactored to use the standardized `LoggerLocal` instead of standard output.

use crate::loggers::loggerlocal::LoggerLocal;
use crate::markets::nasdaq::apicall::ApiCall;
use chrono::{Duration, NaiveDate, NaiveDateTime, Utc};
use chrono_tz::US::Eastern;
use serde::{Deserialize, Serialize};
use serde_json::{from_value, Value};
use std::sync::Arc;

/// Data structure representing the status of the Nasdaq market.
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct MarketStatusData {
    /// The country code associated with the market.
    pub country: String,
    /// The primary market status indicator.
    pub market_indicator: String,
    /// The user-friendly market status indicator for display.
    pub ui_market_indicator: String,
    /// Countdown string until the next market event.
    pub market_count_down: String,
    /// String representation of the pre-market opening time.
    pub pre_market_opening_time: String,
    /// String representation of the pre-market closing time.
    pub pre_market_closing_time: String,
    /// String representation of the main market opening time.
    pub market_opening_time: String,
    /// String representation of the main market closing time.
    pub market_closing_time: String,
    /// String representation of the after-hours market opening time.
    pub after_hours_market_opening_time: String,
    /// String representation of the after-hours market closing time.
    pub after_hours_market_closing_time: String,
    /// Date of the previous trading session.
    pub previous_trade_date: String,
    /// Date of the next scheduled trading session.
    pub next_trade_date: String,
    /// Boolean flag indicating if today is a business day for the market.
    pub is_business_day: bool,
    /// Short status string (e.g., "Open", "Closed").
    pub mrkt_status: String,
    /// Alternative countdown status.
    pub mrkt_count_down: String,

    /// Parsed pre-market opening time in New York timezone.
    #[serde(rename = "pmOpenRaw")]
    pub pm_open_raw: NaiveDateTime,
    /// Parsed after-hours closing time in New York timezone.
    #[serde(rename = "ahCloseRaw")]
    pub ah_close_raw: NaiveDateTime,
    /// Parsed main market opening time in New York timezone.
    #[serde(rename = "openRaw")]
    pub open_raw: NaiveDateTime,
    /// Parsed main market closing time in New York timezone.
    #[serde(rename = "closeRaw")]
    pub close_raw: NaiveDateTime,
}

/// Service to fetch and analyze Nasdaq market status.
pub struct MarketStatus {
    /// Shared Nasdaq API client.
    pub api_call: Arc<ApiCall>,
    /// Standardized local logger.
    pub logger: Arc<LoggerLocal>,
}

impl MarketStatus {
    /// Creates a new MarketStatus instance.
    pub fn new(api_call: Arc<ApiCall>, logger: Arc<LoggerLocal>) -> Self {
        Self { api_call, logger }
    }

    /// Fetches the current market status from Nasdaq and validates the schema.
    ///
    /// # Errors
    /// Returns an error if the API call fails or the response does not match the expected schema.
    pub async fn get_status(
        &self,
    ) -> Result<MarketStatusData, Box<dyn std::error::Error + Send + Sync>> {
        let path = "api/market-info";

        // // Fetching raw JSON from the Nasdaq endpoint
        let raw_json: Value = self
            .api_call
            .fetch_nasdaq(path)
            .await
            .map_err(|e| format!("Nasdaq API Fetch Error: {}", e))?;

        let data_part = raw_json.get("data").unwrap_or(&raw_json);

        // // Validating data against the MarketStatusData schema
        match from_value::<MarketStatusData>(data_part.clone()) {
            Ok(data) => {
                self.logger
                    .debug("Market status schema validated successfully", None)
                    .await;
                Ok(data)
            }
            Err(e) => {
                let error_message = format!("STRICT SCHEMA VALIDATION FAILED: {}", e);
                // // Logging fatal error with payload for post-mortem analysis
                self.logger
                    .fatal(
                        &error_message,
                        Some(serde_json::json!({"payload": raw_json})),
                    )
                    .await;
                Err(error_message.into())
            }
        }
    }

    /// Public wrapper to get sleep duration using the internal logger.
    ///
    /// # Arguments
    /// * `data` - The `MarketStatusData` used to calculate wait time.
    pub async fn calculate_wait(&self, data: &MarketStatusData) -> std::time::Duration {
        data.get_sleep_duration(self.logger.clone()).await
    }
}

impl MarketStatusData {
    /// Gets current time specifically in New York timezone.
    fn now_ny(&self) -> NaiveDateTime {
        Utc::now().with_timezone(&Eastern).naive_local()
    }

    /// Calculates how long the system should sleep before the market opens.
    ///
    /// # Parameters
    /// - `logger`: The Arc-wrapped LocalLogger to use for structured output.
    pub async fn get_sleep_duration(&self, logger: Arc<LoggerLocal>) -> std::time::Duration {
        let now = self.now_ny();

        // // If market is already open, no sleep is required
        if self.mrkt_status == "Open" {
            return std::time::Duration::from_secs(0);
        }

        // // Determine target from raw timestamps (Pre-market or Main open)
        let mut target = if now < self.pm_open_raw {
            self.pm_open_raw
        } else {
            self.open_raw
        };

        // // Handle weekends or holidays using next_trade_date
        if target <= now {
            let fmt = "%b %d, %Y";
            if let Ok(next_date) = NaiveDate::parse_from_str(&self.next_trade_date, fmt) {
                // // Point to 04:00 AM NY on the next trading day
                if let Some(anchored) = next_date.and_hms_opt(4, 0, 0) {
                    target = anchored;
                }
            }
        }

        if target > now {
            let diff = target - now;
            let remaining_str = Self::format_duration(diff);

            // // Refactored: Replaced println! with structured logging
            logger
                .info(
                    &format!("Target NY Open: {}", target.format("%Y-%m-%d %H:%M:%S")),
                    Some(serde_json::json!({
                        "remaining_time": remaining_str,
                        "target_timestamp": target.to_string(),
                        "current_ny_time": now.to_string()
                    })),
                )
                .await;

            diff.to_std().unwrap_or(std::time::Duration::from_secs(60))
        } else {
            // // Default fallback if logic fails to determine a future target
            std::time::Duration::from_secs(300)
        }
    }

    /// Formats a Duration into HH:MM:SS string.
    ///
    /// # Arguments
    /// * `dur` - The duration to format.
    pub fn format_duration(dur: Duration) -> String {
        let total_secs = dur.num_seconds();
        let hours = total_secs / 3600;
        let mins = (total_secs % 3600) / 60;
        let secs = total_secs % 60;
        format!("{:02}:{:02}:{:02}", hours, mins, secs)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::loggers::loggerlocal::LoggerLocal;
    use chrono::NaiveDateTime;
    use std::sync::Arc;

    fn mock_market_data() -> MarketStatusData {
        MarketStatusData {
            country: "US".into(),
            market_indicator: "Closed".into(),
            ui_market_indicator: "Closed".into(),
            market_count_down: "00:00:00".into(),
            pre_market_opening_time: "04:00 AM".into(),
            pre_market_closing_time: "09:30 AM".into(),
            market_opening_time: "09:30 AM".into(),
            market_closing_time: "04:00 PM".into(),
            after_hours_market_opening_time: "04:00 PM".into(),
            after_hours_market_closing_time: "08:00 PM".into(),
            previous_trade_date: "Mar 07, 2025".into(),
            next_trade_date: "Mar 10, 2025".into(),
            is_business_day: true,
            mrkt_status: "Closed".into(),
            mrkt_count_down: "00:00:00".into(),
            pm_open_raw: NaiveDateTime::parse_from_str("2025-03-10 04:00:00", "%Y-%m-%d %H:%M:%S")
                .unwrap(),
            ah_close_raw: NaiveDateTime::parse_from_str("2025-03-10 20:00:00", "%Y-%m-%d %H:%M:%S")
                .unwrap(),
            open_raw: NaiveDateTime::parse_from_str("2025-03-10 09:30:00", "%Y-%m-%d %H:%M:%S")
                .unwrap(),
            close_raw: NaiveDateTime::parse_from_str("2025-03-10 16:00:00", "%Y-%m-%d %H:%M:%S")
                .unwrap(),
        }
    }

    #[test]
    fn test_format_duration() {
        let dur = Duration::hours(1) + Duration::minutes(30) + Duration::seconds(45);
        assert_eq!(MarketStatusData::format_duration(dur), "01:30:45");

        let dur2 = Duration::seconds(59);
        assert_eq!(MarketStatusData::format_duration(dur2), "00:00:59");
    }

    #[tokio::test]
    async fn test_get_sleep_duration_open() {
        let logger = Arc::new(LoggerLocal::new("test".into(), None));
        let mut data = mock_market_data();
        data.mrkt_status = "Open".into();

        let sleep_dur = data.get_sleep_duration(logger).await;
        assert_eq!(sleep_dur, std::time::Duration::from_secs(0));
    }

    #[tokio::test]
    async fn test_get_sleep_duration_future() {
        let logger = Arc::new(LoggerLocal::new("test".into(), None));
        let mut data = mock_market_data();
        // // Set pm_open_raw to a far future date to ensure it's > now_ny
        data.pm_open_raw =
            NaiveDateTime::parse_from_str("2099-01-01 04:00:00", "%Y-%m-%d %H:%M:%S").unwrap();

        let sleep_dur = data.get_sleep_duration(logger).await;
        assert!(sleep_dur.as_secs() > 0);
    }
}
