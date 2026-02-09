//! # ReStream Secure Gateway
//!
//! The primary production server for the `rsdev` project. This binary launches a secure WebSocket
//! (WSS) server that acts as the main public-facing gateway for real-time market data.
//!
//! ## Core Responsibilities:
//! - **Secure WebSocket (WSS) Termination:** Uses Axum and Rustls to handle TLS, providing a secure
//!   endpoint for clients.
//! - **Client Session Management:** Manages WebSocket connections, subscribing them to the central
//!   `Dispatcher`.
//! - **Upstream Ingestion:** Initializes and runs ingestors (e.g., `YahooWssIngestor`) to receive
//!   live data streams.
//! - **System Health & Lifecycle:** Includes a `/health` check endpoint, graceful shutdown logic,
//!   and a scheduled daily restart to ensure long-term stability.
//! - **Configuration:** Fetches settings from a centralized cloud configuration service on startup.
//!
//! This server integrates all core components from `lib_common` to form a cohesive, high-performance
//! data distribution hub.

#![doc(html_logo_url = "https://example.com/logo.png")] // Example, replace with actual logo URL if available
#![forbid(unsafe_code)]
#![warn(missing_docs, rust_2018_idioms, unused_qualifications)]

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

/// # Application State
///
/// Holds all shared state required by the web server's routes.
///
/// This struct is wrapped in an `Arc` to allow for safe, concurrent access from
/// multiple asynchronous tasks and web handlers. It contains the core components
/// of the application's infrastructure.
struct AppState {
    /// The central message broadcaster. It manages client subscriptions and
    /// distributes incoming data frames to all connected WebSocket clients.
    dispatcher: Arc<Dispatcher>,
    /// Manages the lifecycle of upstream data sources, such as market data ingestors.
    /// It ensures that data sources are running and healthy.
    _manager: Arc<UpstreamManager>,
    /// The application-wide logger instance, used for structured logging.
    _logger: Arc<LoggerLocal>,
}

/// # Main Entry Point
///
/// Initializes and runs the secure WebSocket gateway.
///
/// ## Execution Flow:
/// 1.  **Initialize Crypto Provider**: Sets up `ring` for Rustls, a hard requirement for TLS.
/// 2.  **Fetch Configuration**: Loads server settings from the cloud config service.
/// 3.  **Setup Logger**: Initializes the `LoggerLocal` for application-wide logging.
/// 4.  **Instantiate Core Services**: Sets up the `GlobalMemoryGuard`, `Dispatcher`, `Registry`,
///     and `ApiCall` modules.
/// 5.  **Start Upstream Manager**: Spawns the `UpstreamManager` to manage data ingestors.
/// 6.  **Launch Background Tasks**:
///     -   Starts the `spawn_watchdog` for scheduled daily restarts.
///     -   Initializes and runs the `YahooWssIngestor`.
/// 7.  **Configure TLS**: Loads the TLS certificate and private key from the filesystem.
/// 8.  **Build and Run Web Server**:
///     -   Constructs the `Axum` router with handlers for `/health` and `/ws`.
///     -   Applies CORS middleware to allow cross-origin requests.
///     -   Binds the server to the configured port and serves traffic over WSS.
/// 9.  **Enable Graceful Shutdown**: Sets up signal handlers for `CTRL+C` and `terminate` to
///     allow the server to shut down cleanly.
///
/// # Panics
///
/// This function will panic if:
/// - The cloud configuration cannot be fetched.
/// - The TLS certificate or key cannot be loaded.
/// - The `HOME` directory cannot be found (for locating TLS certs).
/// - The server fails to bind to the specified address.
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // --- Phase 0: Crypto Initialization ---
    // Rustls 0.23+ requires an explicit crypto provider to be installed.
    // We use 'ring' as it's a robust, widely-used default. This must be
    // done once at the start of the process.
    rustls::crypto::ring::default_provider()
        .install_default()
        .map_err(|_| anyhow::anyhow!("Failed to install rustls crypto provider"))?;

    // --- Phase 1: Configuration Loading ---
    // Fetch critical runtime parameters from a remote source. This decouples
    // configuration from the binary, allowing for dynamic adjustments without
    // a full redeployment.
    let cloud_json: Value = get_cloud_config()
        .map_err(|e| anyhow::anyhow!("Cloud Config Error: {}", e))?;
    let restream_cfg = &cloud_json["restream"];

    // --- Phase 2: Variable Extraction ---
    // Extract and type-cast configuration values, providing sensible defaults
    // to ensure the server can run even with a partially incomplete config.
    let app_name = restream_cfg["appName"].as_str().unwrap_or("restream").to_string();
    let server_port = restream_cfg["serverPort"].as_u64().unwrap_or(8080) as u16;
    let max_mem = restream_cfg["maxMemoryBytes"].as_u64().unwrap_or(1024 * 1024 * 1024); // 1GB default
    let reg_ttl = restream_cfg["registryTtl"].as_u64().unwrap_or(30);

    // --- Phase 3: Logging Setup ---
    let logger = Arc::new(LoggerLocal::new(app_name, None));
    let startup_ny = Utc::now().with_timezone(&Eastern).format("%Y-%m-%d %H:%M:%S EST");
    logger.info(&format!("ReStream Secure Gateway booting. NY Time: {}", startup_ny), None).await;

    // --- Phase 4: Core Infrastructure ---
    let memory_guard = Arc::new(GlobalMemoryGuard::new(max_mem));
    let dispatcher = Arc::new(Dispatcher::new(memory_guard));
    let registry = Arc::new(Registry::new(reg_ttl)); 
    let api_call = Arc::new(ApiCall::new(logger.clone()));
    
    // --- Phase 5: Upstream Management ---
    // This component is the brain for managing all incoming data sources.
    let market_status = Arc::new(MarketStatus::new(api_call, logger.clone()));
    let manager = Arc::new(UpstreamManager::new(registry.clone(), dispatcher.clone(), market_status.clone()));

    // --- Phase 6: Background Task Spawning ---
    // Spawn essential async tasks that run for the lifetime of the application.
    spawn_watchdog(logger.clone());
    let m_clone = manager.clone();
    tokio::spawn(async move { m_clone.run().await }); // The core upstream manager loop.

    // Initialize and run a specific data ingestor.
    let yahoo_ingestor = YahooWssIngestor::new(YahooConfig::default(), dispatcher.clone(), manager.clone());
    tokio::spawn(async move { yahoo_ingestor.run().await });

    // --- Phase 7: TLS Configuration ---
    // Secure communication is critical. We load LetsEncrypt certificates, which are
    // a common standard for free, automated TLS.
    let home_dir = dirs::home_dir().ok_or_else(|| anyhow::anyhow!("HOME directory not found"))?;
    let cert_path = home_dir.join(".letsencrypt").join("fullchain.pem");
    let key_path = home_dir.join(".letsencrypt").join("privkey.pem");

    logger.info(&format!("Loading TLS certs from: {}", cert_path.display()), None).await;

    let tls_config = RustlsConfig::from_pem_file(cert_path, key_path)
        .await
        .map_err(|e| anyhow::anyhow!("TLS Configuration Error: {}", e))?;

    // --- Phase 8: Router and Server Construction ---
    // Define the web server's routes and shared state.
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

    // --- Phase 9: Server Binding and Signal Handling ---
    let addr = SocketAddr::from(([0, 0, 0, 0], server_port));
    logger.info(&format!("WSS Secure Gateway live at https://{}", addr), None).await;

    let handle = axum_server::Handle::new();
    
    // Spawn a dedicated task for graceful shutdown. This ensures that we can
    // clean up resources and finish in-flight requests before exiting.
    let signal_handle = handle.clone();
    let signal_logger = logger.clone();
    tokio::spawn(async move {
        shutdown_signal().await;
        signal_logger.warn("Shutdown signal received. Closing server gracefully...", None).await;
        signal_handle.graceful_shutdown(Some(std::time::Duration::from_secs(10)));
    });

    // Bind the server with TLS and the signal handler. This is the final blocking call.
    axum_server::bind_rustls(addr, tls_config)
        .handle(handle)
        .serve(app.into_make_service_with_connect_info::<SocketAddr>())
        .await?;

    Ok(())
}

/// # Health Check Endpoint
///
/// A simple HTTP GET endpoint that returns "OK".
///
/// This is used by monitoring services (like load balancers or uptime checkers)
/// to verify that the server process is running and responsive to requests.
async fn health_handler() -> &'static str { 
    "OK" 
}

/// # WebSocket Upgrade Handler
///
/// Handles incoming HTTP requests to `/ws` and attempts to upgrade them to a
/// WebSocket connection.
///
/// It uses Axum's `WebSocketUpgrade` extractor to perform the protocol switch.
/// If successful, it passes the newly created socket to `handle_socket`.
///
/// ## Parameters
/// - `ws`: The `WebSocketUpgrade` extractor.
/// - `ConnectInfo(addr)`: Extracts the remote client's `SocketAddr`.
/// - `State(state)`: Provides access to the shared `AppState`.
async fn ws_handler(
    ws: WebSocketUpgrade,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    // The `on_upgrade` method defines the asynchronous closure that will execute
    // once the WebSocket handshake is complete.
    ws.on_upgrade(move |socket| handle_socket(socket, state, addr))
}

/// # WebSocket Connection Logic
///
/// Manages a single, active WebSocket client session.
///
/// ## Workflow:
/// 1.  **Client Registration**: A unique ID is created for the client, and it is
///     registered with the `Dispatcher` to receive data broadcasts. The client
///     is assigned a `Low` priority by default.
/// 2.  **Data Transmission Loop**: It continuously listens for new data frames
///     from its dedicated MPSC receiver channel (`rx`).
/// 3.  **Send to Client**: Each received frame is serialized to a string and sent
///     to the client over the WebSocket.
/// 4.  **Disconnection Handling**: If the `send` operation fails, it's assumed
///     the client has disconnected. The loop breaks.
/// 5.  **Client Deregistration**: Upon exit (either by disconnection or channel
///     closure), the client is removed from the `Dispatcher` to free up resources.
///
/// ## Parameters
/// - `socket`: The active `WebSocket` connection.
/// - `state`: The shared `AppState`.
/// - `addr`: The remote `SocketAddr` of the client.
async fn handle_socket(mut socket: WebSocket, state: Arc<AppState>, addr: SocketAddr) {
    let client_id = format!("ws-{}", addr);
    // Subscribe the client to the dispatcher, getting a receiver channel in return.
    let mut rx = state.dispatcher.add_client(&client_id, ClientPriority::Low);
    
    // This loop is the heart of the client session. It runs as long as the
    // dispatcher is sending data and the client is connected.
    while let Some(frame) = rx.recv().await {
        let payload_str = frame.payload.to_string();
        
        // Attempt to send the data. If it fails, the client has likely closed
        // the connection, so we break the loop.
        if socket.send(Message::Text(payload_str.into())).await.is_err() {
            break; 
        }
    }
    // Crucial cleanup step: ensure the client is removed from the dispatcher's
    // active list to prevent memory leaks.
    state.dispatcher.remove_client(&client_id);
}

/// # NY Midnight Restart Watchdog
///
/// Spawns a background task that triggers a process exit at midnight in the
/// US/Eastern timezone.
///
/// This is a simple but effective strategy to ensure long-term stability.
/// A daily restart can clear out any potential slow-burning memory leaks,
/// refresh state, and reset connections, preventing unforeseen issues in a
/// long-running process. The server is expected to be run under a process
/// manager (like `systemd` or `pm2`) that will automatically restart it.
///
/// ## Parameters
/// - `logger`: The shared logger for recording the restart event.
fn spawn_watchdog(logger: Arc<LoggerLocal>) {
    tokio::spawn(async move {
        loop {
            // Check the time every 30 seconds.
            let now_ny = Utc::now().with_timezone(&Eastern);
            if now_ny.hour() == 0 && now_ny.minute() == 0 {
                logger.warn("Scheduled NY Midnight restart.", None).await;
                // Exit the process. The process manager is responsible for restarting.
                std::process::exit(0);
            }
            // Wait before checking again to avoid a hot loop.
            tokio::time::sleep(std::time::Duration::from_secs(30)).await;
        }
    });
}

/// # Graceful Shutdown Signal Handler
///
/// Listens for `CTRL+C` (interrupt) and `SIGTERM` (terminate) signals to
/// initiate a graceful shutdown of the server.
///
/// On UNIX-like systems, it listens for both signals. On Windows, it only
/// listens for `CTRL+C`. The `tokio::select!` macro waits for the first
/// signal to be received.
///
/// This function is essential for allowing the server to shut down cleanly,
-/// saving state, and closing connections without abrupt interruptions.
async fn shutdown_signal() {
    // Handler for CTRL+C
    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    // Handler for SIGTERM (on UNIX systems)
    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };

    // On non-UNIX systems, `terminate` is a future that never completes.
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    // `tokio::select!` waits for the first of the futures to complete.
    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
}