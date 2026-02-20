//! # Market Status Live Data Test
//!
//! Connects to the Nasdaq API via lib_common to retrieve and display 
//! the raw data structure.

use lib_common::markets::nasdaq::marketstatus::MarketStatus;
use lib_common::markets::nasdaq::apicall::ApiCall;
use lib_common::loggers::loggerlocal::LoggerLocal;
use std::sync::Arc;

/// Executes the live market status fetch.
/// 
/// // Statement: Prints the full MarketStatusData struct to stdout on success.
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // // Statement: Initialize LoggerLocal with required app_name and None for default options
    // // Fixed E0061: Added "market_test" as app name and None for options
    let logger = Arc::new(LoggerLocal::new("market_test".to_string(), None));

    // // Statement: Initialize ApiCall service by providing the logger
    let api_call = Arc::new(ApiCall::new(Arc::clone(&logger)));

    // // Statement: Initialize the market status provider
    let provider = MarketStatus::new(api_call, logger);

    println!("[*] Requesting live data from Nasdaq API...");

    match provider.get_status().await {
        Ok(data) => {
            // // Statement: Success - Print the actual received data as formatted JSON
            println!("\n[SUCCESS] Data received:");
            println!("-----------------------------------------------");
            println!("{}", serde_json::to_string_pretty(&data)?);
            println!("-----------------------------------------------");
            
            // // Statement: Show calculated sleep duration for verification
            let sleep_dur = data.get_sleep_duration();
            println!("[INFO] Calculated sleep duration: {:?}", sleep_dur);
        }
        Err(e) => {
            // // Statement: Failure - Print specific error details to stderr
            eprintln!("\n[ERROR] Market status retrieval failed:");
            eprintln!(">>> {}", e);
            std::process::exit(1);
        }
    }

    Ok(())
}