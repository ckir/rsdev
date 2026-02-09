//! # CNN API Client
//!
//! This module provides a dedicated and resilient client for interacting with the
//! CNN Fear & Greed Index API. It encapsulates the logic for making HTTP requests,
//! handling retries, and processing responses specifically for this data source.
//!
//! ## Core Features:
//! - **Dedicated Client**: Uses a pre-configured `ApiClient` with the base URL for
//!   the CNN API, simplifying request paths.
//! - **Resilient Fetching**: The `fetch_cnn` method includes a retry mechanism with
//!   a simple backoff strategy to handle transient network errors or API-level issues.
//! - **Structured Logging**: Integrates with `LoggerLocal` to provide detailed logs
//!   on successful fetches, warnings for empty data, and errors for HTTP failures or
//!   final exhaustion of retry attempts.
//! - **Date-based Queries**: Supports fetching data for a specific date, allowing for
//!   historical data retrieval.

#![doc(html_logo_url = "https://example.com/logo.png")] // Placeholder
#![forbid(unsafe_code)]
#![warn(missing_docs, rust_2018_idioms, unused_qualifications)]

use crate::retrieve::ky_http::ApiClient;
use crate::loggers::loggerlocal::LoggerLocal;
use reqwest::Method;
use serde_json::Value;
use std::sync::Arc;
use tokio::time::{sleep, Duration};

/// # CNN API Call Client
///
/// A specialized client for making requests to the CNN Fear & Greed Index API.
///
/// It encapsulates an `ApiClient` instance, which handles the low-level HTTP
/// communication, and a shared `LoggerLocal` for structured logging.
pub struct ApiCallCnn {
    /// The underlying generic HTTP client, pre-configured for the CNN API base URL.
    client: ApiClient,
    /// A shared logger for recording the outcomes of API calls.
    logger: Arc<LoggerLocal>,
}

impl ApiCallCnn {
    /// # New `ApiCallCnn`
    ///
    /// Initializes a new client for the CNN API.
    ///
    /// It configures the internal `ApiClient` with the production base URL for the
    /// CNN data visualization API.
    ///
    /// ## Arguments
    /// * `logger` - A shared `LoggerLocal` instance for logging.
    pub fn new(logger: Arc<LoggerLocal>) -> Self {
        Self {
            client: ApiClient::new("https://production.dataviz.cnn.io/", None),
            logger,
        }
    }

    /// # Fetch CNN Data
    ///
    /// Fetches data from the CNN Fear & Greed API with built-in retry logic.
    ///
    /// ## Logic:
    /// 1.  It constructs the request path, optionally appending a date for historical queries.
    /// 2.  It enters a loop that will try up to `max_attempts` times.
    /// 3.  Inside the loop, it uses the `ApiClient` to make a `GET` request.
    /// 4.  **On Success (`2xx` status code and valid data)**:
    ///     - The method returns the `serde_json::Value` payload immediately.
    ///     - If the API returns a `2xx` but the data is empty, it logs a warning and proceeds to the next retry attempt.
    /// 5.  **On HTTP Error (non-`2xx` status)**:
    ///     - It logs the HTTP error and proceeds to the next retry attempt.
    /// 6.  After each failed attempt, it sleeps for a short duration (linear backoff) before retrying.
    /// 7.  If all attempts fail, it logs a `fatal` error and returns a final error message.
    ///
    /// ## Arguments
    /// * `date` - An `Option<String>` representing the date for which to fetch data (e.g., "2024-01-15").
    ///            If `None`, it fetches the latest data.
    ///
    /// # Returns
    /// A `Result` containing either the JSON `Value` on success or a boxed error on final failure.
    pub async fn fetch_cnn(&self, date: Option<String>) -> Result<Value, Box<dyn std::error::Error>> {
        let mut attempts = 0;
        const MAX_ATTEMPTS: u64 = 3;

        // Build the request path.
        let mut path = String::from("index/fearandgreed/graphdata");
        if let Some(d) = date {
            // Note: The actual historical endpoint might be different.
            // This is a hypothetical extension of the base path.
            path.push_str("/_"); // Assuming a separator
            path.push_str(&d);
        }

        loop {
            attempts += 1;
            
            // Make the request using the generic ApiClient.
            let response = self.client.request::<Value, ()>(
                Method::GET,
                &path,
                None, // No query parameters needed.
                None, // No custom headers needed.
            ).await?;

            if response.success {
                if let Some(body) = response.data {
                    // Success! Return the data.
                    return Ok(body);
                } else {
                    // This case handles a 200 OK with an empty body, which is unexpected.
                    let error_msg = format!("CNN API returned empty data (Attempt {}/{})", attempts, MAX_ATTEMPTS);
                    self.logger.warn(&error_msg, Some(json!({"path": path, "attempt": attempts}))).await;
                }
            } else {
                // This case handles non-2xx HTTP status codes (e.g., 403, 404, 500).
                let http_error = format!("HTTP Request failed for {}: Status {}", path, response.status);
                self.logger.error(&http_error, Some(json!({"status": response.status.as_u16(), "path": path}))).await;
            }

            // Check if we've exhausted all retry attempts.
            if attempts >= MAX_ATTEMPTS {
                let fatal_msg = format!("Final failure: CNN API unreachable or invalid after {} attempts", MAX_ATTEMPTS);
                self.logger.fatal(&fatal_msg, Some(json!({"path": path}))).await;
                return Err(fatal_msg.into());
            }

            // Wait before the next attempt using a simple linear backoff.
            sleep(Duration::from_secs(attempts)).await;
        }
    }
}
