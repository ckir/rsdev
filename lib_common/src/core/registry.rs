//! # Upstream Subscription Registry with Linger
//!
//! This module provides a `Registry` for managing subscriptions to upstream data
//! sources (e.g., individual stock tickers from a WebSocket feed). It is designed
//! to be efficient and prevent "flapping" – the rapid succession of subscribe and
//! unsubscribe requests for the same resource.
//!
//! ## Core Mechanism: The "Linger"
//!
//! When the last client unsubscribes from a symbol, the `Registry` does not
//! immediately tear down the upstream subscription. Instead, it starts a "linger"
//! timer.
//!
//! - **If a new client subscribes** to the same symbol before the timer expires,
//!   the timer is cancelled, and the existing upstream subscription is reused. This
//!   avoids the overhead of a new handshake or request to the upstream provider.
//! - **If the timer expires** without any new subscriptions, the `Registry`
//!   removes the symbol, signaling that the actual upstream connection can now be
//!   torn down.
//!
//! This mechanism is particularly useful in web-based scenarios where a user might
//! briefly navigate away from a page and then return, or where multiple components
//! on a page might independently subscribe to the same data.
//!
//! ## Implementation
//!
//! The registry uses a `HashMap` to store a reference count and a `CancellationToken`
//! for each symbol. The `CancellationToken` is a key part of the design, providing a
//! clean, race-free way to cancel the linger task if a re-subscription occurs.

#![doc(html_logo_url = "https://example.com/logo.png")] // Placeholder
#![forbid(unsafe_code)]
#![warn(missing_docs, rust_2018_idioms, unused_qualifications)]

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tokio::time::{sleep, Duration};
use tokio_util::sync::CancellationToken;

/// # Subscription Registry
///
/// Manages the lifecycle of subscriptions to upstream data sources, implementing
/// a reference counting and linger mechanism.
pub struct Registry {
    /// The core data structure, a thread-safe `HashMap`.
    /// - **Key**: The symbol or identifier of the upstream resource (e.g., "TSLA").
    /// - **Value**: A tuple `(u32, CancellationToken)`.
    ///   - `_0` (`u32`): A reference counter for the number of active subscribers.
    ///   - `_1` (`CancellationToken`): A token used to manage the linger task.
    subscriptions: Arc<Mutex<HashMap<String, (u32, CancellationToken)>>>,
    /// The duration in seconds that a symbol with zero subscribers should "linger"
    /// before being fully removed.
    linger_secs: u64,
}

impl Registry {
    /// Creates a new `Registry` with a specified linger duration.
    ///
    /// # Arguments
    /// * `linger_secs` - The time in seconds to wait before removing a subscription
    ///   with no active clients.
    pub fn new(linger_secs: u64) -> Self {
        Self {
            subscriptions: Arc::new(Mutex::new(HashMap::new())),
            linger_secs,
        }
    }

    /// # Subscribe
    ///
    /// Increments the reference count for a symbol. If this is the *first*
    /// subscription for the symbol, it signals that a new upstream connection
    /// should be established.
    ///
    /// ## Logic:
    /// 1.  Acquires a lock on the `subscriptions` map.
    /// 2.  Uses the `entry` API to efficiently find or create an entry for the symbol.
    /// 3.  Increments the reference count (`entry.0`).
    /// 4.  If the count becomes `1`, it means this is a new subscription (or a
    ///     re-subscription during a linger period).
    ///     - It cancels the existing `CancellationToken` to stop any pending linger task.
    ///     - It creates a fresh token for the next unsubscribe cycle.
    ///     - It returns `true` to signal "new subscription needed".
    /// 5.  If the count is greater than `1`, it returns `false`.
    ///
    /// # Arguments
    /// * `symbol` - The identifier of the resource to subscribe to.
    ///
    /// # Returns
    /// `true` if this is the first active subscriber, `false` otherwise.
    pub fn subscribe(&self, symbol: &str) -> bool {
        let mut subs = self.subscriptions.lock().expect("Registry lock poisoned");
        
        // Find or create the entry for the symbol.
        let entry = subs.entry(symbol.to_string()).or_insert_with(|| {
            (0, CancellationToken::new())
        });

        entry.0 += 1; // Increment the reference count.

        // If this is the first subscriber, it's a "new" subscription.
        if entry.0 == 1 {
            // Cancel any linger task that might be running for this symbol.
            entry.1.cancel(); 
            // Create a new token for when this subscriber eventually unsubscribes.
            entry.1 = CancellationToken::new();
            true
        } else {
            false
        }
    }

    /// # Unsubscribe
    ///
    /// Decrements the reference count for a symbol. If the count reaches zero,
    /// it initiates the "linger" mechanism.
    ///
    /// ## Logic:
    /// 1.  Acquires a lock on the `subscriptions` map.
    /// 2.  Finds the entry for the symbol.
    /// 3.  Decrements the reference count.
    /// 4.  If the count reaches `0`:
    ///     - It spawns a new asynchronous task to handle the linger period.
    ///     - The task is given a clone of the symbol's `CancellationToken`.
    ///     - The task uses `tokio::select!` to wait for one of two events:
    ///         a) The `linger_duration` elapses.
    ///         b) The `token` is cancelled (which happens if `subscribe` is called again).
    ///     - If the duration elapses, the task re-acquires the lock and removes the
    _   ///       symbol from the map, completing the cleanup.
    ///     - If the token is cancelled, the task simply terminates, aborting the cleanup.
    ///
    /// # Arguments
    /// * `symbol` - The identifier of the resource to unsubscribe from.
    pub fn unsubscribe(&self, symbol: &str) {
        let mut subs = self.subscriptions.lock().expect("Registry lock poisoned");
        
        if let Some(entry) = subs.get_mut(symbol) {
            if entry.0 > 0 {
                entry.0 -= 1;
            }

            // If this was the last subscriber, start the linger task.
            if entry.0 == 0 {
                let token: CancellationToken = entry.1.clone();
                let symbol_clone = symbol.to_string();
                let subs_handle = Arc::clone(&self.subscriptions);
                let linger_duration = Duration::from_secs(self.linger_secs);

                tokio::spawn(async move {
                    tokio::select! {
                        // If `subscribe` is called again, this token will be cancelled.
                        _ = token.cancelled() => {
                            log::debug!("Linger cancelled for symbol '{}' due to re-subscription.", symbol_clone);
                        },
                        // If the linger time passes without cancellation...
                        _ = sleep(linger_duration) => {
                            // ...re-lock and perform the final cleanup.
                            let mut lock = subs_handle.lock().unwrap();
                            // Final check to ensure the count is still zero before removing.
                            if let Some(e) = lock.get(&symbol_clone) {
                                if e.0 == 0 {
                                    lock.remove(&symbol_clone);
                                    log::info!("Linger expired. Subscription for '{}' removed.", symbol_clone);
                                }
                            }
                        }
                    }
                });
            }
        }
    }
}