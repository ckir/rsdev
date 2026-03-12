//! # CNN API Call Module
//!
//! This module provides the networking logic to interact with CNN's Fear and Greed index API.
//! It includes automated retry mechanisms and standardized error logging using `LoggerLocal`.

use crate::retrieve::ky_http::ApiClient;
use crate::loggers::loggerlocal::LoggerLocal;
use reqwest::Method;
use serde_json::Value;
use std::sync::Arc;
use tokio::time::{sleep, Duration};

/// Handles HTTP requests to the CNN DataViz API.
pub struct ApiCallCnn {
    /// The underlying HTTP client for CNN requests.
    pub client: ApiClient,
    /// Shared logger for tracing and error reporting.
    pub logger: Arc<LoggerLocal>,
}

impl ApiCallCnn {
    /// Initializes a new `ApiCallCnn` instance with a shared logger.
    ///
    /// # Arguments
    /// * `logger` - An `Arc` pointer to a `LoggerLocal` instance.
    pub fn new(logger: Arc<LoggerLocal>) -> Self {
        Self {
            // // Base URL for CNN API
            client: ApiClient::new("https://production.dataviz.cnn.io/", None),
            logger,
        }
    }

    /// Fetches data from CNN API with retry logic.
    ///
    /// # Arguments
    /// * `date` - Optional date string in YYYY-MM-DD format to fetch historical data.
    ///
    /// # Errors
    /// Returns an error if the request fails after 3 attempts or if data normalization fails.
    pub async fn fetch_cnn(&self, date: Option<String>) -> Result<Value, Box<dyn std::error::Error>> {
        let mut attempts = 0;
        let max_attempts = 3;

        let mut path = String::from("index/fearandgreed");
        if let Some(d) = date {
            path.push('/');
            path.push_str(&d);
        }

        loop {
            attempts += 1;
            // // No specific headers mentioned for CNN, so send without custom headers.
            // // ApiClient already adds default headers and handles auth token if provided.
            let response = self.client.request::<Value, ()>(
                Method::GET,
                &path,
                None,
                None, // // No custom headers for CNN for now
            ).await?;

            if response.success {
                if let Some(body) = response.data {
                    return Ok(body);
                } else {
                    let error_msg = format!("CNN API returned empty data (Attempt {}/{})", attempts, max_attempts);
                    self.logger.warn(&error_msg, Some(serde_json::json!({"path": path, "attempt": attempts}))).await;
                }
            } else {
                // // HTTP Level Error (e.g., 403, 404, 500)
                let http_error = format!("HTTP Request failed for {}: Status {}", path, response.status);
                self.logger.error(&http_error, None).await;
            }

            if attempts >= max_attempts {
                let fatal_msg = format!("Final failure: CNN API unreachable or invalid after {} attempts", max_attempts);
                self.logger.fatal(&fatal_msg, None).await;
                return Err(fatal_msg.into());
            }

            sleep(Duration::from_secs(attempts)).await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::loggers::loggerlocal::LoggerLocal;
    use std::sync::Arc;

    #[tokio::test]
    async fn test_api_call_cnn_new() {
        let logger = Arc::new(LoggerLocal::new("test".into(), None));
        let api_call = ApiCallCnn::new(logger);
        assert_eq!(api_call.client.base_url.as_str(), "https://production.dataviz.cnn.io/");
        assert_eq!(api_call.logger.app_name, "test");
    }

    #[tokio::test]
    async fn test_fetch_cnn_failure() {
        let logger = Arc::new(LoggerLocal::new("test".into(), None));
        let api_call = ApiCallCnn::new(logger);
        
        // // Test with an invalid path/date to trigger retry logic
        let result = api_call.fetch_cnn(Some("invalid-date".into())).await;
        assert!(result.is_err());
    }
}
