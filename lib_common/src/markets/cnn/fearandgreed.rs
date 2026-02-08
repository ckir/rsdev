use crate::markets::cnn::apicallcnn::ApiCallCnn;
use crate::loggers::loggerlocal::LoggerLocal;
use chrono::{DateTime, Utc, TimeZone};
use serde::{Deserialize, Serialize, Deserializer};
use serde_json::from_value;
use std::sync::Arc;

/// Main Fear and Greed Response Structure
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct FearAndGreedData {
    pub fear_and_greed: CurrentStats,
    pub fear_and_greed_historical: HistoricalSection,
    pub market_momentum_sp500: IndicatorSection,
    pub market_momentum_sp125: IndicatorSection,
    pub stock_price_strength: IndicatorSection,
    pub stock_price_breadth: IndicatorSection,
    pub put_call_options: IndicatorSection,
    pub market_volatility_vix: IndicatorSection,
    pub junk_bond_demand: IndicatorSection,
    pub safe_haven_demand: IndicatorSection,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CurrentStats {
    pub score: f64,
    pub rating: String,
    pub timestamp: DateTime<Utc>,
    pub previous_close: f64,
    pub previous_1_week: f64,
    pub previous_1_month: f64,
    pub previous_1_year: f64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct HistoricalSection {
    #[serde(deserialize_with = "deserialize_ms_to_utc")]
    pub timestamp: DateTime<Utc>,
    pub score: f64,
    pub rating: String,
    pub data: Vec<DataPoint>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct IndicatorSection {
    #[serde(deserialize_with = "deserialize_ms_to_utc")]
    pub timestamp: DateTime<Utc>,
    pub score: f64,
    pub rating: String,
    pub data: Vec<DataPoint>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DataPoint {
    #[serde(rename = "x", deserialize_with = "deserialize_ms_to_utc")]
    pub timestamp: DateTime<Utc>,
    #[serde(rename = "y")]
    pub value: f64,
    pub rating: String,
}

/// Custom deserializer to handle CNN's float-based millisecond timestamps
fn deserialize_ms_to_utc<'de, D>(deserializer: D) -> Result<DateTime<Utc>, D::Error>
where
    D: Deserializer<'de>,
{
    let ms = f64::deserialize(deserializer)?;
    Ok(Utc.timestamp_millis_opt(ms as i64).unwrap())
}

pub struct FearAndGreed {
    api_call: Arc<ApiCallCnn>,
    logger: Arc<LoggerLocal>,
}

impl FearAndGreed {
    pub fn new(api_call: Arc<ApiCallCnn>, logger: Arc<LoggerLocal>) -> Self {
        Self { api_call, logger }
    }

    /// Fetches and normalizes the Fear and Greed data
    pub async fn get_full_report(&self, date: Option<String>) -> Result<FearAndGreedData, Box<dyn std::error::Error>> {
        let raw_json = self.api_call.fetch_cnn(date).await?;

        match from_value::<FearAndGreedData>(raw_json.clone()) {
            Ok(normalized) => {
                self.logger.debug("Fear and Greed data normalized successfully", None).await;
                Ok(normalized)
            }
            Err(e) => {
                let err_msg = format!("Normalization failed for Fear and Greed: {}", e);
                self.logger.fatal(&err_msg, Some(serde_json::json!({"raw": raw_json}))).await;
                Err(err_msg.into())
            }
        }
    }
}

impl FearAndGreedData {
    /// Helper to get the very latest data point from the historical trend
    pub fn get_latest_historical_score(&self) -> Option<&DataPoint> {
        self.fear_and_greed_historical.data.last()
    }
}