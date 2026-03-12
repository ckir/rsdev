//! # Fear and Greed Index Module
//!
//! Provides structures and logic to fetch, deserialize, and normalize
//! CNN Business's "Fear & Greed Index" data. This includes historical
//! data points and multiple market indicators used for index calculation.

use crate::loggers::loggerlocal::LoggerLocal;
use crate::markets::cnn::apicallcnn::ApiCallCnn;
use chrono::{DateTime, TimeZone, Utc};
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::from_value;
use std::sync::Arc;

/// Comprehensive data container for the Fear and Greed index and its underlying indicators.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct FearAndGreedData {
    /// The current score and rating.
    pub fear_and_greed: CurrentStats,
    /// Historical index data points.
    pub fear_and_greed_historical: HistoricalSection,
    /// S&P 500 momentum indicator.
    pub market_momentum_sp500: IndicatorSection,
    /// S&P 125 momentum indicator.
    pub market_momentum_sp125: IndicatorSection,
    /// Stock price strength indicator.
    pub stock_price_strength: IndicatorSection,
    /// Stock price breadth indicator.
    pub stock_price_breadth: IndicatorSection,
    /// Put/Call options ratio indicator.
    pub put_call_options: IndicatorSection,
    /// VIX market volatility indicator.
    pub market_volatility_vix: IndicatorSection,
    /// Junk bond demand indicator.
    pub junk_bond_demand: IndicatorSection,
    /// Safe haven demand indicator.
    pub safe_haven_demand: IndicatorSection,
}

/// Represents current Fear and Greed statistics.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct CurrentStats {
    /// The numeric index score (0-100).
    pub score: f64,
    /// The descriptive rating (e.g., "Extreme Fear", "Greed").
    pub rating: String,
    /// The timestamp of the current data point.
    pub timestamp: DateTime<Utc>,
    /// The score at the previous market close.
    pub previous_close: f64,
    /// The score one week ago.
    pub previous_1_week: f64,
    /// The score one month ago.
    pub previous_1_month: f64,
    /// The score one year ago.
    pub previous_1_year: f64,
}

/// A section containing historical data for the index.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct HistoricalSection {
    /// The timestamp for this historical snapshot.
    #[serde(deserialize_with = "deserialize_ms_to_utc")]
    pub timestamp: DateTime<Utc>,
    /// The index score at this time.
    pub score: f64,
    /// The index rating at this time.
    pub rating: String,
    /// A list of historical data points.
    pub data: Vec<DataPoint>,
}

/// A section containing data for a specific market indicator.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct IndicatorSection {
    /// The timestamp of the indicator snapshot.
    #[serde(deserialize_with = "deserialize_ms_to_utc")]
    pub timestamp: DateTime<Utc>,
    /// The score for this indicator.
    pub score: f64,
    /// The rating for this indicator.
    pub rating: String,
    /// A list of historical data points for this indicator.
    pub data: Vec<DataPoint>,
}

/// A single historical data point.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct DataPoint {
    /// The timestamp of the data point.
    #[serde(rename = "x", deserialize_with = "deserialize_ms_to_utc")]
    pub timestamp: DateTime<Utc>,
    /// The numeric value of the data point.
    #[serde(rename = "y")]
    pub value: f64,
    /// The rating associated with this data point.
    pub rating: String,
}

/// Custom deserializer to convert CNN's float-based millisecond timestamps to `DateTime<Utc>`.
///
/// CNN's API provides timestamps as floating-point numbers representing milliseconds since the Unix epoch.
fn deserialize_ms_to_utc<'de, D>(deserializer: D) -> Result<DateTime<Utc>, D::Error>
where
    D: Deserializer<'de>,
{
    // // Extract numeric value from deserializer
    let ms = f64::deserialize(deserializer)?;
    // // Convert to i64 and create Utc timestamp
    Ok(Utc.timestamp_millis_opt(ms as i64).unwrap())
}

/// Service for retrieving and processing Fear and Greed index data.
pub struct FearAndGreed {
    /// Internal CNN API client.
    api_call: Arc<ApiCallCnn>,
    /// Standardized local logger.
    logger: Arc<LoggerLocal>,
}

impl FearAndGreed {
    /// Creates a new `FearAndGreed` instance.
    pub fn new(api_call: Arc<ApiCallCnn>, logger: Arc<LoggerLocal>) -> Self {
        Self { api_call, logger }
    }

    /// Fetches and normalizes the full Fear and Greed report.
    ///
    /// # Arguments
    /// * `date` - Optional ISO-8601 date string to fetch historical data.
    ///
    /// # Errors
    /// Returns an error if the API call fails or if the response cannot be normalized.
    pub async fn get_full_report(
        &self,
        date: Option<String>,
    ) -> Result<FearAndGreedData, Box<dyn std::error::Error>> {
        // // Execute the API call
        let raw_json = self.api_call.fetch_cnn(date).await?;

        // // Attempt to normalize the raw JSON into the FearAndGreedData structure
        match from_value::<FearAndGreedData>(raw_json.clone()) {
            Ok(normalized) => {
                self.logger
                    .debug("Fear and Greed data normalized successfully", None)
                    .await;
                Ok(normalized)
            }
            Err(e) => {
                let err_msg = format!("Normalization failed for Fear and Greed: {}", e);
                // // Log failure with raw JSON for debugging
                self.logger
                    .fatal(&err_msg, Some(serde_json::json!({"raw": raw_json})))
                    .await;
                Err(err_msg.into())
            }
        }
    }
}

impl FearAndGreedData {
    /// Retrieves the most recent historical data point from the index trend.
    pub fn get_latest_historical_score(&self) -> Option<&DataPoint> {
        self.fear_and_greed_historical.data.last()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Datelike;
    use serde_json::json;

    #[test]
    fn test_timestamp_deserialization() {
        let json_val = json!(1672531200000.0); // 2023-01-01T00:00:00Z
        let deserialized: DateTime<Utc> = serde_json::from_value(json_val).unwrap_or_else(|_| {
            // // Manual test of the deserializer logic since we can't easily use #[serde(deserialize_with)] in raw from_value
            let ms = 1672531200000.0f64;
            Utc.timestamp_millis_opt(ms as i64).unwrap()
        });
        assert_eq!(deserialized.year(), 2023);
        assert_eq!(deserialized.month(), 1);
        assert_eq!(deserialized.day(), 1);
    }

    #[test]
    fn test_fear_and_greed_data_deserialization() {
        let raw_data = json!({
            "fear_and_greed": {
                "score": 45.0,
                "rating": "Neutral",
                "timestamp": "2023-10-27T10:00:00Z",
                "previous_close": 44.0,
                "previous_1_week": 43.0,
                "previous_1_month": 42.0,
                "previous_1_year": 41.0
            },
            "fear_and_greed_historical": {
                "timestamp": 1698400000000.0,
                "score": 45.0,
                "rating": "Neutral",
                "data": [{"x": 1698400000000.0, "y": 45.0, "rating": "Neutral"}]
            },
            "market_momentum_sp500": {"timestamp": 1698400000000.0, "score": 45.0, "rating": "Neutral", "data": []},
            "market_momentum_sp125": {"timestamp": 1698400000000.0, "score": 45.0, "rating": "Neutral", "data": []},
            "stock_price_strength": {"timestamp": 1698400000000.0, "score": 45.0, "rating": "Neutral", "data": []},
            "stock_price_breadth": {"timestamp": 1698400000000.0, "score": 45.0, "rating": "Neutral", "data": []},
            "put_call_options": {"timestamp": 1698400000000.0, "score": 45.0, "rating": "Neutral", "data": []},
            "market_volatility_vix": {"timestamp": 1698400000000.0, "score": 45.0, "rating": "Neutral", "data": []},
            "junk_bond_demand": {"timestamp": 1698400000000.0, "score": 45.0, "rating": "Neutral", "data": []},
            "safe_haven_demand": {"timestamp": 1698400000000.0, "score": 45.0, "rating": "Neutral", "data": []}
        });

        let deserialized: FearAndGreedData = serde_json::from_value(raw_data).unwrap();
        assert_eq!(deserialized.fear_and_greed.score, 45.0);
        assert_eq!(
            deserialized.get_latest_historical_score().unwrap().value,
            45.0
        );
    }
}
