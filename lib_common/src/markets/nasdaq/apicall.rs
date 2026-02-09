//! # NASDAQ API Client
//!
//! Provides a specialized and resilient client for making requests to the official
//! NASDAQ API.
//!
//! ## Key Features:
//! - **Browser Mimicry**: Constructs a set of `HeaderMap` that mimics a modern web
//!   browser. This is often necessary to bypass simple anti-bot measures on public
//!   APIs that expect requests to originate from a browser context.
//! - **Resilient Fetching**: The `fetch_nasdaq` method implements a robust retry
//!   loop. It handles not only network/HTTP-level errors but also application-level
//!   errors specific to the NASDAQ API (indicated by the `rCode` in the response body).
//! - **Structured Logging**: Integrates with `LoggerLocal` to provide detailed,
//!   structured logs for both successful and failed API calls, capturing crucial
_   //   debug information like the `rCode` and server messages.
//! - **Targeted Data Extraction**: On a successful call, it automatically unwraps the
//!   nested JSON structure to return only the relevant `data` object, simplifying
//!   the data handling for consumers of this client.

#![doc(html_logo_url = "https://example.com/logo.png")] // Placeholder
#![forbid(unsafe_code)]
#![warn(missing_docs, rust_2018_idioms, unused_qualifications)]

use crate::retrieve::ky_http::ApiClient;
use crate::loggers::loggerlocal::LoggerLocal;
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use reqwest::Method;
use serde_json::Value;
use std::sync::Arc;
use tokio::time::{sleep, Duration};

/// # NASDAQ API Call Client
///
/// A specialized client for making authenticated and resilient requests to the
/// NASDAQ API. It encapsulates an `ApiClient` and a logger to provide a
/// high-level interface for fetching NASDAQ data.
pub struct ApiCall {
    /// The underlying generic HTTP client, pre-configured for the NASDAQ API base URL.
    client: ApiClient,
    /// A shared logger for recording the outcomes of API calls.
    logger: Arc<LoggerLocal>,
}

impl ApiCall {
    /// # New `ApiCall`
    ///
    /// Initializes a new client for the NASDAQ API.
    ///
    /// It configures the internal `ApiClient` with the production base URL for the
    /// NASDAQ API.
    ///
    /// ## Arguments
    /// * `logger` - A shared `LoggerLocal` instance for logging.
    pub fn new(logger: Arc<LoggerLocal>) -> Self {
        Self {
            client: ApiClient::new("https://api.nasdaq.com/", None),
            logger,
        }
    }

    /// # Fetch NASDAQ Data
    ///
    /// Performs a `GET` request to a specified NASDAQ API path, with built-in
    /// browser-mimicking headers and a robust retry mechanism.
    ///
    /// ## Logic:
    /// 1.  Enters a loop that will try up to `max_attempts` times.
    /// 2.  **Generate Headers**: On each attempt, it calls `get_nasdaq_headers()` to
    ///     create a fresh set of headers that mimic a browser.
    /// 3.  **Execute Request**: It uses the generic `ApiClient` to perform the request.
    /// 4.  **Process Response**:
    ///     -   It checks for network-level success first.
    ///     -   If the network request is OK, it then inspects the JSON body for the
    ///         NASDAQ-specific status code (`rCode`).
    ///     -   **Success**: The request is only considered successful if the HTTP status
    ///       is `2xx`, the `rCode` is `200`, and the `data` field in the response is not `null`.
    ///     -   **Failure**: Any other condition (e.g., non-200 `rCode`, null data, or a
    ///       non-`2xx` HTTP status) is treated as a failure. It logs a detailed warning
    ///       or error and proceeds to the next retry attempt.
    /// 5.  **Retry**: After a failure, it sleeps for a short duration (linear backoff)
    ///     before the next attempt.
    /// 6.  **Final Failure**: If all attempts are exhausted, it logs a `fatal` error and
    ///     returns a final, conclusive error.
    ///
    /// ## Arguments
    /// * `path` - The specific API endpoint path to request (e.g., "api/market-info").
    ///
    /// # Returns
    /// A `Result` containing the extracted `data` field as a `serde_json::Value` on
    /// success, or a boxed error on final failure.
    pub async fn fetch_nasdaq(&self, path: &str) -> Result<Value, Box<dyn std::error::Error>> {
        let mut attempts = 0;
        const MAX_ATTEMPTS: u64 = 3;

        loop {
            attempts += 1;
            let headers = self.get_nasdaq_headers();

            // 1. Execute the network request using the underlying client.
            let response = self.client.request::<Value, ()>(
                Method::GET,
                path,
                None, // No query params needed for this specific path
                Some(headers),
            ).await?;

            // 2. Process the response.
            if response.success {
                if let Some(body) = response.data {
                    // NASDAQ API has its own status code within the JSON body.
                    let r_code = body["status"]["rCode"].as_i64().unwrap_or(-1);
                    let data = &body["data"];

                    // The strict success condition: HTTP 200, rCode 200, and non-null data.
                    if r_code == 200 && !data.is_null() {
                        return Ok(data.clone());
                    }

                    // Handle API-level errors (e.g., bad request, auth failure).
                    let log_extras = json!({
                        "path": path,
                        "attempt": attempts,
                        "rCode": r_code,
                        "data_is_null": data.is_null(),
                        "server_message": body.get("message") // Use .get for safety
                    });

                    let error_msg = format!("Nasdaq API business logic error: rCode {} (Attempt {}/{})", r_code, attempts, MAX_ATTEMPTS);
                    self.logger.warn(&error_msg, Some(log_extras)).await;

                } else {
                    // Handle cases where we get a 2xx response but an empty body.
                     self.logger.warn(&format!("Nasdaq API returned success status but empty body (Attempt {}/{})", attempts, MAX_ATTEMPTS), Some(json!({"path": path}))).await;
                }
            } else {
                // Handle HTTP-level errors (e.g., 403, 404, 500).
                let http_error = format!("HTTP Request failed for {}: Status {}", path, response.status);
                self.logger.error(&http_error, Some(json!({"path": path, "status": response.status.as_u16()}))).await;
            }

            // 3. Retry Logic
            if attempts >= MAX_ATTEMPTS {
                let fatal_msg = format!("Final failure: Nasdaq API unreachable or invalid after {} attempts for path '{}'", MAX_ATTEMPTS, path);
                self.logger.fatal(&fatal_msg, None).await;
                return Err(fatal_msg.into());
            }

            // Delay before the next attempt using a linear backoff.
            sleep(Duration::from_secs(attempts)).await;
        }
    }

    /// # Get NASDAQ Headers
    ///
    /// Constructs a `HeaderMap` that mimics a legitimate browser request.
    ///
    /// Public APIs like NASDAQ's often have simple checks to deter programmatic
    /// access. By sending headers that are typical of a Chrome browser on Windows,
    /// we significantly increase the likelihood of a successful API response.
    /// This includes setting `User-Agent`, `Referer`, `Origin`, and various
    /// `sec-ch-ua` (Client Hints) headers.
    fn get_nasdaq_headers(&self) -> HeaderMap {
        let mut headers = HeaderMap::new();
        
        // This list of headers is curated to appear like a standard browser request.
        let header_list = [
            ("accept", "application/json, text/plain, */*"),
            ("accept-language", "en-US,en;q=0.9"),
            ("origin", "https://www.nasdaq.com"),
            ("referer", "https://www.nasdaq.com/"),
            ("user-agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/126.0.0.0 Safari/537.36"),
            // Client Hint headers for added realism
            ("sec-ch-ua", "\"Not/A)Brand\";v=\"8\", \"Chromium\";v=\"126\", \"Google Chrome\";v=\"126\""),
            ("sec-ch-ua-mobile", "?0"),
            ("sec-ch-ua-platform", "\"Windows\""),
            ("sec-fetch-dest", "empty"),
            ("sec-fetch-mode", "cors"),
            ("sec-fetch-site", "same-site"),
        ];

        for (name, value) in header_list {
            if let (Ok(h_name), Ok(h_value)) = (HeaderName::from_bytes(name.as_bytes()), HeaderValue::from_str(value)) {
                headers.insert(h_name, h_value);
            }
        }

        headers
    }
}