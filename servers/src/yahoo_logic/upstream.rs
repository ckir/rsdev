use crate::yahoo_logic::config::Config;
use crate::yahoo_logic::state::{AppState, UpstreamCommand};
use crate::yahoo_logic::yahoo_finance::PricingData;
use base64::{engine::general_purpose, Engine as _};
use futures_util::{
    stream::{SplitSink, SplitStream},
    SinkExt, StreamExt,
};
use prost::Message;
use serde_json::json;
use std::sync::Arc;
use std::time::Duration;
use tokio::net::TcpStream;
use tokio::sync::{broadcast, mpsc};
use tokio::time::sleep;
use tokio_tungstenite::{
    connect_async, tungstenite::protocol::Message as WsMessage,
    MaybeTlsStream, WebSocketStream,
};
use http::Uri;

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
                log::info!("Connected to Yahoo Finance");
                let (mut write, mut read): (
                    SplitSink<WebSocketStream<MaybeTlsStream<TcpStream>>, WsMessage>,
                    SplitStream<WebSocketStream<MaybeTlsStream<TcpStream>>>,
                ) = ws_stream.split();

                loop {
                    tokio::select! {
                        _ = shutdown.recv() => {
                            log::info!("Upstream shutting down...");
                            let _ = write.close().await;
                            return;
                        }
                        Some(cmd) = cmd_rx.recv() => {
                            log::info!("Received command for upstream: {:?}", cmd);
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