//! # Upstream Manager
//! 
//! The central coordinator for the ReStream engine. 
//! It monitors market state and manages the transition between 
//! Streaming (Primary) and Polling (Failover) modes.

use std::sync::Arc;
use tokio::sync::RwLock;
use std::time::Duration;

use crate::core::registry::Registry;
use crate::core::dispatcher::Dispatcher;
use crate::markets::nasdaq::marketstatus::MarketStatus;

/// Operational states for the gateway.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OperationMode {
    /// Normal Market Hours: Primary WSS is active.
    Streaming,
    /// WSS Failure or Market Volatility: REST Polling is active.
    FailoverPolling,
    /// Market Closed: Minimal activity, low-frequency napping.
    Idle,
}

pub struct UpstreamManager {
    _registry: Arc<Registry>,
    _dispatcher: Arc<Dispatcher>,
    market_status: Arc<MarketStatus>,
    mode: Arc<RwLock<OperationMode>>,
}

impl UpstreamManager {
    /// Creates a new UpstreamManager instance.
    pub fn new(registry: Arc<Registry>, dispatcher: Arc<Dispatcher>, market_status: Arc<MarketStatus>) -> Self {
        Self {
            _registry: registry, // Fixed: explicitly mapping to the underscored field
            _dispatcher: dispatcher,
            market_status,
            mode: Arc::new(RwLock::new(OperationMode::Idle)),
        }
    }

    /// The main coordination loop.
    pub async fn run(&self) {
        println!("Upstream Manager started.");
        
        loop {
            self.reconcile_state().await;
        }
    }

    /// Determines if we should be in Streaming, Polling, or Idle mode.
    async fn reconcile_state(&self) {
        match self.market_status.get_status().await {
            Ok(data) => {
                let new_mode = if data.mrkt_status == "Open" {
                    OperationMode::Streaming
                } else {
                    OperationMode::Idle
                };

                // Update Mode
                {
                    let mut mode_lock = self.mode.write().await;
                    if *mode_lock != new_mode {
                        println!("Transitioning mode: {:?} -> {:?}", *mode_lock, new_mode);
                        *mode_lock = new_mode;
                        self.handle_mode_change(new_mode).await;
                    }
                }

                // Handle the "Nap" or wait interval
                if new_mode == OperationMode::Idle {
                    let nap = data.get_sleep_duration();
                    println!("Market is closed. Napping for {} seconds.", nap.as_secs());
                    tokio::time::sleep(nap).await;
                } else {
                    // While streaming, check status every 60 seconds
                    tokio::time::sleep(Duration::from_secs(60)).await;
                }
            }
            Err(e) => {
                eprintln!("Error fetching market status: {}. Retrying in 30s.", e);
                tokio::time::sleep(Duration::from_secs(30)).await;
            }
        }
    }

    /// Executes side-effects of a mode change (e.g., stopping/starting pollers).
    async fn handle_mode_change(&self, _new_mode: OperationMode) {
        match _new_mode {
            OperationMode::Streaming => {
                // YahooWssIngestor monitors this mode via get_current_mode()
            }
            OperationMode::FailoverPolling => {}
            OperationMode::Idle => {}
        }
    }

    /// Thread-safe access to the current operation mode.
    pub async fn get_current_mode(&self) -> OperationMode {
        *self.mode.read().await
    }
}