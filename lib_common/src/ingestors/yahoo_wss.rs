//! # Yahoo WSS Ingestor
//! 
//! High-performance WebSocket ingestor for Yahoo Finance Protobuf streams.
//! Location: lib_common/src/ingestors/yahoo_wss.rs

use std::sync::Arc;
use std::time::{Duration, Instant};
use futures_util::StreamExt;
use tokio_tungstenite::{connect_async, tungstenite::protocol::Message};
use serde_json::json;

use crate::core::dispatcher::Dispatcher;
use crate::core::upstream_manager::{UpstreamManager, OperationMode};

/// Configuration for the Yahoo WebSocket stream.
pub struct YahooConfig {
    pub wss_url: String,
    pub heartbeat_interval: Duration,
    pub silent_failure_timeout: Duration,
}

impl Default for YahooConfig {
    fn default() -> Self {
        Self {
            wss_url: "wss://streamer.finance.yahoo.com".to_string(),
            heartbeat_interval: Duration::from_secs(30),
            silent_failure_timeout: Duration::from_secs(20), // Increased slightly for stability
        }
    }
}

pub struct YahooWssIngestor {
    config: YahooConfig,
    dispatcher: Arc<Dispatcher>,
    manager: Arc<UpstreamManager>,
}

impl YahooWssIngestor {
    /// Creates a new Yahoo Ingestor instance.
    pub fn new(config: YahooConfig, dispatcher: Arc<Dispatcher>, manager: Arc<UpstreamManager>) -> Self {
        Self { 
            config, 
            dispatcher, 
            manager 
        }
    }

    /// Primary execution loop with reconnection logic.
    pub async fn run(&self) {
        loop {
            // 1. Check Operational Mode before attempting connection
            let current_mode = self.manager.get_current_mode().await;
            
            if current_mode == OperationMode::Idle {
                log::debug!("System is IDLE. Yahoo WSS ingestor sleeping...");
                tokio::time::sleep(Duration::from_secs(10)).await;
                continue;
            }

            log::info!("Connecting to Yahoo WSS: {}", self.config.wss_url);
            
            match connect_async(&self.config.wss_url).await {
                Ok((ws_stream, _)) => {
                    log::info!("Successfully connected to Yahoo Streamer.");
                    let (_write, mut read) = ws_stream.split();
                    let mut last_activity = Instant::now();
                    
                    loop {
                        tokio::select! {
                            msg = read.next() => {
                                match msg {
                                    Some(Ok(Message::Text(text))) => {
                                        last_activity = Instant::now();
                                        self.handle_message(text.to_string()).await;
                                    }
                                    Some(Ok(Message::Binary(bin))) => {
                                        last_activity = Instant::now();
                                        self.handle_binary_protobuf(bin.to_vec()).await;
                                    }
                                    Some(Ok(Message::Ping(_))) | Some(Ok(Message::Pong(_))) => {
                                        // Update activity on heartbeats so we don't reconnect during low volume
                                        last_activity = Instant::now();
                                    }
                                    Some(Err(e)) => {
                                        log::error!("WSS Read Error: {}", e);
                                        break;
                                    }
                                    None => {
                                        log::warn!("WSS Stream closed by remote host.");
                                        break;
                                    }
                                    _ => {}
                                }
                            }
                            // 2. Watchdog: Detects "Zombie" connections or Market Close transitions
                            _ = tokio::time::sleep(Duration::from_secs(1)) => {
                                // Break if we've been silent too long
                                if last_activity.elapsed() > self.config.silent_failure_timeout {
                                    log::warn!("Inactivity timeout ({}s). Reconnecting...", self.config.silent_failure_timeout.as_secs());
                                    break; 
                                }

                                // Break if UpstreamManager switched to Idle while we were connected
                                if self.manager.get_current_mode().await == OperationMode::Idle {
                                    log::info!("Market closed. Closing active WSS connection.");
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

    /// Handles binary frames (Protobuf encoded quotes).
    async fn handle_binary_protobuf(&self, _data: Vec<u8>) {
        let ts_in = Instant::now();
        
        // MOCK LOGIC: TSLA quote for demonstration
        let normalized = json!({
            "symbol": "TSLA",
            "price": 175.22,
            "source": "yahoo_wss",
            "ts_ingest": chrono::Utc::now().to_rfc3339()
        });

        self.dispatcher.broadcast(normalized, 0, ts_in).await;
    }

    /// Handles text-based frames.
    async fn handle_message(&self, text: String) {
        log::debug!("Received text frame: {}", text);
    }

    /// Place-holder for subscription logic.
    pub async fn subscribe_symbol(&self, _symbols: Vec<String>) {
        // Implementation for sending subscription JSON to Yahoo
    }
}