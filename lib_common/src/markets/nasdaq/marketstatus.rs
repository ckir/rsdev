use crate::markets::nasdaq::apicall::ApiCall;
use crate::loggers::loggerlocal::LoggerLocal;
use chrono::{NaiveDateTime, Local, Duration};
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

    pub async fn get_status(&self) -> Result<MarketStatusData, Box<dyn std::error::Error>> {
        let path = "api/market-info";

        let raw_json: Value = match self.api_call.fetch_nasdaq(path).await {
            Ok(val) => val,
            Err(e) => return Err(e),
        };

        match from_value::<MarketStatusData>(raw_json.clone()) {
            Ok(data) => {
                self.logger.debug("Market status schema validated successfully", None).await;
                Ok(data)
            }
            Err(e) => {
                let error_message = format!("STRICT SCHEMA VALIDATION FAILED: {}", e);
                self.logger.fatal(
                    &error_message,
                    Some(serde_json::json!({
                        "endpoint": path,
                        "error_details": e.to_string(),
                        "received_payload": raw_json
                    })),
                ).await;
                Err(error_message.into())
            }
        }
    }
}

/// Represents the possible upcoming market milestones
#[derive(Debug)]
pub enum MarketEvent {
    PreMarketOpen(Duration),
    RegularMarketOpen(Duration),
    RegularMarketClose(Duration),
    AfterHoursClose(Duration),
    MarketOffline,
}

impl MarketStatusData {
    /// Calculates the next market event based on the current system time.
    /// Returns the type of event and the duration remaining.
    pub fn get_next_event(&self) -> MarketEvent {
        let now = Local::now().naive_local();

        if now < self.pm_open_raw {
            MarketEvent::PreMarketOpen(self.pm_open_raw - now)
        } else if now < self.open_raw {
            MarketEvent::RegularMarketOpen(self.open_raw - now)
        } else if now < self.close_raw {
            MarketEvent::RegularMarketClose(self.close_raw - now)
        } else if now < self.ah_close_raw {
            MarketEvent::AfterHoursClose(self.ah_close_raw - now)
        } else {
            MarketEvent::MarketOffline
        }
    }

    /// Formats the duration into a human-readable string: "HH:MM:SS"
    pub fn format_duration(dur: Duration) -> String {
        let total_secs = dur.num_seconds();
        let hours = total_secs / 3600;
        let mins = (total_secs % 3600) / 60;
        let secs = total_secs % 60;
        format!("{:02}:{:02}:{:02}", hours, mins, secs)
    }
}