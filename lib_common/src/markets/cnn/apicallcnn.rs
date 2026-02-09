use crate::retrieve::ky_http::ApiClient;
use crate::loggers::loggerlocal::LoggerLocal;
use reqwest::Method;
use serde_json::Value;
use std::sync::Arc;
use tokio::time::{sleep, Duration};

pub struct ApiCallCnn {
    client: ApiClient,
    logger: Arc<LoggerLocal>,
}

impl ApiCallCnn {
    /// Initialize with a shared logger instance
    pub fn new(logger: Arc<LoggerLocal>) -> Self {
        Self {
            // Base URL for CNN API
            client: ApiClient::new("https://production.dataviz.cnn.io/", None),
            logger,
        }
    }

    /// Fetches data from CNN API with retry logic.
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
            // No specific headers mentioned for CNN, so send without custom headers.
            // ApiClient already adds default headers and handles auth token if provided.
            let response = self.client.request::<Value, ()>(
                Method::GET,
                &path,
                None,
                None, // No custom headers for CNN for now
            ).await?;

            if response.success {
                if let Some(body) = response.data {
                    return Ok(body);
                } else {
                    let error_msg = format!("CNN API returned empty data (Attempt {}/{})", attempts, max_attempts);
                    self.logger.warn(&error_msg, Some(serde_json::json!({"path": path, "attempt": attempts}))).await;
                }
            } else {
                // HTTP Level Error (e.g., 403, 404, 500)
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
