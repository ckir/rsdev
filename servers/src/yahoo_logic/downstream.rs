use axum_server::tls_rustls::RustlsConfig;
// use std::path::PathBuf;
use crate::yahoo_logic::config::Config;
use crate::yahoo_logic::model::{ClientMessage, ServerMessage};
use crate::yahoo_logic::state::AppState;
use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        State,
    },
    response::IntoResponse,
    routing::get,
    Router,
};
use futures_util::{StreamExt};
use std::net::SocketAddr;
use std::sync::atomic::{AtomicUsize, Ordering};
use tokio::sync::broadcast;

static NEXT_CLIENT_ID: AtomicUsize = AtomicUsize::new(1);

pub async fn run(
    config: Config,
    app_state: AppState,
    mut shutdown: broadcast::Receiver<()>,
) {
    let app = Router::new()
        .route("/ws", get(ws_handler))
        .route("/health", get(health_handler))
        .with_state(app_state);

    let addr = SocketAddr::from(([0, 0, 0, 0], config.port));
    log::info!("Downstream server listening on {}", addr);

    if let (Some(cert_path), Some(key_path)) = (config.tls_cert_path, config.tls_key_path) {
        let tls_config = RustlsConfig::from_pem_file(cert_path, key_path)
            .await
            .expect("Failed to load TLS configuration");

        axum_server::bind_rustls(addr, tls_config)
            .serve(app.into_make_service())
            .await
            .unwrap();
    } else {
        let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
        axum::serve(listener, app)
            .with_graceful_shutdown(async move {
                shutdown.recv().await.ok();
                log::info!("Downstream server shutting down.");
            })
            .await
            .unwrap();
    }
}

async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
) -> impl IntoResponse {
    ws.on_upgrade(|socket| handle_socket(socket, state))
}

async fn health_handler() -> impl IntoResponse {
    (axum::http::StatusCode::OK, "OK")
}

async fn handle_socket(mut socket: WebSocket, state: AppState) {
    let client_id = NEXT_CLIENT_ID.fetch_add(1, Ordering::Relaxed);
    state.add_client(client_id).await;
    log::info!("Client {} connected", client_id);

    let mut data_rx = state.data_tx.subscribe();

    loop {
        tokio::select! {
            // Handle incoming messages from the client
            Some(msg) = socket.next() => {
                if let Ok(msg) = msg {
                    match msg {
                        Message::Text(text) => {
                            log::debug!("Received message from client {}: {}", client_id, text);
                            if let Ok(client_msg) = serde_json::from_str::<ClientMessage>(&text) {
                                // Since we await, we handle one command at a time and need to respond within the same block
                                let (result, action, symbols) = if let Some(symbols) = client_msg.subscribe {
                                    (state.subscribe(client_id, symbols.clone()).await, "subscribe", symbols)
                                } else if let Some(symbols) = client_msg.unsubscribe {
                                    (state.unsubscribe(client_id, symbols.clone()).await, "unsubscribe", symbols)
                                } else {
                                    continue; // Or handle empty message
                                };

                                let ack_msg = match result {
                                    Ok(_) => ServerMessage {
                                        r#type: "ack".to_string(),
                                        message: Some(serde_json::Value::String(format!("Successfully processed {} for {:?}", action, symbols))),
                                        error: None,
                                        ack: Some(true),
                                    },
                                    Err(e) => ServerMessage {
                                        r#type: "ack".to_string(),
                                        message: Some(serde_json::Value::String(format!("Failed to process {} for {:?}: {}", action, symbols, e))),
                                        error: Some(e),
                                        ack: Some(false),
                                    },
                                };

                                if let Ok(json_str) = serde_json::to_string(&ack_msg) {
                                    if socket.send(Message::Text(json_str.into())).await.is_err() {
                                        break; // client disconnected
                                    }
                                }
                            } else {
                                log::warn!("Failed to parse message from client {}: {}", client_id, text);
                                let err_msg = ServerMessage {
                                    r#type: "ack".to_string(),
                                    message: Some(serde_json::Value::String("Failed to parse message.".to_string())),
                                    error: Some("Invalid JSON format.".to_string()),
                                    ack: Some(false),
                                };
                                if let Ok(json_str) = serde_json::to_string(&err_msg) {
                                    if socket.send(Message::Text(json_str.into())).await.is_err() {
                                        break; // client disconnected
                                    }
                                }
                            }
                        }
                        Message::Close(_) => {
                            break;
                        }
                        _ => {}
                    }
                } else {
                    // client disconnected
                    break;
                }
            }
            // Handle broadcasted data from the upstream
            Ok(pricing_data) = data_rx.recv() => {
                log::debug!("Received pricing data to broadcast: {:?}", pricing_data);
                if state.is_subscribed(client_id, &pricing_data.id).await {
                    match serde_json::to_value(&*pricing_data) {
                        Ok(json_val) => {
                            let server_msg = ServerMessage {
                                r#type: "pricing".to_string(),
                                message: Some(json_val),
                                error: None,
                                ack: None,
                            };
                            if let Ok(json_str) = serde_json::to_string(&server_msg) {
                                if socket.send(Message::Text(json_str.into())).await.is_err() {
                                    break; // client disconnected
                                }
                            }
                        }
                        Err(e) => {
                            log::error!("Failed to serialize pricing data to JSON: {}", e);
                        }
                    }
                }
            }
        }
    }

    state.remove_client(client_id).await;
    log::info!("Client {} disconnected", client_id);
}
