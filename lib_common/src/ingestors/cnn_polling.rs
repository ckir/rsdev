//! # CNN Polling Plugin
//! 
//! A self-scheduling ingestor for REST-based data sources.
//! Implements triple-timestamping and dynamic polling intervals 
//! based on data volatility or market state.

use std::sync::Arc;
use std::time::{Duration, Instant};
use serde_json::json;
use crate::core::dispatcher::Dispatcher;

/// Represents the data returned by a single polling cycle.
pub struct PollResult {
    /// Normalized data payload.
    pub data: serde_json::Value,
    /// Upstream server timestamp (if available).
    pub ts_upstream: u64,
    /// The plugin's requested delay before the next poll.
    pub next_delay: Duration,
}

pub struct CnnPollingPlugin {
    dispatcher: Arc<Dispatcher>,
    _client: reqwest::Client,
}

impl CnnPollingPlugin {
    /// Creates a new instance of the CNN Poller.
    pub fn new(dispatcher: Arc<Dispatcher>) -> Self {
        Self {
            dispatcher,
            // Configure a client with a reasonable timeout for polling
            _client: reqwest::Client::builder()
                .timeout(Duration::from_secs(10))
                .user_agent("ReStream/1.0")
                .build()
                .unwrap_or_default(),
        }
    }

    /// The main execution loop. 
    /// This task manages its own lifecycle and scheduling.
    pub async fn run(&self) {
        println!("CNN Polling Plugin started.");
        
        loop {
            // 1. Capture 'In' timestamp immediately before the network call
            let ts_in = Instant::now();

            match self.execute_poll().await {
                Ok(result) => {
                    // 2. Dispatch the data through the zero-copy pipeline
                    self.dispatcher.broadcast(
                        result.data,
                        result.ts_upstream,
                        ts_in
                    ).await;

                    // 3. Self-Schedule: Sleep for the duration determined by the plugin
                    tokio::time::sleep(result.next_delay).await;
                }
                Err(e) => {
                    eprintln!("CNN Polling Error: {}. Retrying in 60s...", e);
                    // Exponential backoff or static retry delay on error
                    tokio::time::sleep(Duration::from_secs(60)).await;
                }
            }
        }
    }

    /// Internal logic for the HTTP request and data normalization.
    async fn execute_poll(&self) -> Result<PollResult, String> {
        // In a real implementation, you would use:
        // let resp = self.client.get("https://hook.finance/cnn/feargreed").send().await...
        
        // Simulating a successful API response
        let mock_payload = json!({
            "indicator": "Fear & Greed",
            "score": 65,
            "rating": "Greed",
            "timestamp": 1700000000
        });

        // The plugin logic determines the next poll frequency.
        // For example: poll every 5 mins during market hours, 15 mins otherwise.
        let next_delay = Duration::from_secs(300); 

        Ok(PollResult {
            data: mock_payload,
            ts_upstream: 1700000000,
            next_delay,
        })
    }
}