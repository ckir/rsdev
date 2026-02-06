use crate::yahoo_logic::pricing_data::PricingData;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tokio::sync::{broadcast, mpsc, oneshot, Mutex};

// Result type for acknowledgements
pub type AckResult = Result<(), String>;

// Struct to wrap the command and a one-time channel for the response
pub struct UpstreamRequest {
    pub command: UpstreamCommand,
    pub responder: oneshot::Sender<AckResult>,
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
}

#[derive(Debug)]
pub enum UpstreamCommand {
    Subscribe(Vec<String>),
    Unsubscribe(Vec<String>),
}

impl AppState {
    pub fn new() -> Self {
        let (data_tx, _) = broadcast::channel(1000); // Buffer size 1000
        Self {
            client_subscriptions: Arc::new(Mutex::new(HashMap::new())),
            symbol_counts: Arc::new(Mutex::new(HashMap::new())),
            upstream_tx: Arc::new(Mutex::new(None)),
            data_tx,
        }
    }

    pub async fn set_upstream_tx(&self, tx: mpsc::UnboundedSender<UpstreamRequest>) {
        let mut guard = self.upstream_tx.lock().await;
        *guard = Some(tx);
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
}
