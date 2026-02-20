//! # HTTP Retrieval Utilities
//! 
//! This module provides a robust, asynchronous API client wrapper around `reqwest`.
//! it includes middleware support for exponential backoff retries and standardized
//! JSON response handling.

use reqwest::{header::{HeaderMap, AUTHORIZATION}, Method, Url};
use reqwest_middleware::{ClientBuilder, ClientWithMiddleware};
use reqwest_retry::{RetryTransientMiddleware, policies::ExponentialBackoff};
use serde::{de::DeserializeOwned, Serialize};
use serde_json;

/// A standardized container for API responses.
/// 
/// This struct wraps the deserialized data along with metadata about the 
/// HTTP transaction, such as status codes and headers.
#[derive(Debug)]
pub struct ApiResponse<T> {
    /// The successfully deserialized response body, if any.
    pub data: Option<T>,
    /// The raw error body returned by the server if the request failed.
    pub error_body: Option<String>,
    /// The numeric HTTP status code.
    pub status: u16,
    /// Indicates if the status code was in the 2xx range.
    pub success: bool,
    /// The headers returned by the server.
    pub headers: HeaderMap,
}

/// A flexible asynchronous HTTP client.
/// 
/// Built on top of `reqwest_middleware`, it handles base URLs,
/// authentication tokens, and automatic retries.
pub struct ApiClient {
    /// The underlying middleware-enabled client.
    inner: ClientWithMiddleware,
    /// The base URL to which all relative paths are joined.
    base_url: Url,
    /// An optional Bearer token used for authorization.
    auth_token: Option<String>,
}

impl ApiClient {
    /// Creates a new `ApiClient` instance with a retry policy.
    ///
    /// # Arguments
    /// * `base_url` - The absolute base URL for the API (e.g., "https://api.example.com/v1/").
    /// * `auth_token` - An optional string for the Authorization header.
    ///
    /// # Panics
    /// Panics if the `base_url` is not a valid absolute URL.
    pub fn new(base_url: &str, auth_token: Option<String>) -> Self {
        // Parse the base URL to ensure it is valid and absolute
        let url = Url::parse(base_url).expect("Invalid Base URL (must be absolute)");
        
        // Configure an exponential backoff policy with 3 retries
        let retry_policy = ExponentialBackoff::builder().build_with_max_retries(3);
        
        // Construct the client with the retry middleware
        let client = ClientBuilder::new(reqwest::Client::new())
            .with(RetryTransientMiddleware::new_with_policy(retry_policy))
            .build();

        Self {
            inner: client,
            base_url: url,
            auth_token,
        }
    }

    /// Performs a generic HTTP request and handles the response.
    ///
    /// This method manages URL joining, header injection, authentication, 
    /// and JSON serialization/deserialization.
    ///
    /// # Arguments
    /// * `method` - The HTTP verb (GET, POST, etc.).
    /// * `path` - The relative path to append to the base URL.
    /// * `headers` - Optional additional headers for this specific request.
    /// * `body` - Optional serializable object to send as the JSON body.
    ///
    /// # Errors
    /// Returns an `anyhow::Error` if URL joining or network execution fails.
    pub async fn request<T, B>(
        &self,
        method: Method,
        path: &str,
        headers: Option<HeaderMap>,
        body: Option<B>,
    ) -> anyhow::Result<ApiResponse<T>>
    where
        T: DeserializeOwned,
        B: Serialize,
    {
        // 1. Construct the full absolute URL
        let full_url = self.base_url.join(path)?;
        let mut req = self.inner.request(method, full_url);

        // 2. Add Custom Headers if provided
        if let Some(h) = headers {
            req = req.headers(h);
        }

        // 3. Inject Bearer Authentication if a token is present
        if let Some(token) = &self.auth_token {
            req = req.header(AUTHORIZATION, format!("Bearer {}", token));
        }

        // 4. Serialize and attach the JSON body if present
        if let Some(b) = body {
            use reqwest::header::CONTENT_TYPE;
            let json_body = serde_json::to_string(&b)?;
            req = req.header(CONTENT_TYPE, "application/json").body(json_body);
        }

        // 5. Execute the request and capture response metadata
        // Explicitly type the response to fix previous inference errors
        let response: reqwest::Response = req.send().await?;
        let status = response.status();
        let resp_headers = response.headers().clone();
        let success = status.is_success();

        // 6. Handle the result based on success status
        if success {
            // Attempt to deserialize the body into the target type T
            let data = response.json::<T>().await?;
            Ok(ApiResponse {
                data: Some(data),
                error_body: None,
                status: status.as_u16(),
                success: true,
                headers: resp_headers,
            })
        } else {
            // Capture the error body as a string for debugging
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