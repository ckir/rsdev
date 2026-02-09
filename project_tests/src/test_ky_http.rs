use reqwest::header::{HeaderMap, HeaderValue};
use reqwest::Method;
use serde::{Deserialize, Serialize};

use lib_common::retrieve::ky_http::ApiClient;

#[derive(Debug, Deserialize, Serialize)]
struct HttpbinResponse {
    headers: Option<std::collections::HashMap<String, String>>,
    url: Option<String>, // Make url optional
    json: Option<serde_json::Value>, // Make json optional
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. Initialize with Base URL and a Test Token
    let base_url = "https://httpbin.org/";
    let api = ApiClient::new(base_url, Some("test_secret_123".into()));
    
    println!("--- Starting API Module Tests ---");

    // TEST 1: URL Joining & Success Response
    // Path "get" should join with base to form https://httpbin.org/get
    println!("
[Test 1] Testing URL Joining & Success...");
    let res1 = api.request::<HttpbinResponse, ()>(
        Method::GET, 
        "get", 
        None, 
        None
    ).await?;
    
    assert!(res1.success);
    println!("✅ URL Joined: {:?}", res1.data.as_ref().unwrap().url);

    // TEST 2: Custom Headers (Quick & Dirty)
    println!("
[Test 2] Testing Custom Headers & Auth Token...");
    let mut headers = HeaderMap::new();
    headers.insert("X-Custom-Client", HeaderValue::from_static("Rust-Test-Suite"));
    
    let res2 = api.request::<HttpbinResponse, ()>(
        Method::GET, 
        "headers", 
        None, 
        Some(headers)
    ).await?;

    let echoed_headers = res2.data.unwrap().headers.unwrap();
    println!("✅ Custom Header: {}", echoed_headers.get("X-Custom-Client").unwrap());
    println!("✅ Auth Token: {}", echoed_headers.get("Authorization").unwrap());

    // TEST 3: Failures (Non-throwing 404)
    println!("
[Test 3] Testing 404 handling (Should return Result::Ok with success: false)...");
    let res3 = api.request::<serde_json::Value, ()>(
        Method::GET, 
        "status/404", 
        None, 
        None
    ).await?;

    assert!(!res3.success);
    assert_eq!(res3.status, 404);
    println!("✅ Non-throwing failure handled. Status: {}", res3.status);

    // TEST 4: POST with JSON Body
    println!("
[Test 4] Testing POST Body serialization...");
    #[derive(Serialize)]
    struct MyBody { message: String }
    let body = MyBody { message: "Hello from Rust".into() };

    let res4 = api.request::<HttpbinResponse, MyBody>(
        Method::POST, 
        "post", 
        Some(body), 
        None
    ).await?;

    println!("✅ POST Success. Server received: {:?}", res4.data.unwrap().json);

    println!("
--- All Tests Passed Successfully ---");
    Ok(())
}
