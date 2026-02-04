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
use base64::{Engine as _, engine::general_purpose};
use prost::Message as ProstMessage;

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
    state.add_client(client_id);
    log::info!("Client {} connected", client_id);

    let mut data_rx = state.data_tx.subscribe();

    loop {
        tokio::select! {
            // Handle incoming messages from the client
            Some(msg) = socket.next() => {
                if let Ok(msg) = msg {
                    match msg {
                        Message::Text(text) => {
                            if let Ok(client_msg) = serde_json::from_str::<ClientMessage>(&text) {
                                if let Some(symbols) = client_msg.subscribe {
                                    state.subscribe(client_id, symbols);
                                }
                                if let Some(symbols) = client_msg.unsubscribe {
                                    state.unsubscribe(client_id, symbols);
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
                if state.is_subscribed(client_id, &pricing_data.id) {
                    let mut buf = Vec::new();
                    if pricing_data.encode(&mut buf).is_ok() {
                        let b64_msg = general_purpose::STANDARD.encode(&buf);
                        let server_msg = ServerMessage {
                            r#type: "pricing".to_string(),
                            message: Some(b64_msg),
                            error: None,
                        };
                        if let Ok(json_str) = serde_json::to_string(&server_msg) {
                            if socket.send(Message::Text(json_str.into())).await.is_err() {
                                break; // client disconnected
                            }
                        }
                    }
                }
            }
        }
    }

    state.remove_client(client_id);
    log::info!("Client {} disconnected", client_id);
}
