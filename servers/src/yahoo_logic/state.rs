use crate::yahoo_logic::pricing_data::PricingData;
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, atomic::{AtomicU64, Ordering}};
use tokio::sync::{broadcast, mpsc, oneshot, Mutex};
use tokio::time::Instant;

// Result type for acknowledgements
pub type AckResult = Result<(), String>;

// Struct to wrap the command and a one-time channel for the response
pub struct UpstreamRequest {
    pub command: UpstreamCommand,
    pub responder: oneshot::Sender<AckResult>,
}

#[derive(Clone, Debug)]
pub enum Notification {
    UpstreamDisconnected,
    UpstreamReconnected,
    UpstreamResubscribed,
    Error(String),
}

#[derive(Clone)]
pub struct AppState {
    // Map of client_id -> Set of subscribed symbols
    client_subscriptions: Arc<Mutex<HashMap<usize, HashSet<String>>>>,
    // Map of symbol -> Count of clients subscribed
    symbol_counts: Arc<Mutex<HashMap<String, usize>>>,
    // Channel to send commands to the upstream client
    upstream_tx: Arc<Mutex<Option<mpsc::UnboundedSender<UpstreamRequest>>>>,
    // Channel to broadcast pricing data to all clients
    pub data_tx: broadcast::Sender<Arc<PricingData>>,
    // Timestamp of the last received data from upstream
    last_data_timestamp: Arc<AtomicU64>, // Stored as nanoseconds since epoch
    // Channel to broadcast notifications to all clients
    pub notification_tx: broadcast::Sender<Notification>,
}

#[derive(Debug)]
pub enum UpstreamCommand {
    Subscribe(Vec<String>),
    Unsubscribe(Vec<String>),
}

impl AppState {
    pub fn new() -> Self {
        let (data_tx, _) = broadcast::channel(1000); // Buffer size 1000
        let (notification_tx, _) = broadcast::channel(100); // Buffer size 100 for notifications
        Self {
            client_subscriptions: Arc::new(Mutex::new(HashMap::new())),
            symbol_counts: Arc::new(Mutex::new(HashMap::new())),
            upstream_tx: Arc::new(Mutex::new(None)),
            data_tx,
            last_data_timestamp: Arc::new(AtomicU64::new(Instant::now().elapsed().as_nanos() as u64)),
            notification_tx,
        }
    }

    pub async fn set_upstream_tx(&self, tx: mpsc::UnboundedSender<UpstreamRequest>) {
        let mut guard = self.upstream_tx.lock().await;
        *guard = Some(tx);
    }

    pub fn update_last_data_timestamp(&self) {
        self.last_data_timestamp.store(Instant::now().elapsed().as_nanos() as u64, Ordering::Relaxed);
    }

    pub fn get_last_data_timestamp(&self) -> Instant {
        // We cannot directly reconstruct Instant from elapsed nanos,
        // so we store the current elapsed nanos and compare against it.
        // This is primarily for monitoring the time *since* the last data.
        let elapsed_at_data_time_nanos = self.last_data_timestamp.load(Ordering::Relaxed);
        let elapsed_at_data_time = tokio::time::Duration::from_nanos(elapsed_at_data_time_nanos);
        Instant::now() - (tokio::time::Instant::now().elapsed() - elapsed_at_data_time)
    }

    pub async fn has_active_subscriptions(&self) -> bool {
        let subs = self.client_subscriptions.lock().await;
        !subs.is_empty()
    }

    pub async fn get_all_subscribed_symbols(&self) -> Vec<String> {
        let counts = self.symbol_counts.lock().await;
        counts.keys().cloned().collect()
    }

    pub async fn add_client(&self, client_id: usize) {
        let mut subs = self.client_subscriptions.lock().await;
        subs.insert(client_id, HashSet::new());
    }

    pub async fn remove_client(&self, client_id: usize) {
        let mut subs = self.client_subscriptions.lock().await;
        if let Some(client_subs) = subs.remove(&client_id) {
            let mut counts = self.symbol_counts.lock().await;
            let mut to_unsubscribe = Vec::new();

            for symbol in client_subs {
                if let Some(count) = counts.get_mut(&symbol) {
                    *count -= 1;
                    if *count == 0 {
                        counts.remove(&symbol);
                        to_unsubscribe.push(symbol);
                    }
                }
            }

            if !to_unsubscribe.is_empty() {
                // We don't really need to wait for the ack here since the client is gone,
                // but for consistency we can. We'll ignore the result.
                let _ = self.send_upstream(UpstreamCommand::Unsubscribe(to_unsubscribe)).await;
            }
        }
    }

    pub async fn subscribe(&self, client_id: usize, symbols: Vec<String>) -> AckResult {
        let mut subs = self.client_subscriptions.lock().await;
        let mut counts = self.symbol_counts.lock().await;
        let mut to_subscribe = Vec::new();

        if let Some(client_subs) = subs.get_mut(&client_id) {
            for symbol in symbols {
                if client_subs.insert(symbol.clone()) {
                    let count = counts.entry(symbol.clone()).or_insert(0);
                    *count += 1;
                    if *count == 1 {
                        to_subscribe.push(symbol);
                    }
                }
            }
        }

        if !to_subscribe.is_empty() {
            self.send_upstream(UpstreamCommand::Subscribe(to_subscribe)).await
        } else {
            Ok(()) // Nothing to do, but the command is "successful"
        }
    }

    // This method will be called when we need to re-subscribe all active symbols
    pub async fn resubscribe_all(&self, symbols: Vec<String>) -> AckResult {
        if symbols.is_empty() {
            return Ok(());
        }
        self.send_upstream(UpstreamCommand::Subscribe(symbols)).await
    }

    pub async fn unsubscribe(&self, client_id: usize, symbols: Vec<String>) -> AckResult {
        let mut subs = self.client_subscriptions.lock().await;
        let mut counts = self.symbol_counts.lock().await;
        let mut to_unsubscribe = Vec::new();

        if let Some(client_subs) = subs.get_mut(&client_id) {
            for symbol in symbols {
                if client_subs.remove(&symbol) {
                    if let Some(count) = counts.get_mut(&symbol) {
                        *count -= 1;
                        if *count == 0 {
                            counts.remove(&symbol);
                            to_unsubscribe.push(symbol);
                        }
                    }
                }
            }
        }

        if !to_unsubscribe.is_empty() {
            self.send_upstream(UpstreamCommand::Unsubscribe(to_unsubscribe)).await
        } else {
            Ok(()) // Nothing to do, but the command is "successful"
        }
    }

    async fn send_upstream(&self, cmd: UpstreamCommand) -> AckResult {
        let (tx, rx) = oneshot::channel();
        let request = UpstreamRequest {
            command: cmd,
            responder: tx,
        };

        {
            let guard = self.upstream_tx.lock().await;
            if let Some(tx_chan) = &*guard {
                if tx_chan.send(request).is_err() {
                    return Err("Failed to send command to upstream service.".to_string());
                }
            } else {
                return Err("Upstream service not available.".to_string());
            }
        }

        // Wait for the response from the upstream task
        rx.await.unwrap_or_else(|_| Err("No response from upstream service.".to_string()))
    }

    pub async fn is_subscribed(&self, client_id: usize, symbol: &str) -> bool {
        let subs = self.client_subscriptions.lock().await;
        if let Some(client_subs) = subs.get(&client_id) {
            client_subs.contains(symbol)
        } else {
            false
        }
    }

    pub fn notify_clients(&self, notification: Notification) {
        if let Err(e) = self.notification_tx.send(notification) {
            log::warn!("Failed to send notification to clients: {}", e);
        }
    }
}

