use std::sync::Arc;
use std::net::SocketAddr;
use tokio::signal;
use chrono::{Utc, Timelike};
use chrono_tz::US::Eastern;
use serde_json::Value;

// Web Layer (Axum + TLS)
use axum::{
    routing::get,
    extract::{State, WebSocketUpgrade, ws::{WebSocket, Message}, ConnectInfo},
    response::IntoResponse,
    Router,
};
use axum_server::tls_rustls::RustlsConfig;

// CORS Middleware
use tower_http::cors::{Any, CorsLayer};

// Internal Library Imports
use lib_common::config_cloud::get_cloud_config;
use lib_common::core::{
    dispatcher::Dispatcher, 
    registry::Registry, 
    upstream_manager::UpstreamManager,
    memory_guard::{GlobalMemoryGuard, ClientPriority},
};
use lib_common::ingestors::{YahooWssIngestor, YahooConfig};
use lib_common::markets::nasdaq::marketstatus::MarketStatus;
use lib_common::markets::nasdaq::apicall::ApiCall;
use lib_common::loggers::loggerlocal::LoggerLocal;

/// State shared across all HTTP/WS routes
struct AppState {
    dispatcher: Arc<Dispatcher>,
    _manager: Arc<UpstreamManager>,
    _logger: Arc<LoggerLocal>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // 0. Initialize the CryptoProvider (Required for rustls 0.23+)
    // This installs 'ring' as the default provider for the entire process.
    rustls::crypto::ring::default_provider()
        .install_default()
        .map_err(|_| anyhow::anyhow!("Failed to install rustls crypto provider"))?;

    // 1. Fetch Cloud Config
    let cloud_json: Value = get_cloud_config()
        .map_err(|e| anyhow::anyhow!("Cloud Config Error: {}", e))?;
    let restream_cfg = &cloud_json["restream"];

    // 2. Extract variables
    let app_name = restream_cfg["appName"].as_str().unwrap_or("restream").to_string();
    let server_port = restream_cfg["serverPort"].as_u64().unwrap_or(8080) as u16;
    let max_mem = restream_cfg["maxMemoryBytes"].as_u64().unwrap_or(1024 * 1024 * 1024);
    let reg_ttl = restream_cfg["registryTtl"].as_u64().unwrap_or(30);

    // 3. Setup Logger
    let logger = Arc::new(LoggerLocal::new(app_name, None));
    let startup_ny = Utc::now().with_timezone(&Eastern).format("%Y-%m-%d %H:%M:%S EST");
    logger.info(&format!("ReStream Secure Gateway booting. NY Time: {}", startup_ny), None).await;

    // 4. Infrastructure Initialization
    let memory_guard = Arc::new(GlobalMemoryGuard::new(max_mem));
    let dispatcher = Arc::new(Dispatcher::new(memory_guard));
    let registry = Arc::new(Registry::new(reg_ttl)); 
    let api_call = Arc::new(ApiCall::new(logger.clone()));
    
    // 5. Upstream Manager
    let market_status = Arc::new(MarketStatus::new(api_call, logger.clone()));
    let manager = Arc::new(UpstreamManager::new(registry.clone(), dispatcher.clone(), market_status.clone()));

    // 6. Background Tasks
    spawn_watchdog(logger.clone());
    let m_clone = manager.clone();
    tokio::spawn(async move { m_clone.run().await });

    let yahoo_ingestor = YahooWssIngestor::new(YahooConfig::default(), dispatcher.clone(), manager.clone());
    tokio::spawn(async move { yahoo_ingestor.run().await });

    // 7. TLS Configuration from $HOME/.letsencrypt
    let home_dir = dirs::home_dir().ok_or_else(|| anyhow::anyhow!("HOME directory not found"))?;
    let cert_path = home_dir.join(".letsencrypt").join("fullchain.pem");
    let key_path = home_dir.join(".letsencrypt").join("privkey.pem");

    logger.info(&format!("Loading TLS certs from: {}", cert_path.display()), None).await;

    let tls_config = RustlsConfig::from_pem_file(cert_path, key_path)
        .await
        .map_err(|e| anyhow::anyhow!("TLS Configuration Error: {}", e))?;

    // 8. Build Router
    let cors = CorsLayer::new().allow_origin(Any).allow_methods(Any).allow_headers(Any);
    let shared_state = Arc::new(AppState {
        dispatcher: dispatcher.clone(),
        _manager: manager.clone(),
        _logger: logger.clone(),
    });

    let app = Router::new()
        .route("/health", get(health_handler))
        .route("/ws", get(ws_handler))
        .layer(cors)
        .with_state(shared_state);

    // 9. Bind Secure Server with Signal Handling
    let addr = SocketAddr::from(([0, 0, 0, 0], server_port));
    logger.info(&format!("WSS Secure Gateway live at https://{}", addr), None).await;

    let handle = axum_server::Handle::new();
    
    // Graceful Shutdown Task
    let signal_handle = handle.clone();
    let signal_logger = logger.clone();
    tokio::spawn(async move {
        shutdown_signal().await;
        signal_logger.warn("Shutdown signal received. Closing server gracefully...", None).await;
        signal_handle.graceful_shutdown(Some(std::time::Duration::from_secs(10)));
    });

    axum_server::bind_rustls(addr, tls_config)
        .handle(handle)
        .serve(app.into_make_service_with_connect_info::<SocketAddr>())
        .await?;

    Ok(())
}

async fn health_handler() -> &'static str { "OK" }

async fn ws_handler(
    ws: WebSocketUpgrade,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, state, addr))
}

async fn handle_socket(mut socket: WebSocket, state: Arc<AppState>, addr: SocketAddr) {
    let client_id = format!("ws-{}", addr);
    let mut rx = state.dispatcher.add_client(&client_id, ClientPriority::Low);
    
    while let Some(frame) = rx.recv().await {
        let payload_str = frame.payload.to_string();
        // Axum 0.8 requires .into() for Utf8Bytes
        if socket.send(Message::Text(payload_str.into())).await.is_err() {
            break; 
        }
    }
    state.dispatcher.remove_client(&client_id);
}

fn spawn_watchdog(logger: Arc<LoggerLocal>) {
    tokio::spawn(async move {
        loop {
            let now_ny = Utc::now().with_timezone(&Eastern);
            if now_ny.hour() == 0 && now_ny.minute() == 0 {
                logger.warn("Scheduled NY Midnight restart.", None).await;
                std::process::exit(0);
            }
            tokio::time::sleep(std::time::Duration::from_secs(30)).await;
        }
    });
}

async fn shutdown_signal() {
    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
}