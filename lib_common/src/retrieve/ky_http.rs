use reqwest::{header::{HeaderMap, AUTHORIZATION}, Method, Url};
use reqwest_middleware::{ClientBuilder, ClientWithMiddleware};
use reqwest_retry::{RetryTransientMiddleware, policies::ExponentialBackoff};
use serde::{de::DeserializeOwned, Serialize};
use serde_json; // This needs to be at the top

#[derive(Debug)] // Removed Serialize derive
pub struct ApiResponse<T> {
    pub data: Option<T>,
    pub error_body: Option<String>,
    pub status: u16,
    pub success: bool,
    pub headers: HeaderMap,
}

pub struct ApiClient {
    inner: ClientWithMiddleware,
    base_url: Url,
    auth_token: Option<String>,
}

impl ApiClient {
    pub fn new(base_url: &str, auth_token: Option<String>) -> Self {
        let url = Url::parse(base_url).expect("Invalid Base URL (must be absolute)");
        
        let retry_policy = ExponentialBackoff::builder().build_with_max_retries(3);
        
        // Standard reqwest client configured once
        let client = ClientBuilder::new(reqwest::Client::new())
            .with(RetryTransientMiddleware::new_with_policy(retry_policy))
            .build();

        Self {
            inner: client,
            base_url: url,
            auth_token,
        }
    }

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
        // 1. Join URL
        let full_url = self.base_url.join(path)?;
        let mut req = self.inner.request(method, full_url);

        // 2. Add Custom Headers (Quick & Dirty)
        if let Some(h) = headers {
            req = req.headers(h);
        }

        // 3. Optional Auth
        if let Some(token) = &self.auth_token {
            req = req.header(AUTHORIZATION, format!("Bearer {}", token));
        }

        // 4. Body
        if let Some(b) = body {
            use reqwest::header::CONTENT_TYPE; // This should be here
            let json_body = serde_json::to_string(&b)?;
            req = req.header(CONTENT_TYPE, "application/json").body(json_body);
        }

        // 5. Execution
        let response = req.send().await?;
        let status = response.status();
        let resp_headers = response.headers().clone();
        let success = status.is_success();

        if success {
            let data = response.json::<T>().await?;
            Ok(ApiResponse {
                data: Some(data),
                error_body: None,
                status: status.as_u16(),
                success: true,
                headers: resp_headers,
            })
        } else {
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