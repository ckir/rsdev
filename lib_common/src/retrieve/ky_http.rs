//! # Generic HTTP API Client with Retry Middleware
//!
//! This module provides a robust and generic HTTP client (`ApiClient`) designed
//! for making requests to external APIs. It leverages `reqwest_middleware` to
//! automatically handle transient network errors with an exponential backoff
//! retry policy, making API interactions more resilient.
//!
//! ## Key Features:
//! - **Generic Request Handling**: Supports various HTTP methods (`GET`, `POST`, etc.)
//!   and can send JSON bodies and process JSON responses, typed by `serde`.
//! - **Automatic Retries**: Integrates `reqwest-retry` with an `ExponentialBackoff`
//!   policy, meaning failed requests (e.g., due to network glitches or server-side
//!   throttling) are automatically re-attempted.
//! - **Authentication Support**: Optionally attaches a Bearer token to requests
//!   for authenticated API endpoints.
//! - **Custom Headers**: Allows for injection of additional `HeaderMap` instances
//!   for specific API requirements (e.g., browser-mimicking headers).
//! - **Structured Responses**: Encapsulates API responses in `ApiResponse`, which
//!   provides a clear indication of success/failure, the deserialized data (if successful),
//!   and raw error bodies (if unsuccessful).

#![doc(html_logo_url = "https://example.com/logo.png")] // Placeholder
#![forbid(unsafe_code)]
#![warn(missing_docs, rust_2018_idioms, unused_qualifications)]

use reqwest::{header::{HeaderMap, AUTHORIZATION}, Method, Url};
use reqwest_middleware::{ClientBuilder, ClientWithMiddleware};
use reqwest_retry::{RetryTransientMiddleware, policies::ExponentialBackoff};
use serde::{de::DeserializeOwned, Serialize};
// This needs to be at the top level for proper macro expansion if used by derive.
use serde_json; 

/// # API Response Structure
///
/// A generic structure to encapsulate the outcome of an API request.
/// It distinguishes between successful responses with data and unsuccessful
/// responses (HTTP errors) with optional error details.
#[derive(Debug)]
pub struct ApiResponse<T> {
    /// The deserialized data payload if the request was successful and returned data.
    pub data: Option<T>,
    /// The raw error body as a string, typically present if `success` is `false`.
    pub error_body: Option<String>,
    /// The HTTP status code of the response.
    pub status: u16,
    /// A boolean indicating if the HTTP request was successful (status code 2xx).
    pub success: bool,
    /// The full set of HTTP headers returned by the server.
    pub headers: HeaderMap,
}

/// # API Client
///
/// A client for making HTTP requests to a specified base URL.
/// It includes retry logic and optional authentication.
pub struct ApiClient {
    /// The underlying `reqwest_middleware` client, configured with retry policies.
    inner: ClientWithMiddleware,
    /// The base URL for all requests made by this client. All `path` arguments
    /// in `request` will be joined against this base URL.
    base_url: Url,
    /// An optional Bearer token to be included in the `Authorization` header
    /// for authenticated requests.
    auth_token: Option<String>,
}

impl ApiClient {
    /// Creates a new `ApiClient` instance.
    ///
    /// It configures the internal `reqwest_middleware` client with an exponential
    /// backoff retry policy for transient errors (up to 3 retries).
    ///
    /// # Arguments
    /// * `base_url` - The base URL for all API requests (e.g., "https://api.example.com/").
    /// * `auth_token` - An optional `Bearer` token for authentication.
    ///
    /// # Panics
    /// If the provided `base_url` is not a valid absolute URL.
    pub fn new(base_url: &str, auth_token: Option<String>) -> Self {
        let url = Url::parse(base_url).expect("Invalid Base URL (must be absolute)");
        
        // Configure an exponential backoff policy with a maximum of 3 retries.
        let retry_policy = ExponentialBackoff::builder().build_with_max_retries(3);
        
        // Build the reqwest client with the retry middleware.
        let client = ClientBuilder::new(reqwest::Client::new())
            .with(RetryTransientMiddleware::new_with_policy(retry_policy))
            .build();

        Self {
            inner: client,
            base_url: url,
            auth_token,
        }
    }

    /// # Make a Request
    ///
    /// Sends an HTTP request to the configured API.
    ///
    /// ## Generics
    /// - `T`: The type into which the successful response body should be deserialized.
    ///   Must implement `DeserializeOwned`.
    /// - `B`: The type of the request body (e.g., for `POST` or `PUT` requests).
    ///   Must implement `Serialize`.
    ///
    /// ## Logic:
    /// 1.  **URL Construction**: Joins the provided `path` with the `base_url` to form
    ///     the complete request URL.
    /// 2.  **Request Building**: Initializes a `reqwest` request with the specified
    ///     `method` and `full_url`.
    /// 3.  **Custom Headers**: If `headers` are provided, they are added to the request.
    /// 4.  **Authentication**: If an `auth_token` was provided during client creation,
    ///     a `Bearer` token is added to the `Authorization` header.
    /// 5.  **Request Body**: If a `body` is provided, it is serialized to JSON and
    ///     set as the request body with the `Content-Type: application/json` header.
    /// 6.  **Execution**: The request is sent using the internal `ClientWithMiddleware`,
    ///     which handles retries automatically.
    /// 7.  **Response Processing**:
    ///     - If the HTTP status code is `2xx` (`success`), the response body is
    ///       deserialized into type `T` and returned within `ApiResponse::data`.
    ///     - If the HTTP status code is not `2xx` (`failure`), the raw response body
    ///       is captured as `error_body` and `data` is `None`.
    ///
    /// # Arguments
    /// * `method` - The HTTP method to use (e.g., `Method::GET`, `Method::POST`).
    /// * `path` - The API endpoint path relative to the `base_url`.
    /// * `body` - An `Option` containing the request body to be sent as JSON.
    /// * `headers` - An `Option` containing a `HeaderMap` for custom headers.
    ///
    /// # Returns
    /// A `Result` containing an `ApiResponse<T>` on success, or a boxed error
    /// (`Box<dyn std::error::Error>`) if the request itself fails (e.g., network error
    /// after retries, invalid URL, or JSON serialization/deserialization error).
    pub async fn request<T, B>(
        &self, 
        method: Method, 
        path: &str, 
        body: Option<B>,
        headers: Option<HeaderMap>
    ) -> Result<ApiResponse<T>, Box<dyn std::error::Error>> 
    where 
        T: DeserializeOwned,
        B: Serialize,
    {
        // 1. Construct the full URL by joining the base URL with the path.
        let full_url = self.base_url.join(path)?;
        let mut req = self.inner.request(method, full_url);

        // 2. Add any custom headers provided by the caller.
        if let Some(h) = headers {
            req = req.headers(h);
        }

        // 3. Attach a Bearer token for authentication if available.
        if let Some(token) = &self.auth_token {
            req = req.header(AUTHORIZATION, format!("Bearer {}", token));
        }

        // 4. Serialize and attach the request body if present.
        if let Some(b) = body {
            // Need to bring CONTENT_TYPE into scope for use with reqwest::header
            use reqwest::header::CONTENT_TYPE; 
            let json_body = serde_json::to_string(&b)?;
            req = req.header(CONTENT_TYPE, "application/json").body(json_body);
        }

        // 5. Send the request and process the response.
        let response = req.send().await?;
        let status = response.status();
        let resp_headers = response.headers().clone();
        let success = status.is_success();

        if success {
            // If the request was successful, attempt to deserialize the JSON body.
            let data = response.json::<T>().await?;
            Ok(ApiResponse {
                data: Some(data),
                error_body: None,
                status: status.as_u16(),
                success: true,
                headers: resp_headers,
            })
        } else {
            // If the request was not successful, capture the raw error body.
            let error_text = response.text().await.ok();
            Ok(ApiResponse {
                data: None,
                error_body: error_text,
                status: status.as_u16(),
                success: false,
                headers: resp_headers,
            })
        }
    }
}