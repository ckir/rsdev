use crate::yahoo_logic::config::Config;
use crate::yahoo_logic::state::{AppState, UpstreamCommand};
use crate::yahoo_logic::yahoo_finance::PricingData;
use futures_util::{SinkExt, StreamExt};
use prost::Message;
use serde_json::json;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{broadcast, mpsc};
use tokio::time::sleep;
use tokio_tungstenite::{connect_async, tungstenite::protocol::Message as WsMessage};
use base64::{Engine as _, engine::general_purpose};

pub async fn run(
    config: Config,
    app_state: AppState,
    mut shutdown: broadcast::Receiver<()>,
) {
    let (cmd_tx, mut cmd_rx) = mpsc::unbounded_channel::<UpstreamCommand>();
    app_state.set_upstream_tx(cmd_tx);

    loop {
        if shutdown.try_recv().is_ok() {
            break;
        }

        log::info!("Connecting to Yahoo Finance: {}", config.yahoo_ws_url);
        match connect_async(&config.yahoo_ws_url).await {
            Ok((ws_stream, _)) => {
                log::info!("Connected to Yahoo Finance");
                let (mut write, mut read) = ws_stream.split();

                loop {
                    tokio::select! {
                        _ = shutdown.recv() => {
                            log::info!("Upstream shutting down...");
                            let _ = write.close().await;
                            return;
                        }
                        Some(cmd) = cmd_rx.recv() => {
                            match cmd {
                                UpstreamCommand::Subscribe(symbols) => {
                                    let msg = json!({ "subscribe": symbols }).to_string();
                                    log::debug!("Sending upstream: {}", msg);
                                    if let Err(e) = write.send(WsMessage::Text(msg.into())).await {
                                        log::error!("Failed to send subscribe: {}", e);
                                        break; // Reconnect
                                    }
                                }
                                UpstreamCommand::Unsubscribe(symbols) => {
                                    let msg = json!({ "unsubscribe": symbols }).to_string();
                                    log::debug!("Sending upstream: {}", msg);
                                    if let Err(e) = write.send(WsMessage::Text(msg.into())).await {
                                        log::error!("Failed to send unsubscribe: {}", e);
                                        break; // Reconnect
                                    }
                                }
                            }
                        }
                        Some(msg) = read.next() => {
                            match msg {
                                Ok(WsMessage::Text(text)) => {
                                    if let Ok(json_val) = serde_json::from_str::<serde_json::Value>(&text) {
                                        if let Some(b64_msg) = json_val.get("message").and_then(|v| v.as_str()) {
                                            if let Ok(decoded) = general_purpose::STANDARD.decode(b64_msg) {
                                                if let Ok(pricing) = PricingData::decode(&decoded[..]) {
                                                    // Broadcast to downstream
                                                    let _ = app_state.data_tx.send(Arc::new(pricing));
                                                }
                                            }
                                        }
                                    }
                                }
                                Ok(WsMessage::Ping(_)) => {}
                                Err(e) => {
                                    log::error!("Upstream error: {}", e);
                                    break;
                                }
                                _ => {}
                            }
                        }
                    }
                }
            }
            Err(e) => {
                log::error!("Failed to connect to Yahoo: {}", e);
                sleep(Duration::from_secs(5)).await;
            }
        }
    }
}
