//! # Yahoo Finance WebSocket (WSS) Ingestor
//!
//! This module provides a high-performance, resilient WebSocket client for
//! consuming real-time financial data from Yahoo Finance's protobuf stream.
//!
//! ## Core Functionality:
//! - **Connection Management**: Automatically connects to the Yahoo WSS endpoint
//!   and handles the entire connection lifecycle.
//! - **Resilience**: Implements robust reconnection logic with exponential backoff
//!   to handle network interruptions, server-side disconnects, and transient failures.
//! - **State-Aware Operation**: Integrates with the `UpstreamManager` to respect the
//!   global `OperationMode`. It will only attempt to connect when the system is in
//!   `Streaming` mode, and will disconnect and sleep during `Idle` (market closed) periods.
//! - **Protocol Handling**: Differentiates between binary (Protobuf) and text messages,
//!   with a focus on decoding the binary frames which contain the core market data.
//! - **Inactivity Watchdog**: Includes a timeout mechanism to detect "zombie" connections
//!   where the TCP socket is open but no data is flowing, forcing a reconnect to
-//!   ensure data freshness.
//!
//! The `YahooWssIngestor` is designed to be run as a long-lived, dedicated async task
//! that continuously feeds data into the central `Dispatcher`.

#![doc(html_logo_url = "https://example.com/logo.png")] // Placeholder
#![forbid(unsafe_code)]
#![warn(missing_docs, rust_2018_idioms, unused_qualifications)]

use std::sync::Arc;
use std::time::{Duration, Instant};
use futures_util::StreamExt;
use tokio_tungstenite::{connect_async, tungstenite::protocol::Message};
use serde_json::json;

use crate::core::dispatcher::Dispatcher;
use crate::core::upstream_manager::{UpstreamManager, OperationMode};

/// # Yahoo WSS Configuration
///
/// Holds all the settings required to connect and interact with the Yahoo Finance
/// WebSocket stream.
pub struct YahooConfig {
    /// The URL of the Yahoo Finance WebSocket endpoint.
    pub wss_url: String,
    /// The interval at which to expect heartbeats (pings/pongs) from the server.
    /// While not used to send heartbeats, it's related to the inactivity logic.
    pub heartbeat_interval: Duration,
    /// The maximum duration to wait without receiving any message (data or heartbeat)
    /// before considering the connection "zombie" and forcing a reconnect. This is
    /// crucial for preventing silent connection failures.
    pub silent_failure_timeout: Duration,
}

impl Default for YahooConfig {
    /// Provides a default, production-ready configuration for the ingestor.
    fn default() -> Self {
        Self {
            wss_url: "wss://streamer.finance.yahoo.com".to_string(),
            heartbeat_interval: Duration::from_secs(30),
            // This timeout should be longer than the expected interval between messages
            // during low-volume periods, but short enough to detect a dead connection quickly.
            silent_failure_timeout: Duration::from_secs(20),
        }
    }
}

/// # Yahoo WebSocket Ingestor
///
/// The primary struct responsible for managing the connection to Yahoo Finance,
/// processing incoming messages, and dispatching them to the rest of the system.
pub struct YahooWssIngestor {
    /// The configuration settings for the ingestor.
    config: YahooConfig,
    /// A shared reference to the central `Dispatcher` for broadcasting normalized data.
    dispatcher: Arc<Dispatcher>,
    /// A shared reference to the `UpstreamManager` to query the global system state.
    manager: Arc<UpstreamManager>,
}

impl YahooWssIngestor {
    /// Creates a new `YahooWssIngestor` instance.
    ///
    /// # Arguments
    /// * `config` - The configuration for the WebSocket connection.
    /// * `dispatcher` - The central dispatcher to send normalized data to.
    /// * `manager` - The system's state manager to check for operational readiness.
    pub fn new(config: YahooConfig, dispatcher: Arc<Dispatcher>, manager: Arc<UpstreamManager>) -> Self {
        Self { 
            config, 
            dispatcher, 
            manager 
        }
    }

    /// # Primary Execution Loop
    ///
    /// This is the main entry point for the ingestor task. It contains an infinite loop
    /// that manages the connection lifecycle.
    ///
    /// ## Logic:
    /// 1.  **Check Mode**: Before anything else, it checks the `UpstreamManager`'s state.
    ///     If the system is `Idle`, it sleeps and loops, effectively pausing itself.
    /// 2.  **Connect**: Attempts to establish a WebSocket connection. If it fails, it
    ///     logs the error and retries after a delay.
    /// 3.  **Event Loop**: On a successful connection, it enters an inner `tokio::select!` loop:
    ///     -   It concurrently listens for incoming WebSocket messages and a watchdog timer.
    ///     -   **Message Handling**: Processes binary (Protobuf) and text messages.
    ///       Any message receipt resets the `last_activity` timer.
    ///     -   **Watchdog**: A timer fires periodically (e.g., every second).
    ///         - It checks if `last_activity` has exceeded the `silent_failure_timeout`.
    ///           If so, it breaks the inner loop to force a reconnect.
    ///         - It checks if the `UpstreamManager` has transitioned to `Idle`.
    ///           If so, it breaks the inner loop to gracefully close the connection.
    /// 4.  **Reconnect**: When the inner loop breaks (due to error, timeout, or mode change),
    ///     the outer loop continues, leading to a reconnection attempt.
    pub async fn run(&self) {
        loop {
            // --- Phase 1: Pre-Connection State Check ---
            // Don't even try to connect if the market is closed. This prevents flapping
            // and unnecessary log spam during off-hours.
            if self.manager.get_current_mode().await == OperationMode::Idle {
                log::debug!("System is IDLE. Yahoo WSS ingestor sleeping...");
                tokio::time::sleep(Duration::from_secs(10)).await;
                continue; // Re-evaluate the loop condition.
            }

            log::info!("Connecting to Yahoo WSS: {}", self.config.wss_url);
            
            // --- Phase 2: Connection Attempt ---
            match connect_async(&self.config.wss_url).await {
                Ok((ws_stream, _)) => {
                    log::info!("Successfully connected to Yahoo Streamer.");
                    let (_write, mut read) = ws_stream.split();
                    let mut last_activity = Instant::now();
                    
                    // --- Phase 3: Active Session Event Loop ---
                    loop {
                        tokio::select! {
                            // Branch 1: Handle incoming messages
                            msg = read.next() => {
                                match msg {
                                    Some(Ok(Message::Text(text))) => {
                                        last_activity = Instant::now();
                                        self.handle_message(text).await;
                                    }
                                    Some(Ok(Message::Binary(bin))) => {
                                        last_activity = Instant::now();
                                        self.handle_binary_protobuf(bin).await;
                                    }
                                    Some(Ok(Message::Ping(_))) | Some(Ok(Message::Pong(_))) => {
                                        // The server is alive, reset the inactivity timer.
                                        last_activity = Instant::now();
                                    }
                                    Some(Err(e)) => {
                                        log::error!("WSS Read Error: {}. Breaking for reconnect.", e);
                                        break;
                                    }
                                    None => {
                                        log::warn!("WSS Stream closed by remote host. Breaking for reconnect.");
                                        break;
                                    }
                                    _ => {} // Ignore other message types
                                }
                            }
                            // Branch 2: Run the watchdog timer
                            _ = tokio::time::sleep(Duration::from_secs(1)) => {
                                // Check for silent connection failure.
                                if last_activity.elapsed() > self.config.silent_failure_timeout {
                                    log::warn!("Inactivity timeout ({}s). Forcing reconnect...", self.config.silent_failure_timeout.as_secs());
                                    break; 
                                }

                                // Check if the market has closed while we were connected.
                                if self.manager.get_current_mode().await == OperationMode::Idle {
                                    log::info!("Market closed. Closing active WSS connection to enter idle state.");
                                    break;
                                }
                            }
                        }
                    }
                }
                Err(e) => {
                    log::error!("Failed to connect to Yahoo: {}. Retrying in 10s...", e);
                    tokio::time::sleep(Duration::from_secs(10)).await;
                }
            }
        }
    }

    /// # Handle Binary Protobuf Frame
    ///
    /// Processes the raw Protobuf data received from Yahoo Finance.
    ///
    /// **Note**: This is currently a MOCK implementation. A real implementation
    /// would use a Protobuf decoder (like `prost`) to deserialize the `data`
    /// byte vector into a structured Rust type representing the Yahoo quote.
    ///
    /// ## Current Logic:
    /// - Records an entry timestamp (`ts_in`).
    /// - Creates a hardcoded JSON object for "TSLA" to simulate normalization.
    /// - Broadcasts this normalized frame to the `Dispatcher`.
    async fn handle_binary_protobuf(&self, _data: Vec<u8>) {
        let ts_in = Instant::now();
        
        // TODO: Implement actual Protobuf decoding here.
        // For now, we simulate the output for a TSLA quote.
        let normalized = json!({
            "symbol": "TSLA",
            "price": 175.22,
            "source": "yahoo_wss",
            "ts_ingest": chrono::Utc::now().to_rfc3339()
        });

        self.dispatcher.broadcast(normalized, 0, ts_in).await;
    }

    /// # Handle Text Frame
    ///
    /// Processes non-binary messages, which are typically informational or metadata.
    /// In the Yahoo stream, these are rare, but it's good practice to log them.
    async fn handle_message(&self, text: String) {
        log::debug!("Received text frame: {}", text);
    }

    /// # Subscribe to Symbols
    ///
    /// Placeholder for the logic to send subscription messages to Yahoo.
    ///
    /// The Yahoo WSS stream requires sending a JSON message to subscribe to a list
    /// of ticker symbols. This function would construct and send that message
    /// over the WebSocket connection.
    pub async fn subscribe_symbol(&self, _symbols: Vec<String>) {
        // Example subscription message format:
        // let subscribe_msg = json!({
        //     "subscribe": _symbols
        // });
        // (This would be sent via the `write` half of the ws_stream)
    }
}