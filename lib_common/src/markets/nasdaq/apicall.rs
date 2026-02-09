use crate::retrieve::ky_http::ApiClient;
use crate::loggers::loggerlocal::LoggerLocal;
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use reqwest::Method;
use serde_json::Value;
use std::sync::Arc;
use tokio::time::{sleep, Duration};

pub struct ApiCall {
    client: ApiClient,
    logger: Arc<LoggerLocal>,
}

impl ApiCall {
    /// Initialize with a shared logger instance
    pub fn new(logger: Arc<LoggerLocal>) -> Self {
        Self {
            // Base URL for Nasdaq API
            client: ApiClient::new("https://api.nasdaq.com/", None),
            logger,
        }
    }

    /// Performs a GET request with Nasdaq headers and 3 retries for rCode != 200
    pub async fn fetch_nasdaq(&self, path: &str) -> Result<Value, Box<dyn std::error::Error>> {
        let mut attempts = 0;
        let max_attempts = 3;

        loop {
            attempts += 1;
            let headers = self.get_nasdaq_headers();

            // 1. Execute Network Request
            // We use Value as the generic so we can inspect the raw JSON structure
            let response = self.client.request::<Value, ()>(
                Method::GET,
                path,
                None,
                Some(headers),
            ).await?;

            // 2. Process Response
            if response.success {
                if let Some(body) = response.data {
                    // Extract Nasdaq-specific status code and data object
                    let r_code = body["status"]["rCode"].as_i64().unwrap_or(-1);
                    let data = &body["data"];

                    // SUCCESS CONDITION: rCode is 200 AND data is not null
                    if r_code == 200 && !data.is_null() {
                        return Ok(data.clone());
                    }

                    // FAILURE CONDITION: Log and prepare for retry
                    let log_extras = serde_json::json!({
                        "path": path,
                        "attempt": attempts,
                        "rCode": r_code,
                        "data_is_null": data.is_null(),
                        "server_message": body["message"]
                    });

                    let error_msg = if data.is_null() && r_code == 200 {
                        format!("Nasdaq API returned rCode 200 but NULL data (Attempt {}/{})", attempts, max_attempts)
                    } else {
                        format!("Nasdaq API Business Error: rCode {} (Attempt {}/{})", r_code, attempts, max_attempts)
                    };

                    self.logger.warn(&error_msg, Some(log_extras)).await;
                }
            } else {
                // HTTP Level Error (e.g., 403, 404, 500)
                let http_error = format!("HTTP Request failed for {}: Status {}", path, response.status);
                self.logger.error(&http_error, None).await;
            }

            // 3. Retry Logic
            if attempts >= max_attempts {
                let fatal_msg = format!("Final failure: Nasdaq API unreachable or invalid after {} attempts", max_attempts);
                self.logger.fatal(&fatal_msg, None).await;
                return Err(fatal_msg.into());
            }

            // Delay before next attempt (Linear backoff: 1s, 2s)
            sleep(Duration::from_secs(attempts)).await;
        }
    }

    /// Internal helper to construct the browser-mimic headers
    fn get_nasdaq_headers(&self) -> HeaderMap {
        let mut headers = HeaderMap::new();
        
        let header_list = [
            ("accept", "application/json, text/plain, */*"),
            ("accept-language", "en-US,en;q=0.9,el-GR;q=0.8,el;q=0.7"),
            ("cache-control", "no-cache"),
            ("dnt", "1"),
            ("origin", "https://www.nasdaq.com"),
            ("pragma", "no-cache"),
            ("priority", "u=1, i"),
            ("referer", "https://www.nasdaq.com/"),
            ("sec-ch-ua", "\"Google Chrome\";v=\"135\", \"Not-A.Brand\";v=\"8\", \"Chromium\";v=\"135\""),
            ("sec-ch-ua-mobile", "?0"),
            ("sec-ch-ua-platform", "\"Windows\""),
            ("sec-fetch-dest", "empty"),
            ("sec-fetch-mode", "cors"),
            ("sec-fetch-site", "same-site"),
            ("user-agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/135.0.0.0 Safari/537.36"),
        ];

        for (name, value) in header_list {
            if let (Ok(h_name), Ok(h_value)) = (HeaderName::from_bytes(name.as_bytes()), HeaderValue::from_str(value)) {
                headers.insert(h_name, h_value);
            }
        }

        headers
    }
}