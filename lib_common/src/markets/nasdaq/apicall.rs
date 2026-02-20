//! # Nasdaq API Call Module
//!
//! This module provides specialized logic for interacting with the Nasdaq API endpoints.
//! It handles the complexity of mimicking browser headers to avoid anti-bot detection
//! and implements manual retry logic for specific API response codes.

use crate::retrieve::ky_http::ApiClient;
use crate::loggers::loggerlocal::LoggerLocal;
use reqwest::header::{HeaderMap, HeaderValue};
use reqwest::Method;
use serde_json::Value;
use std::sync::Arc;
use tokio::time::{sleep, Duration};

/// Handles network calls to Nasdaq API services.
pub struct ApiCall {
    /// The underlying HTTP client used for requests.
    client: ApiClient,
    /// A shared logger instance for recording request status and errors.
    logger: Arc<LoggerLocal>,
}

impl ApiCall {
    /// Initializes a new `ApiCall` instance.
    ///
    /// # Arguments
    /// * `logger` - An `Arc` pointer to a `LoggerLocal` instance for thread-safe logging.
    pub fn new(logger: Arc<LoggerLocal>) -> Self {
        Self {
            // Initialize the client with the base Nasdaq API URL.
            client: ApiClient::new("https://api.nasdaq.com/", None),
            logger,
        }
    }

    /// Performs a GET request to a Nasdaq endpoint with automatic retries.
    ///
    /// This function mimics browser headers and will retry up to 3 times if the
    /// API returns a non-200 status code within its internal JSON response.
    ///
    /// # Arguments
    /// * `path` - The relative API path (e.g., "api/quote/AAPL/info?assetclass=stocks").
    ///
    /// # Errors
    /// Returns an error if the network fails or if the maximum number of retries is exceeded.
    pub async fn fetch_nasdaq(&self, path: &str) -> Result<Value, Box<dyn std::error::Error>> {
        let mut attempts = 0;
        let max_attempts = 3;

        loop {
            attempts += 1;
            // Generate a fresh set of browser-mimic headers for each attempt.
            let headers = self.get_nasdaq_headers();

            // Execute the network request.
            // Generics: <Value> for the expected response type, <()> for no request body.
            // Arguments: method, path, optional headers, optional body.
            let response = self.client.request::<Value, ()>(
                Method::GET,
                path,
                Some(headers),
                None,
            ).await?;

            // Check if the HTTP request itself was successful.
            if response.success {
                if let Some(json) = response.data {
                    // Nasdaq API often returns 200 OK but includes an error code in the JSON body.
                    let r_code = json["status"]["rCode"].as_i64().unwrap_or(0);
                    
                    if r_code == 200 {
                        return Ok(json);
                    }

                    // // Statement: Log internal error matching the required (level, msg, extras) signature.
                    self.logger.log(
                        1, // Log level
                        &format!("Nasdaq API Internal Error (rCode {}): Attempt {}/{}", r_code, attempts, max_attempts),
                        None // No extra JSON data
                    ).await;
                }
            } else {
                // // Statement: Log network failure matching the required (level, msg, extras) signature.
                self.logger.log(
                    1, // Log level
                    &format!("HTTP Failure (Status {}): Attempt {}/{}", response.status, attempts, max_attempts),
                    None
                ).await;
            }

            // Exit the loop if maximum retries are reached.
            if attempts >= max_attempts {
                return Err("Max retries exceeded for Nasdaq API".into());
            }

            // Apply a linear backoff delay (1s, 2s) before the next attempt.
            sleep(Duration::from_secs(attempts)).await;
        }
    }

    /// Constructs a `HeaderMap` containing headers required to mimic a standard web browser.
    ///
    /// Nasdaq's API filters requests that do not appear to originate from a valid browser
    /// session. This helper ensures all necessary security and session headers are present.
    fn get_nasdaq_headers(&self) -> HeaderMap {
        let mut headers = HeaderMap::new();
        
        // Define the list of headers to be injected into the request.
        let header_list = [
            ("accept", "application/json, text/plain, */*"),
            ("accept-language", "en-US,en;q=0.9"),
            ("cache-control", "no-cache"),
            ("dnt", "1"),
            ("origin", "https://www.nasdaq.com"),
            ("pragma", "no-cache"),
            ("referer", "https://www.nasdaq.com/"),
            ("sec-ch-ua", "\"Google Chrome\";v=\"135\", \"Not-A.Brand\";v=\"8\", \"Chromium\";v=\"135\""),
            ("sec-ch-ua-mobile", "?0"),
            ("sec-ch-ua-platform", "\"Windows\""),
            ("sec-fetch-dest", "empty"),
            ("sec-fetch-mode", "cors"),
            ("sec-fetch-site", "same-site"),
            ("user-agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/135.0.0.0 Safari/537.36"),
        ];

        // Iterate and insert headers into the map.
        for (key, val) in header_list {
            if let Ok(value) = HeaderValue::from_str(val) {
                headers.insert(key, value);
            }
        }

        headers
    }
}