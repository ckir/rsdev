//! # Registry (Full Version)
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tokio::time::{sleep, Duration};
use tokio_util::sync::CancellationToken;

pub struct Registry {
    // Explicitly defining the Tuple types here helps the compiler inference
    subscriptions: Arc<Mutex<HashMap<String, (u32, CancellationToken)>>>,
    linger_secs: u64,
}

impl Registry {
    pub fn new(linger_secs: u64) -> Self {
        Self {
            subscriptions: Arc::new(Mutex::new(HashMap::new())),
            linger_secs,
        }
    }

    pub fn subscribe(&self, symbol: &str) -> bool {
        let mut subs = self.subscriptions.lock().expect("Registry lock poisoned");
        
        // Use entry API to find or create the symbol entry
        let entry = subs.entry(symbol.to_string()).or_insert_with(|| {
            (0, CancellationToken::new())
        });

        entry.0 += 1;

        if entry.0 == 1 {
            // If it was lingering, cancel the old task and create a fresh token
            entry.1.cancel(); 
            entry.1 = CancellationToken::new();
            true
        } else {
            false
        }
    }

    pub fn unsubscribe(&self, symbol: &str) {
        let mut subs = self.subscriptions.lock().expect("Registry lock poisoned");
        
        if let Some(entry) = subs.get_mut(symbol) {
            if entry.0 > 0 {
                entry.0 -= 1;
            }

            if entry.0 == 0 {
                // Clone the token to move into the async linger task
                let token: CancellationToken = entry.1.clone();
                let symbol_clone = symbol.to_string();
                let subs_handle = Arc::clone(&self.subscriptions);
                let linger_duration = Duration::from_secs(self.linger_secs);

                tokio::spawn(async move {
                    tokio::select! {
                        // If token is cancelled, someone re-subscribed
                        _ = token.cancelled() => {
                            log::debug!("Linger cancelled for {}", symbol_clone);
                        },
                        // Wait for linger period
                        _ = sleep(linger_duration) => {
                            let mut lock = subs_handle.lock().unwrap();
                            if let Some(e) = lock.get(&symbol_clone) {
                                if e.0 == 0 {
                                    lock.remove(&symbol_clone);
                                    log::info!("Linger expired for {}", symbol_clone);
                                }
                            }
                        }
                    }
                });
            }
        }
    }
}