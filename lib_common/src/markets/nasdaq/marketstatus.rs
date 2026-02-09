use crate::markets::nasdaq::apicall::ApiCall;
use crate::loggers::loggerlocal::LoggerLocal;
use chrono::{NaiveDateTime, NaiveDate, Utc, Duration};
use chrono_tz::US::Eastern;
use serde::{Deserialize, Serialize};
use serde_json::{from_value, Value};
use std::sync::Arc;

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct MarketStatusData {
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
    
    #[serde(rename = "pmOpenRaw")]
    pub pm_open_raw: NaiveDateTime,
    #[serde(rename = "ahCloseRaw")]
    pub ah_close_raw: NaiveDateTime,
    #[serde(rename = "openRaw")]
    pub open_raw: NaiveDateTime,
    #[serde(rename = "closeRaw")]
    pub close_raw: NaiveDateTime,
}

pub struct MarketStatus {
    api_call: Arc<ApiCall>,
    logger: Arc<LoggerLocal>,
}

impl MarketStatus {
    pub fn new(api_call: Arc<ApiCall>, logger: Arc<LoggerLocal>) -> Self {
        Self { api_call, logger }
    }

    pub async fn get_status(&self) -> Result<MarketStatusData, Box<dyn std::error::Error + Send + Sync>> {
        let path = "api/market-info";

        let raw_json: Value = self.api_call.fetch_nasdaq(path).await
            .map_err(|e| format!("Nasdaq API Fetch Error: {}", e))?;

        let data_part = raw_json.get("data").unwrap_or(&raw_json);

        match from_value::<MarketStatusData>(data_part.clone()) {
            Ok(data) => {
                self.logger.debug("Market status schema validated successfully", None).await;
                Ok(data)
            }
            Err(e) => {
                let error_message = format!("STRICT SCHEMA VALIDATION FAILED: {}", e);
                self.logger.fatal(&error_message, Some(serde_json::json!({"payload": raw_json}))).await;
                Err(error_message.into())
            }
        }
    }
}

impl MarketStatusData {
    /// Gets current time specifically in New York timezone
    fn now_ny(&self) -> NaiveDateTime {
        Utc::now().with_timezone(&Eastern).naive_local()
    }

    pub fn get_sleep_duration(&self) -> std::time::Duration {
        let now = self.now_ny();
        
        if self.mrkt_status == "Open" {
            return std::time::Duration::from_secs(0);
        }

        // Determine target from raw timestamps
        let mut target = if now < self.pm_open_raw { 
            self.pm_open_raw 
        } else { 
            self.open_raw 
        };

        // If timestamps are in the past (Weekend/Holiday), use next_trade_date
        if target <= now {
            let fmt = "%b %d, %Y";
            if let Ok(next_date) = NaiveDate::parse_from_str(&self.next_trade_date, fmt) {
                // Point to 04:00 AM NY on the next trading day
                if let Some(anchored) = next_date.and_hms_opt(4, 0, 0) {
                    target = anchored;
                }
            }
        }

        if target > now {
            let diff = target - now;
            println!("Target NY Open: {} ({} remaining)", 
                target.format("%Y-%m-%d %H:%M:%S"), 
                Self::format_duration(diff)
            );
            diff.to_std().unwrap_or(std::time::Duration::from_secs(60))
        } else {
            std::time::Duration::from_secs(300)
        }
    }

    pub fn format_duration(dur: Duration) -> String {
        let total_secs = dur.num_seconds();
        let hours = total_secs / 3600;
        let mins = (total_secs % 3600) / 60;
        let secs = total_secs % 60;
        format!("{:02}:{:02}:{:02}", hours, mins, secs)
    }
}