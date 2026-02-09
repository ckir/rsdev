//! # Upstream Manager
//!
//! The central coordinator for the data ingestion and distribution engine.
//!
//! This module acts as the "brain" of the system, responsible for monitoring
//! the state of the financial markets and managing the operational mode of the
//! gateway. Its primary duty is to transition the system between different
//! states based on market hours and the health of data sources.
//!
//! ## Core Responsibilities:
//! - **State Reconciliation**: Continuously checks the market status (via `MarketStatus`).
//! - **Mode Management**: Transitions the system between `OperationMode` states
//!   (`Streaming`, `FailoverPolling`, `Idle`).
//! - **Lifecycle Coordination**: Provides a single source of truth (`get_current_mode`) for
//!   other components (like ingestors) to query the system's intended operational state.
//!   This allows dependent services to start, stop, or modify their behavior in sync
//!   with the market.
//!
//! The manager's run loop (`run`) is designed to be a long-lived async task that
//! orchestrates the entire application lifecycle.

#![doc(html_logo_url = "https://example.com/logo.png")] // Placeholder
#![forbid(unsafe_code)]
#![warn(missing_docs, rust_2018_idioms, unused_qualifications)]


use std::sync::Arc;
use tokio::sync::RwLock;
use std::time::Duration;

use crate::core::registry::Registry;
use crate::core::dispatcher::Dispatcher;
use crate::markets::nasdaq::marketstatus::MarketStatus;

/// # Operational Mode
///
/// Defines the possible states of the data gateway.
///
/// The `UpstreamManager` uses this enum to track and transition the system's
/// behavior based on external factors like market hours or data source health.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OperationMode {
    /// **Normal Market Hours**: The primary, high-throughput WebSocket ingestors
    /// are active. Data is being streamed in real-time.
    Streaming,
    /// **WSS Failure or High Volatility**: The primary ingestor has failed. The system
    /// switches to a REST-based polling mechanism as a backup to ensure data flow
    /// continues, albeit at a lower frequency.
    FailoverPolling,
    /// **Market Closed**: The market is not open for trading. The system enters a
    /// low-power state, pausing most data ingestion activities and sleeping until
    /// the next market open.
    Idle,
}

/// # Upstream Manager
///
/// The core orchestrator for managing data sources and system state.
///
/// It holds shared references to all critical infrastructure components required
/// to make decisions about the system's operational mode.
pub struct UpstreamManager {
    /// A registry for tracking the health and status of upstream data sources.
    /// (Currently held, but full implementation of failover logic is pending).
    _registry: Arc<Registry>,
    /// The central dispatcher for broadcasting data to connected clients.
    /// (Held for future use, e.g., to issue system-wide notifications).
    _dispatcher: Arc<Dispatcher>,
    /// The client for checking the NASDAQ market's open/closed status.
    market_status: Arc<MarketStatus>,
    /// The current `OperationMode` of the system, protected by a `RwLock` for
    /// safe concurrent access from multiple async tasks.
    mode: Arc<RwLock<OperationMode>>,
}

impl UpstreamManager {
    /// Creates a new `UpstreamManager` instance.
    ///
    /// # Arguments
    /// * `registry` - Shared `Registry` for upstream health tracking.
    /// * `dispatcher` - Shared `Dispatcher` for data distribution.
    /// * `market_status` - Shared `MarketStatus` client for market state checks.
    pub fn new(registry: Arc<Registry>, dispatcher: Arc<Dispatcher>, market_status: Arc<MarketStatus>) -> Self {
        Self {
            _registry: registry,
            _dispatcher: dispatcher,
            market_status,
            mode: Arc::new(RwLock::new(OperationMode::Idle)),
        }
    }

    /// # Main Coordination Loop
    ///
    /// The primary entry point for the manager's long-running task. It continuously
    /// calls `reconcile_state` to ensure the system's mode is always aligned with
    /// the actual market status.
    ///
    /// This function is designed to be spawned as a persistent background task.
    pub async fn run(&self) {
        println!("Upstream Manager started.");
        
        loop {
            self.reconcile_state().await;
        }
    }

    /// # State Reconciliation
    ///
    /// Checks the current market status and adjusts the system's `OperationMode` accordingly.
    ///
    /// ## Logic:
    /// 1.  Fetches the latest market status from the `MarketStatus` client.
    /// 2.  **On Success**:
    ///     -   Determines the `new_mode` (`Streaming` if market is "Open", `Idle` otherwise).
    ///     -   Acquires a write lock on the `mode`.
    ///     -   If the mode has changed, it logs the transition and calls `handle_mode_change`.
    ///     -   If the new mode is `Idle`, it calculates the duration until the next market
    ///         open and puts the task to sleep for that duration (`nap`).
    ///     -   If the mode is `Streaming`, it sleeps for a shorter, fixed interval before
    ///         the next check.
    /// 3.  **On Error**:
    ///     -   Logs the error and sleeps for a brief retry interval. This makes the manager
    ///         resilient to transient network failures when checking market status.
    async fn reconcile_state(&self) {
        match self.market_status.get_status().await {
            Ok(data) => {
                let new_mode = if data.mrkt_status == "Open" {
                    OperationMode::Streaming
                } else {
                    OperationMode::Idle
                };

                // Lock the mode for writing only when a change is needed.
                {
                    let mut mode_lock = self.mode.write().await;
                    if *mode_lock != new_mode {
                        println!("Transitioning mode: {:?} -> {:?}", *mode_lock, new_mode);
                        *mode_lock = new_mode;
                        // Trigger side-effects associated with the new mode.
                        self.handle_mode_change(new_mode).await;
                    }
                }

                // Determine the appropriate sleep duration based on the current mode.
                if new_mode == OperationMode::Idle {
                    let nap = data.get_sleep_duration();
                    println!("Market is closed. Napping for {} seconds.", nap.as_secs());
                    tokio::time::sleep(nap).await;
                } else {
                    // During market hours, periodically re-verify status.
                    tokio::time::sleep(Duration::from_secs(60)).await;
                }
            }
            Err(e) => {
                eprintln!("Error fetching market status: {}. Retrying in 30s.", e);
                tokio::time::sleep(Duration::from_secs(30)).await;
            }
        }
    }

    /// # Handle Mode Change
    ///
    /// Executes side-effects when the `OperationMode` changes.
    ///
    /// This function is the designated place to orchestrate actions like starting
    /// or stopping ingestors. For example, transitioning to `FailoverPolling` would
    /// trigger the shutdown of the primary `Streaming` ingestor and the startup of
    /// a secondary polling ingestor.
    ///
    /// **Note**: Currently, this is a placeholder. The primary `YahooWssIngestor`
    /// polls `get_current_mode` directly to manage its own lifecycle. A more
_   /// sophisticated implementation would use this function to push commands to ingestors.
    async fn handle_mode_change(&self, _new_mode: OperationMode) {
        match _new_mode {
            OperationMode::Streaming => {
                // The primary ingestor (e.g., YahooWssIngestor) actively monitors
                // the mode via `get_current_mode` and will start itself.
                println!("Handler: Mode changed to Streaming. Ingestors should activate.");
            }
            OperationMode::FailoverPolling => {
                // Placeholder for future logic to activate backup REST pollers.
                println!("Handler: Mode changed to FailoverPolling. Activating REST pollers.");
            }
            OperationMode::Idle => {
                // Ingestors will see this mode and should go to sleep or shut down.
                println!("Handler: Mode changed to Idle. Ingestors should deactivate.");
            }
        }
    }

    /// # Get Current Operation Mode
    ///
    /// Provides thread-safe, read-only access to the current `OperationMode`.
    ///
    /// This is the primary public method for other parts of the system to query the
    /// manager's state. It acquires a read lock on the `mode` to ensure that it
    /// returns a consistent value, even if another task is in the process of
    /// changing the mode.
    ///
    /// ## Example Usage
    /// ```ignore
    /// // In an ingestor's run loop:
    /// if manager.get_current_mode().await == OperationMode::Streaming {
    ///     // ... connect to WebSocket and stream data
    /// } else {
    ///     // ... disconnect and sleep
    /// }
    /// ```
    pub async fn get_current_mode(&self) -> OperationMode {
        *self.mode.read().await
    }
}