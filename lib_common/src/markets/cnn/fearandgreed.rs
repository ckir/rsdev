//! # CNN Fear & Greed Index Data Model and Client
//!
//! This module defines the data structures and client logic for fetching and
//! normalizing the CNN Business Fear & Greed Index. It's designed to provide a
//! strongly-typed representation of the JSON response from the CNN API.
//!
//! ## Key Features:
//! - **Strict Data Modeling**: Uses `serde` to meticulously map the incoming
//!   JSON payload into Rust structs, ensuring data integrity and compile-time
//!   type checking.
//! - **Timestamp Normalization**: Includes a custom `deserialize_ms_to_utc`
//!   function to correctly convert Unix millisecond timestamps (common in web APIs)
//!   into `chrono::DateTime<Utc>`.
//! - **Centralized Fetching**: Utilizes the `ApiCallCnn` client to handle the
//!   underlying HTTP requests and retry logic, keeping the concerns separated.
//! - **Data Normalization**: The `get_full_report` method fetches the raw data
//!   and then attempts to deserialize it into the `FearAndGreedData` struct,
//!   logging any validation failures.
//! - **Convenience Methods**: Provides helper methods, like `get_latest_historical_score`,
//!   to easily access commonly used data points.

#![doc(html_logo_url = "https://example.com/logo.png")] // Placeholder
#![forbid(unsafe_code)]
#![warn(missing_docs, rust_2018_idioms, unused_qualifications)]

use crate::markets::cnn::apicallcnn::ApiCallCnn;
use crate::loggers::loggerlocal::LoggerLocal;
use chrono::{DateTime, Utc, TimeZone};
use serde::{Deserialize, Serialize, Deserializer};
use serde_json::from_value;
use std::sync::Arc;

/// # Fear and Greed Data
///
/// The top-level structure for the CNN Fear & Greed Index report.
///
/// This struct aggregates various sections of the report, including current sentiment,
/// historical data, and individual indicators that contribute to the overall score.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct FearAndGreedData {
    /// The current Fear & Greed score and its associated rating.
    pub fear_and_greed: CurrentStats,
    /// Historical data points for the Fear & Greed Index.
    pub fear_and_greed_historical: HistoricalSection,
    /// Detailed breakdown of the S&P 500 market momentum indicator.
    pub market_momentum_sp500: IndicatorSection,
    /// Detailed breakdown of the S&P 125 market momentum indicator.
    pub market_momentum_sp125: IndicatorSection,
    /// Detailed breakdown of the stock price strength indicator.
    pub stock_price_strength: IndicatorSection,
    /// Detailed breakdown of the stock price breadth indicator.
    pub stock_price_breadth: IndicatorSection,
    /// Detailed breakdown of the put/call options indicator.
    pub put_call_options: IndicatorSection,
    /// Detailed breakdown of the market volatility (VIX) indicator.
    pub market_volatility_vix: IndicatorSection,
    /// Detailed breakdown of the junk bond demand indicator.
    pub junk_bond_demand: IndicatorSection,
    /// Detailed breakdown of the safe haven demand indicator (e.g., gold vs. stocks).
    pub safe_haven_demand: IndicatorSection,
}

/// # Current Statistics
///
/// Represents the immediate, most recent values for an index or indicator.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CurrentStats {
    /// The numerical score of the index/indicator.
    pub score: f64,
    /// The qualitative rating (e.g., "Extreme Greed", "Fear").
    pub rating: String,
    /// The UTC timestamp of these statistics.
    pub timestamp: DateTime<Utc>,
    /// The score from the previous trading close.
    pub previous_close: f64,
    /// The score from one week prior.
    pub previous_1_week: f64,
    /// The score from one month prior.
    pub previous_1_month: f64,
    /// The score from one year prior.
    pub previous_1_year: f64,
}

/// # Historical Section
///
/// Represents a section containing historical data points for an index or indicator.
/// This typically includes a current score, rating, and a time series of past values.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct HistoricalSection {
    /// The UTC timestamp of the most recent data point in this section.
    #[serde(deserialize_with = "deserialize_ms_to_utc")]
    pub timestamp: DateTime<Utc>,
    /// The numerical score of the most recent historical data point.
    pub score: f64,
    /// The qualitative rating for the most recent historical data point.
    pub rating: String,
    /// A vector of individual `DataPoint`s representing the historical trend.
    pub data: Vec<DataPoint>,
}

/// # Indicator Section
///
/// Similar to `HistoricalSection`, but specifically used for the individual
/// indicators that comprise the overall Fear & Greed Index.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct IndicatorSection {
    /// The UTC timestamp of the most recent data point for this indicator.
    #[serde(deserialize_with = "deserialize_ms_to_utc")]
    pub timestamp: DateTime<Utc>,
    /// The numerical score of the most recent data point for this indicator.
    pub score: f64,
    /// The qualitative rating for the most recent data point for this indicator.
    pub rating: String,
    /// A vector of individual `DataPoint`s representing the historical trend of this indicator.
    pub data: Vec<DataPoint>,
}

/// # Data Point
///
/// Represents a single historical entry in a time series.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DataPoint {
    /// The UTC timestamp of this data point. Mapped from JSON field `x`.
    #[serde(rename = "x", deserialize_with = "deserialize_ms_to_utc")]
    pub timestamp: DateTime<Utc>,
    /// The numerical value at this data point. Mapped from JSON field `y`.
    #[serde(rename = "y")]
    pub value: f64,
    /// The qualitative rating associated with this data point.
    pub rating: String,
}

/// # Deserialize Milliseconds to UTC DateTime
///
/// A custom `serde` deserializer function for converting Unix millisecond
/// timestamps (represented as `f64` in the JSON) into `chrono::DateTime<Utc>`.
///
/// This is necessary because many web APIs return timestamps in this format,
/// and `chrono` expects integers for Unix timestamps. The `f64` is cast to `i64`.
fn deserialize_ms_to_utc<'de, D>(deserializer: D) -> Result<DateTime<Utc>, D::Error>
where
    D: Deserializer<'de>,
{
    let ms = f64::deserialize(deserializer)?;
    Utc.timestamp_millis_opt(ms as i64)
        .single() // Attempt to get a single valid DateTime
        .ok_or_else(|| serde::de::Error::custom("invalid timestamp"))
}

/// # Fear and Greed Client
///
/// A client for fetching and normalizing the CNN Fear & Greed Index report.
pub struct FearAndGreed {
    /// An `Arc`'d `ApiCallCnn` instance for making HTTP requests to the CNN API.
    api_call: Arc<ApiCallCnn>,
    /// A shared logger for recording API call outcomes and normalization results.
    logger: Arc<LoggerLocal>,
}

impl FearAndGreed {
    /// Creates a new `FearAndGreed` client instance.
    ///
    /// # Arguments
    /// * `api_call` - A shared `ApiCallCnn` instance for network requests.
    /// * `logger` - A shared `LoggerLocal` for structured logging.
    pub fn new(api_call: Arc<ApiCallCnn>, logger: Arc<LoggerLocal>) -> Self {
        Self { api_call, logger }
    }

    /// # Get Full Report
    ///
    /// Fetches the raw CNN Fear & Greed Index data and attempts to normalize it
    /// into the `FearAndGreedData` struct.
    ///
    /// ## Logic:
    /// 1.  Calls `api_call.fetch_cnn` to get the raw JSON payload, optionally
    ///     specifying a `date`.
    /// 2.  Attempts to deserialize the raw JSON into `FearAndGreedData`.
    /// 3.  **On Success**: Logs a debug message and returns the normalized data.
    /// 4.  **On Error**: If deserialization fails (e.g., due to API schema changes),
    ///     it logs a fatal error with the raw payload for debugging and returns
    ///     a boxed error.
    ///
    /// ## Arguments
    /// * `date` - An `Option<String>` for fetching historical data. If `None`, fetches the latest.
    ///
    /// # Returns
    /// A `Result` containing either the `FearAndGreedData` or a boxed error.
    pub async fn get_full_report(&self, date: Option<String>) -> Result<FearAndGreedData, Box<dyn std::error::Error>> {
        let raw_json = self.api_call.fetch_cnn(date).await?;

        // Attempt to deserialize the raw JSON into our strict struct.
        match from_value::<FearAndGreedData>(raw_json.clone()) {
            Ok(normalized) => {
                self.logger.debug("Fear and Greed data normalized successfully", None).await;
                Ok(normalized)
            }
            Err(e) => {
                // If deserialization fails, it's a critical error indicating a schema mismatch.
                let err_msg = format!("Normalization failed for Fear and Greed: {}", e);
                self.logger.fatal(&err_msg, Some(serde_json::json!({"raw": raw_json}))).await;
                Err(err_msg.into())
            }
        }
    }
}

impl FearAndGreedData {
    /// # Get Latest Historical Score
    ///
    /// A convenience method to extract the very last `DataPoint` from the
    /// `fear_and_greed_historical` section. This typically represents the
    /// most recent historical data available.
    ///
    /// # Returns
    /// An `Option<&DataPoint>`, which is `Some` if there is historical data,
    /// and `None` otherwise.
    pub fn get_latest_historical_score(&self) -> Option<&DataPoint> {
        self.fear_and_greed_historical.data.last()
    }
}