//! # Yahoo Finance Streaming Module
//!
//! This module provides a real-time WebSocket client for consuming market data
//! from the Yahoo Finance streaming service. It utilizes Protobuf for message 
//! decoding and features a built-in watchdog timer to detect silent stream failures.

use std::collections::HashSet;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::time::{timeout, sleep};

// Traits required for async WebSocket operations
use futures_util::sink::SinkExt;   // Provides .send()
use futures_util::stream::StreamExt; // Provides .next()

// Networking and Serialization
use tokio_tungstenite::{connect_async, tungstenite::protocol::Message as WsMessage};
use base64::{engine::general_purpose, Engine as _};
use serde::Deserialize;
use prost::Message; // Provides .decode()

/// Internal Protobuf handlers and models generated from PricingData.proto
pub mod proto_handler;
use proto_handler::{PricingData, QuoteType};

/// The primary URL for Yahoo Finance WebSocket version 2
const YAHOO_WS_URL: &str = "wss://streamer.finance.yahoo.com/?version=2";

/// Commands sent to the streaming module to manage active subscriptions.
#[derive(Debug, Clone)]
pub enum YahooCommand {
    /// Subscribe to a list of ticker symbols (e.g., "AAPL", "BTC-USD")
    Subscribe(Vec<String>),
    /// Remove a list of ticker symbols from the active stream
    Unsubscribe(Vec<String>),
}

/// JSON envelope structure returned by the Yahoo WebSocket
#[derive(Deserialize)]
struct YahooEnvelope {
    /// Base64 encoded Protobuf string
    pub message: String,
}

/// The core engine for managing the Yahoo Finance WebSocket connection.
/// 
/// It maintains a list of active tickers to automatically resubscribe upon
/// connection loss and implements a watchdog timer to trigger a reconnect
/// if no market data is received within the specified duration.
pub struct YahooStreamingModule {
    // Set of tickers currently being tracked
    active_tickers: HashSet<String>,
    // Receiver for external commands (Sub/Unsub)
    cmd_rx: mpsc::Receiver<YahooCommand>,
    // Sender to forward decoded pricing data to the ingestor
    data_tx: mpsc::Sender<PricingData>,
    // The duration after which a silent stream is considered failed
    watchdog_timeout: Duration,
}

impl YahooStreamingModule {
    /// Creates a new instance of the Yahoo Streaming Module.
    ///
    /// # Arguments
    /// * `cmd_rx` - Receiver for control commands.
    /// * `data_tx` - Sender for outbound market data.
    /// * `timeout_seconds` - Max seconds of silence before reconnecting.
    pub fn new(
        cmd_rx: mpsc::Receiver<YahooCommand>,
        data_tx: mpsc::Sender<PricingData>,
        timeout_seconds: u64,
    ) -> Self {
        Self {
            active_tickers: HashSet::new(),
            cmd_rx,
            data_tx,
            watchdog_timeout: Duration::from_secs(timeout_seconds),
        }
    }

    /// Starts the main execution loop.
    ///
    /// This function handles connection establishment, automatic retries with 
    /// exponential backoff, and the processing of incoming WebSocket messages.
    pub async fn run(mut self) {
        // Initial delay for exponential backoff
        let mut backoff = Duration::from_secs(1);
        
        loop {
            // Attempt to establish WebSocket connection
            match connect_async(YAHOO_WS_URL).await {
                Ok((mut ws_stream, _)) => {
                    // Connection successful; reset backoff
                    backoff = Duration::from_secs(1);
                    
                    // Resubscribe to existing tickers if this is a reconnection
                    if !self.active_tickers.is_empty() {
                        let subs: Vec<String> = self.active_tickers.iter().cloned().collect();
                        let msg = serde_json::json!({"subscribe": subs}).to_string();
                        // .into() converts String to Utf8Bytes for newer tungstenite versions
                        let _ = ws_stream.send(WsMessage::Text(msg.into())).await;
                    }

                    loop {
                        tokio::select! {
                            // Listen for incoming commands (Subscribe/Unsubscribe)
                            Some(cmd) = self.cmd_rx.recv() => {
                                match cmd {
                                    YahooCommand::Subscribe(tks) => {
                                        for t in &tks { self.active_tickers.insert(t.clone()); }
                                        let msg = serde_json::json!({"subscribe": tks}).to_string();
                                        let _ = ws_stream.send(WsMessage::Text(msg.into())).await;
                                    }
                                    YahooCommand::Unsubscribe(tks) => {
                                        for t in &tks { self.active_tickers.remove(t); }
                                        let msg = serde_json::json!({"unsubscribe": tks}).to_string();
                                        let _ = ws_stream.send(WsMessage::Text(msg.into())).await;
                                    }
                                }
                            }

                            // Watchdog logic: detect silent stream failures
                            msg_res = timeout(self.watchdog_timeout, ws_stream.next()) => {
                                match msg_res {
                                    // Received a message from the WebSocket
                                    Ok(Some(Ok(WsMessage::Text(text)))) => {
                                        // Parse the JSON envelope
                                        if let Ok(env) = serde_json::from_str::<YahooEnvelope>(&text) {
                                            // Decode the Base64 message field
                                            if let Ok(bin) = general_purpose::STANDARD.decode(env.message) {
                                                // Decode the binary data into the PricingData Protobuf struct
                                                if let Ok(pricing) = PricingData::decode(bin.as_slice()) {
                                                    // Only reset watchdog and forward data if it's not a Heartbeat
                                                    if pricing.quote_type != QuoteType::Heartbeat as i32 {
                                                        let _ = self.data_tx.send(pricing).await;
                                                    } else {
                                                        // Heartbeat received: continue loop without resetting timer
                                                        continue;
                                                    }
                                                }
                                            }
                                        }
                                    }
                                    // Socket was closed or encountered an error
                                    Ok(None) | Ok(Some(Err(_))) => break, 
                                    // Silent Failure: Watchdog triggered (no data received within timeout)
                                    Err(_) => {
                                        tracing::warn!("Watchdog triggered: No data for {}s. Reconnecting...", self.watchdog_timeout.as_secs());
                                        break;
                                    }
                                    _ => {}
                                }
                            }
                        }
                    }
                }
                Err(e) => {
                    // Log connection error and apply backoff
                    tracing::error!("Failed to connect to Yahoo: {}. Retrying in {}s...", e, backoff.as_secs());
                    sleep(backoff).await;
                    // Cap backoff at 60 seconds
                    backoff = std::cmp::min(backoff * 2, Duration::from_secs(60));
                }
            }
        }
    }
}