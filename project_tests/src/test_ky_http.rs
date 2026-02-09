//! # `ky_http` Client Integration Tests
//!
//! This module contains integration tests for the `lib_common::retrieve::ky_http::ApiClient`.
//! It uses the `httpbin.org` service as a public, well-behaved endpoint for testing
//! various HTTP request functionalities, such as URL joining, custom headers,
//! authentication, error handling, and JSON body serialization.
//!
//! ## Purpose:
//! The primary goal of these tests is to ensure that the `ApiClient` correctly
//! constructs requests, handles responses, and integrates with `reqwest_middleware`'s
//! retry mechanisms and `serde`'s serialization/deserialization.
//!
//! These tests are executed asynchronously using `tokio::main`.

#![doc(html_logo_url = "https://example.com/logo.png")] // Placeholder for consistency
#![forbid(unsafe_code)]
#![warn(missing_docs, rust_2018_idioms, unused_qualifications)]

use reqwest::header::{HeaderMap, HeaderValue};
use reqwest::Method;
use serde::{Deserialize, Serialize};

use lib_common::retrieve::ky_http::{ApiClient, ApiResponse};

/// # Httpbin Response Model
///
/// A utility struct to deserialize responses from `httpbin.org`.
/// `httpbin.org` often echoes back parts of the request (like headers, URL, JSON body),
/// which is useful for verifying our `ApiClient`'s behavior.
#[derive(Debug, Deserialize, Serialize)]
struct HttpbinResponse {
    /// Echoed headers sent with the request.
    headers: Option<std::collections::HashMap<String, String>>,
    /// The URL that was hit, as seen by httpbin.org.
    url: Option<String>, 
    /// The JSON body that was sent in a POST request.
    json: Option<serde_json::Value>, 
}

/// # Main Test Function
///
/// Executes a series of integration tests for the `ApiClient` against `httpbin.org`.
///
/// Each test case verifies a specific aspect of the `ApiClient`'s functionality.
#[tokio::main]
/// # Main Test Function
///
/// Executes a series of integration tests for the `ApiClient` against `httpbin.org`.
///
/// Each test case verifies a specific aspect of the `ApiClient`'s functionality.
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. Initialize ApiClient
    // We use a test base URL and a mock authentication token for consistency.
    let base_url = "https://httpbin.org/";
    let api = ApiClient::new(base_url, Some("test_secret_123".into()));
    
    println!("--- Starting API Module Tests ---");

    // --- TEST 1: URL Joining & Success Response ---
    // Verifies that the ApiClient correctly joins the base URL and the path,
    // and successfully deserializes a standard GET response.
    println!("
[Test 1] Testing URL Joining & Success...");
    let res1 = api.request::<HttpbinResponse, ()>(
        Method::GET, 
        "get", // This path should be appended to the base_url.
        None, // No request body.
        None  // No custom headers.
    ).await?;
    
    // Assert that the request was marked as successful.
    assert!(res1.success);
    // Print the URL to confirm it was correctly constructed.
    println!("✅ URL Joined: {:?}", res1.data.as_ref().unwrap().url);

    // --- TEST 2: Custom Headers & Auth Token ---
    // Verifies that both custom headers and the configured authentication token
    // are correctly added to the outgoing request.
    println!("
[Test 2] Testing Custom Headers & Auth Token...");
    let mut headers = HeaderMap::new();
    headers.insert("X-Custom-Client", HeaderValue::from_static("Rust-Test-Suite"));
    
    let res2 = api.request::<HttpbinResponse, ()>(
        Method::GET, 
        "headers", // httpbin.org/headers echoes back all request headers.
        None, 
        Some(headers) // Inject custom headers.
    ).await?;

    let echoed_headers = res2.data.unwrap().headers.unwrap();
    // Assert that the custom header is present and correct.
    println!("✅ Custom Header: {}", echoed_headers.get("X-Custom-Client").unwrap());
    // Assert that the Authorization header (from `auth_token`) is present.
    println!("✅ Auth Token: {}", echoed_headers.get("Authorization").unwrap());

    // --- TEST 3: Failures (Non-throwing 404) ---
    // Verifies that the ApiClient correctly handles non-2xx HTTP status codes
    // without throwing an error, instead returning `ApiResponse::success` as `false`.
    println!("
[Test 3] Testing 404 handling (Should return Result::Ok with success: false)...");
    let res3 = api.request::<serde_json::Value, ()>(
        Method::GET, 
        "status/404", // This endpoint returns a 404 status.
        None, 
        None
    ).await?;

    // Assert that the request was NOT marked as successful, but also didn't panic.
    assert!(!res3.success);
    // Assert that the HTTP status code is correctly captured.
    assert_eq!(res3.status, 404);
    println!("✅ Non-throwing failure handled. Status: {}", res3.status);

    // --- TEST 4: POST with JSON Body ---
    // Verifies that a Rust struct can be correctly serialized into a JSON request body
    // and sent via a POST request.
    println!("
[Test 4] Testing POST Body serialization...");
    #[derive(Serialize)]
    struct MyBody { message: String }
    let body = MyBody { message: "Hello from Rust".into() };

    let res4 = api.request::<HttpbinResponse, MyBody>(
        Method::POST, 
        "post", // httpbin.org/post echoes back the JSON request body.
        Some(body), // Provide a struct to be serialized as JSON.
        None
    ).await?;

    // Assert that the server received and echoed the correct JSON body.
    println!("✅ POST Success. Server received: {:?}", res4.data.unwrap().json);

    println!("
--- All Tests Passed Successfully ---");
    Ok(())
}
