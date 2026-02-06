use crate::yahoo_logic::config::Config;
use crate::yahoo_logic::state::{AppState, UpstreamCommand, UpstreamRequest};
use crate::yahoo_logic::pricing_data::PricingData;
use base64::{engine::general_purpose, Engine as _};
use futures_util::{
    SinkExt, StreamExt,
};
use prost::Message;
use serde_json::json;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{broadcast, mpsc};
use tokio::time::{interval, sleep, Instant};
use tokio_tungstenite::{
    connect_async, tungstenite::protocol::Message as WsMessage,
};
use http::Uri;

pub async fn run(
    config: Config,
    app_state: AppState,
    mut shutdown: broadcast::Receiver<()>,
) {
    let (cmd_tx, mut cmd_rx) = mpsc::unbounded_channel::<UpstreamRequest>();
    app_state.set_upstream_tx(cmd_tx).await;

    let mut reconnect_attempts = 0;

    loop {
        if shutdown.try_recv().is_ok() {
            break;
        }

        if reconnect_attempts > 0 {
            let delay = std::cmp::min(
                config.reconnect_max_delay_ms,
                config.reconnect_base_delay_ms * 2_u32.pow(reconnect_attempts - 1) as u64,
            );
            log::warn!(
                "Reconnecting to Yahoo Finance in {}ms (attempt {})...",
                delay,
                reconnect_attempts
            );
            sleep(Duration::from_millis(delay)).await;
        }

        log::info!("Connecting to Yahoo Finance: {}", config.yahoo_ws_url);

        let uri = config.yahoo_ws_url.parse::<Uri>().unwrap();

        let request = http::Request::builder()
            .method("GET")
            .uri(uri.clone())
            .header("Host", uri.host().unwrap())
            .header("Connection", "Upgrade")
            .header("Upgrade", "websocket")
            .header("Sec-WebSocket-Version", "13")
            .header("Sec-WebSocket-Key", "dGhlIHNhbXBsZSBub25jZQ==") // Base64 encoded nonce, can be anything
            .header("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:126.0) Gecko/20100101 Firefox/126.0")
            .body(())
            .unwrap();

        match connect_async(request).await {
            Ok((ws_stream, _)) => {
                log::info!("Successfully connected to Yahoo Finance.");
                reconnect_attempts = 0; // Reset on successful connection
                let (mut write, mut read) = ws_stream.split();

                let mut last_message_time = Instant::now();
                let mut heartbeat_interval = interval(Duration::from_secs(1));

                loop {
                    tokio::select! {
                        _ = shutdown.recv() => {
                            log::info!("Upstream shutting down...");
                            let _ = write.close().await;
                            return;
                        }
                        Some(req) = cmd_rx.recv() => {
                            let UpstreamRequest { command, responder } = req;
                            log::info!("Received command for upstream: {:?}", command);

                            let result = match command {
                                UpstreamCommand::Subscribe(symbols) => {
                                    let msg = json!({ "subscribe": symbols }).to_string();
                                    log::debug!("Sending upstream: {}", msg);
                                    write.send(WsMessage::Text(msg.into())).await
                                }
                                UpstreamCommand::Unsubscribe(symbols) => {
                                    let msg = json!({ "unsubscribe": symbols }).to_string();
                                    log::debug!("Sending upstream: {}", msg);
                                    write.send(WsMessage::Text(msg.into())).await
                                }
                            };

                            let ack_result = match result {
                                Ok(_) => Ok(()),
                                Err(e) => {
                                    log::error!("Failed to send command to Yahoo: {}", e);
                                    // Breaking here will trigger a reconnect, which is often what we want if we can't send.
                                    break;
                                }
                            };
                            
                            // Send the acknowledgement. If it fails, the client is likely gone, so we just log it.
                            if let Err(_) = responder.send(ack_result) {
                                log::warn!("Failed to send acknowledgement to downstream client. It may have disconnected.");
                            }
                        }
                        Some(msg) = read.next() => {
                            last_message_time = Instant::now(); // Reset timer on any message
                            match msg {
                                Ok(WsMessage::Binary(data)) => {
                                    log::trace!("Received binary message from Yahoo: {} bytes", data.len());
                                    if let Ok(pricing) = PricingData::decode(&data[..]) {
                                        log::debug!("Decoded pricing data: {:?}", pricing);
                                        // Broadcast to downstream
                                        if let Err(e) = app_state.data_tx.send(Arc::new(pricing)) {
                                            log::error!("Failed to broadcast pricing data: {}", e);
                                        }
                                    } else {
                                        log::warn!("Failed to decode binary message from Yahoo");
                                    }
                                }
                                Ok(WsMessage::Text(text)) => {
                                    log::trace!("Received text message from Yahoo: {}", text);
                                    if let Ok(json_val) = serde_json::from_str::<serde_json::Value>(&text) {
                                        if let Some(b64_msg) = json_val.get("message").and_then(|v| v.as_str()) {
                                            if let Ok(decoded) = general_purpose::STANDARD.decode(b64_msg) {
                                                if let Ok(pricing) = PricingData::decode(&decoded[..]) {
                                                    log::debug!("Decoded pricing data from text: {:?}", pricing);
                                                    // Broadcast to downstream
                                                    let _ = app_state.data_tx.send(Arc::new(pricing));
                                                }
                                            }
                                        }
                                    }
                                }
                                Ok(WsMessage::Ping(_)) => {
                                    log::trace!("Received Ping from Yahoo");
                                }
                                Err(e) => {
                                    log::error!("Upstream error: {}", e);
                                    break;
                                }
                                _ => {}
                            }
                        }
                        _ = heartbeat_interval.tick() => {
                            if last_message_time.elapsed().as_secs() > config.heartbeat_threshold_seconds {
                                log::warn!("Heartbeat lost. No message received for over {} seconds. Reconnecting...", config.heartbeat_threshold_seconds);
                                break; // Trigger reconnect
                            }
                        }
                    }
                }
            }
            Err(e) => {
                log::error!("Failed to connect to Yahoo: {}", e);
                reconnect_attempts += 1;
            }
        }
    }
}