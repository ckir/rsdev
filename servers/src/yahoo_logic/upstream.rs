use crate::yahoo_logic::config::Config;
use crate::yahoo_logic::state::{AppState, Notification, UpstreamCommand, UpstreamRequest};
use crate::yahoo_logic::pricing_data::PricingData;
use base64::{engine::general_purpose, Engine as _};
use futures_util::{SinkExt, StreamExt, stream::{SplitSink, SplitStream}};
use prost::Message;
use serde_json::json;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{broadcast, mpsc};
use tokio::time::{interval, sleep, Instant};
use tokio_tungstenite::{
    connect_async, WebSocketStream, tungstenite::protocol::Message as WsMessage,
};
use http::Uri;

type UpstreamSink = SplitSink<WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>, WsMessage>;
type UpstreamStream = SplitStream<WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>>;

pub async fn run(
    config: Config,
    app_state: AppState,
    mut shutdown: broadcast::Receiver<()>,
) {
    let (cmd_tx, mut cmd_rx) = mpsc::unbounded_channel::<UpstreamRequest>();
    app_state.set_upstream_tx(cmd_tx).await;

    let mut reconnect_attempts = 0;
    let mut current_tx_sink: Option<UpstreamSink> = None;

    loop {
        if shutdown.try_recv().is_ok() {
            log::info!("Upstream service received shutdown signal.");
            if let Some(mut sink) = current_tx_sink.take() {
                let _ = sink.close().await;
            }
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
            app_state.notify_clients(Notification::UpstreamDisconnected);
        }

        match connect_to_yahoo(&config).await {
            Ok(ws_stream) => {
                log::info!("Successfully connected to Yahoo Finance.");
                app_state.notify_clients(Notification::UpstreamReconnected);
                reconnect_attempts = 0; // Reset on successful connection
                let (mut write, mut read) = ws_stream.split();
                current_tx_sink = Some(write);

                // Attempt to resubscribe all active symbols
                let active_symbols = app_state.get_all_subscribed_symbols().await;
                if !active_symbols.is_empty() {
                    log::info!("Resubscribing to {} active symbols.", active_symbols.len());
                    if let Err(e) = send_upstream_command(&mut current_tx_sink, UpstreamCommand::Subscribe(active_symbols)).await {
                        log::error!("Failed to resubscribe to active symbols: {}", e);
                        app_state.notify_clients(Notification::Error(format!("Failed to resubscribe: {}", e)));
                        // If resubscription fails, we should probably force a reconnect
                        current_tx_sink = None; // Invalidate sink to trigger reconnect
                        continue; // Skip to next loop iteration for reconnect
                    } else {
                        app_state.notify_clients(Notification::UpstreamResubscribed);
                    }
                }

                let mut last_message_time = Instant::now();
                let mut heartbeat_interval = interval(Duration::from_secs(1));

                loop {
                    tokio::select! {
                        _ = shutdown.recv() => {
                            log::info!("Upstream shutting down...");
                            if let Some(mut sink) = current_tx_sink.take() {
                                let _ = sink.close().await;
                            }
                            return;
                        }
                        Some(req) = cmd_rx.recv() => {
                            let UpstreamRequest { command, responder } = req;
                            log::info!("Received command for upstream: {:?}", command);

                            let ack_result = match send_upstream_command(&mut current_tx_sink, command).await {
                                Ok(_) => Ok(()),
                                Err(e) => {
                                    log::error!("Failed to send command to Yahoo: {}", e);
                                    Err(e)
                                }
                            };
                            
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
                                        app_state.update_last_data_timestamp(); // Update timestamp on data reception
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
                                                    app_state.update_last_data_timestamp(); // Update timestamp on data reception
                                                    let _ = app_state.data_tx.send(Arc::new(pricing));
                                                }
                                            }
                                        }
                                    }
                                }
                                Ok(WsMessage::Ping(_)) => {
                                    log::trace!("Received Ping from Yahoo");
                                }
                                Ok(WsMessage::Pong(_)) => {
                                    log::trace!("Received Pong from Yahoo");
                                }
                                Err(e) => {
                                    log::error!("Upstream error: {}", e);
                                    // Invalidate sink to trigger reconnect
                                    current_tx_sink = None;
                                    break;
                                }
                                _ => {}
                            }
                        }
                        _ = heartbeat_interval.tick() => {
                            if last_message_time.elapsed().as_secs() > config.heartbeat_threshold_seconds {
                                log::warn!("Heartbeat lost. No message received for over {} seconds. Reconnecting...", config.heartbeat_threshold_seconds);
                                current_tx_sink = None; // Invalidate sink to trigger reconnect
                                break; // Trigger reconnect
                            }
                        }
                    }
                }
            }
            Err(e) => {
                log::error!("Failed to connect to Yahoo: {}", e);
                reconnect_attempts += 1;
                app_state.notify_clients(Notification::Error(format!("Upstream connection failed: {}", e)));
            }
        }
    }
}

async fn connect_to_yahoo(config: &Config) -> anyhow::Result<WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>> {
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
        .body(())?;

    let (ws_stream, _) = connect_async(request).await?;
    Ok(ws_stream)
}

async fn send_upstream_command(sink: &mut Option<UpstreamSink>, command: UpstreamCommand) -> Result<(), String> {
    let msg = match command {
        UpstreamCommand::Subscribe(symbols) => json!({ "subscribe": symbols }).to_string(),
        UpstreamCommand::Unsubscribe(symbols) => json!({ "unsubscribe": symbols }).to_string(),
    };
    log::debug!("Sending upstream: {}", msg);

    if let Some(s) = sink {
        s.send(WsMessage::Text(msg.into()))
            .await
            .map_err(|e| format!("Failed to send message to Yahoo: {}", e))
    } else {
        Err("Upstream sink not available.".to_string())
    }
}