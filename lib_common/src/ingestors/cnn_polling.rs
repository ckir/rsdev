//! # CNN Fear & Greed Polling Ingestor
//!
//! A self-scheduling ingestor designed for REST-based data sources that do not
//! provide a real-time streaming interface. This plugin polls the CNN Fear & Greed
//! index as a demonstration of a resilient, scheduled data fetching mechanism.
//!
//! ## Key Design Principles:
//! - **Self-Scheduling**: Unlike a streaming ingestor that reacts to incoming data,
//!   this plugin is responsible for its own timing. It runs in a loop, performs a
//!   poll, and then sleeps for a calculated duration before the next poll.
//! - **Dynamic Intervals**: The duration between polls (`next_delay`) is determined
-//!   by the plugin itself after each poll. This allows it to be adaptive, for
//!   example, polling more frequently during market hours and less frequently
//!   during off-hours.
//! - **Resilience**: Implements a simple retry mechanism with a fixed delay on
//!   polling errors, making it tolerant of transient network issues.
//! - **Triple-Timestamping**: The `run` loop captures an "in" timestamp (`ts_in`) just
//!   before the network request. This, combined with an upstream timestamp and a
//!   dispatcher timestamp, allows for precise latency analysis of the entire data
//!   pipeline.

#![doc(html_logo_url = "https://example.com/logo.png")] // Placeholder
#![forbid(unsafe_code)]
#![warn(missing_docs, rust_2018_idioms, unused_qualifications)]

use std::sync::Arc;
use std::time::{Duration, Instant};
use serde_json::json;
use crate::core::dispatcher::Dispatcher;

/// # Poll Result
///
/// Represents the data returned by a single, successful polling cycle.
///
/// This struct standardizes the output of the `execute_poll` function, ensuring
/// that all necessary information is passed back to the main run loop for
/// processing and scheduling.
pub struct PollResult {
    /// The normalized data payload, structured as a `serde_json::Value`.
    /// Normalization happens within the poll logic, so the dispatcher receives
    /// a consistent data format.
    pub data: serde_json::Value,
    /// The timestamp provided by the upstream server, if available (in Unix seconds).
    /// This is a critical part of the triple-timestamping system. A value of `0`
    /// is used if no upstream timestamp is provided.
    pub ts_upstream: u64,
    /// The duration the plugin should wait before initiating the next poll. This
    /// value is determined by the plugin's internal logic and allows for dynamic,
    /// self-managed scheduling.
    pub next_delay: Duration,
}

/// # CNN Polling Plugin
///
/// The main struct for the self-scheduling polling ingestor.
pub struct CnnPollingPlugin {
    /// A shared reference to the central `Dispatcher` to broadcast the polled data.
    dispatcher: Arc<Dispatcher>,
    /// An `reqwest::Client` instance for making HTTP requests. It is configured
    /// with a timeout and a custom user agent and is reused across all polls to
    /// leverage connection pooling.
    _client: reqwest::Client,
}

impl CnnPollingPlugin {
    /// Creates a new instance of the `CnnPollingPlugin`.
    ///
    /// It initializes a shared `reqwest::Client` with sensible defaults, such as
    /// a request timeout to prevent the poller from hanging indefinitely on an
    /// unresponsive server.
    pub fn new(dispatcher: Arc<Dispatcher>) -> Self {
        Self {
            dispatcher,
            _client: reqwest::Client::builder()
                .timeout(Duration::from_secs(10))
                .user_agent("ReStream/1.0")
                .build()
                .unwrap_or_default(), // Fallback to a default client if builder fails.
        }
    }

    /// # Main Execution Loop
    ///
    /// This is the entry point for the long-running async task. It orchestrates the
    /// polling, dispatching, and sleeping cycle.
    ///
    /// ## Workflow:
    /// 1.  **Capture Timestamp**: Records `ts_in` (`Instant`) immediately before the poll
    ///     to accurately measure network and processing latency.
    /// 2.  **Execute Poll**: Calls the internal `execute_poll` method.
    /// 3.  **On Success**:
    ///     -   It receives a `PollResult`.
    ///     -   It broadcasts the normalized `data` to the `Dispatcher`, passing along
    ///       the `ts_upstream` and `ts_in` timestamps.
    ///     -   It then sleeps for the `next_delay` duration specified in the result,
    ///       effectively scheduling its next run.
    /// 4.  **On Error**:
    ///     -   It logs the error.
    ///     -   It sleeps for a fixed, longer duration (e.g., 60 seconds) before
    ///       retrying. A more advanced implementation might use exponential backoff.
    pub async fn run(&self) {
        println!("CNN Polling Plugin started.");
        
        loop {
            // --- Phase 1: Capture 'In' Timestamp ---
            let ts_in = Instant::now();

            // --- Phase 2: Execute Poll Logic ---
            match self.execute_poll().await {
                Ok(result) => {
                    // --- Phase 3a: Dispatch Data on Success ---
                    self.dispatcher.broadcast(
                        result.data,
                        result.ts_upstream,
                        ts_in
                    ).await;

                    // --- Phase 3b: Self-Schedule for Next Poll ---
                    tokio::time::sleep(result.next_delay).await;
                }
                Err(e) => {
                    // --- Phase 4: Handle Error and Schedule Retry ---
                    eprintln!("CNN Polling Error: {}. Retrying in 60s...", e);
                    tokio::time::sleep(Duration::from_secs(60)).await;
                }
            }
        }
    }

    /// # Execute Poll
    ///
    /// Contains the specific logic for fetching and normalizing data from the target
    /// REST API.
    ///
    /// **Note**: This is a MOCK implementation. A real implementation would use the
    /// `_client` field to make an actual HTTP `GET` request to the CNN API endpoint.
    ///
    /// ## Mock Logic:
    /// - It simulates a successful API response with a predefined JSON payload.
    /// - It defines a static `next_delay` (e.g., 5 minutes). In a real scenario, this
    ///   could be determined dynamically based on market hours or other factors.
    /// - It packages the mock data, upstream timestamp, and delay into a `PollResult`.
    ///
    /// # Returns
    /// A `Result` containing either a successful `PollResult` or a `String` error.
    async fn execute_poll(&self) -> Result<PollResult, String> {
        // --- Real Implementation Placeholder ---
        // let resp = self._client.get("https://production.dataviz.cnn.io/index/fearandgreed/graphdata")
        //     .send()
        //     .await
        //     .map_err(|e| e.to_string())?;
        // let data = resp.json::<serde_json::Value>().await.map_err(|e| e.to_string())?;
        
        // --- Mock Implementation ---
        // Simulating a successful API response for the Fear & Greed Index.
        let mock_payload = json!({
            "indicator": "Fear & Greed",
            "score": 65,
            "rating": "Greed",
            "source": "cnn_poll",
            "timestamp": 1700000000 // Mock upstream Unix timestamp
        });

        // The plugin's own logic determines the next poll frequency.
        // For example, poll every 5 minutes during market hours, 15 mins otherwise.
        let next_delay = Duration::from_secs(300); // 5 minutes

        Ok(PollResult {
            data: mock_payload,
            ts_upstream: 1700000000,
            next_delay,
        })
    }
}