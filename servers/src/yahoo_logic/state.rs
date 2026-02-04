use crate::yahoo_logic::yahoo_finance::PricingData;
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};
use tokio::sync::{broadcast, mpsc};

#[derive(Clone)]
pub struct AppState {
    // Map of client_id -> Set of subscribed symbols
    client_subscriptions: Arc<Mutex<HashMap<usize, HashSet<String>>>>,
    // Map of symbol -> Count of clients subscribed
    symbol_counts: Arc<Mutex<HashMap<String, usize>>>,
    // Channel to send commands to the upstream client
    upstream_tx: Arc<Mutex<Option<mpsc::UnboundedSender<UpstreamCommand>>>>,
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

    pub fn set_upstream_tx(&self, tx: mpsc::UnboundedSender<UpstreamCommand>) {
        let mut guard = self.upstream_tx.lock().unwrap();
        *guard = Some(tx);
    }

    pub fn add_client(&self, client_id: usize) {
        let mut subs = self.client_subscriptions.lock().unwrap();
        subs.insert(client_id, HashSet::new());
    }

    pub fn remove_client(&self, client_id: usize) {
        let mut subs = self.client_subscriptions.lock().unwrap();
        if let Some(client_subs) = subs.remove(&client_id) {
            let mut counts = self.symbol_counts.lock().unwrap();
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
                self.send_upstream(UpstreamCommand::Unsubscribe(to_unsubscribe));
            }
        }
    }

    pub fn subscribe(&self, client_id: usize, symbols: Vec<String>) {
        let mut subs = self.client_subscriptions.lock().unwrap();
        let mut counts = self.symbol_counts.lock().unwrap();
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
            self.send_upstream(UpstreamCommand::Subscribe(to_subscribe));
        }
    }

    pub fn unsubscribe(&self, client_id: usize, symbols: Vec<String>) {
        let mut subs = self.client_subscriptions.lock().unwrap();
        let mut counts = self.symbol_counts.lock().unwrap();
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
            self.send_upstream(UpstreamCommand::Unsubscribe(to_unsubscribe));
        }
    }

    fn send_upstream(&self, cmd: UpstreamCommand) {
        let guard = self.upstream_tx.lock().unwrap();
        if let Some(tx) = &*guard {
            let _ = tx.send(cmd);
        }
    }

    pub fn is_subscribed(&self, client_id: usize, symbol: &str) -> bool {
        let subs = self.client_subscriptions.lock().unwrap();
        if let Some(client_subs) = subs.get(&client_id) {
            client_subs.contains(symbol)
        } else {
            false
        }
    }
}
