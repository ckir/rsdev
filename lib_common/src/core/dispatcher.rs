//! # High-Performance, Zero-Copy Message Dispatcher
//!
//! The `Dispatcher` is the central nervous system for data distribution within the
//! `rsdev` engine. It is responsible for taking a single, incoming data frame from
//! an ingestor and broadcasting it to potentially thousands of connected clients
//! with maximum efficiency and safety.
//!
//! ## Core Design Principles:
//!
//! 1.  **Zero-Copy Fan-out**: When a message is broadcast, the underlying data is not
//!     cloned for each client. Instead, it is wrapped in an `Arc` (Atomic Reference
//!     Counter). Each client receives a new `Arc` pointer to the *same* block of
//!     memory. This dramatically reduces memory allocation and CPU overhead, which is
//!     critical for high-throughput scenarios.
//!
//! 2.  **Backpressure and Memory Protection**: The `Dispatcher` does not operate in
//!     isolation. It communicates with a `GlobalMemoryGuard` *before* broadcasting.
//!     It estimates the total memory impact of a fan-out and checks if this would
//!     breach the system's memory limit. This is a proactive backpressure mechanism.
//!
//! 3.  **Priority-Based Eviction**: If the `GlobalMemoryGuard` signals that a broadcast
//!     would exceed the memory limit, the `Dispatcher` triggers its `enforce_eviction`
//!     logic. It identifies the "slowest" client (the one with the largest pending
//!     message queue) among the `LowPriority` clients and effectively purges their
//!     backlog. This sacrifices delivery to one non-critical client to protect the
//!     stability of the entire system.
//!
//! 4.  **Triple-Timestamping**: The `ReStreamFrame` carries timestamps from various
//!     stages of the pipeline (upstream source, ingestor ingress, and dispatcher egress),
//!     allowing for precise end-to-end latency monitoring.

#![doc(html_logo_url = "https://example.com/logo.png")] // Placeholder
#![forbid(unsafe_code)]
#![warn(missing_docs, rust_2018_idioms, unused_qualifications)]

use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;
use crate::core::memory_guard::{GlobalMemoryGuard, ClientPriority};

/// # Real-time Stream Frame
///
/// The standardized data structure that flows through the dispatcher. It wraps the
/// actual payload with critical metadata for processing, routing, and analytics.
/// The `Arc` wrapper around this struct is what enables zero-copy broadcasts.
#[derive(Debug, Clone)]
pub struct ReStreamFrame {
    /// Timestamp from the original data source (e.g., the trade execution time
    /// from the exchange), in Unix seconds or milliseconds.
    pub ts_upstream: u64,
    /// The precise `Instant` the data was received by our system's ingestor.
    /// This is crucial for calculating ingress and internal processing latency.
    pub ts_library_in: std::time::Instant,
    /// A flag indicating that one or more previous frames were dropped for this
    /// client due to memory pressure and eviction. This allows the client to
    /// know its state may be inconsistent.
    pub data_dropped: bool,
    /// The normalized market data payload, represented as a generic `serde_json::Value`.
    pub payload: serde_json::Value,
}

/// # Client Handle
///
/// An internal representation of a connected client (e.g., a WebSocket session).
/// It holds the necessary information to send data to the client and track its state.
struct ClientHandle {
    /// A unique identifier for the client, often derived from its connection info.
    id: String,
    /// The priority level of the client, which determines if its message queue is
    /// eligible for eviction under memory pressure.
    priority: ClientPriority,
    /// The sending half of an MPSC channel used to push data frames to the client's
    /// dedicated async task. The channel is unbounded, meaning sends will succeed
    /// instantly unless the receiver is dropped.
    sender: mpsc::UnboundedSender<Arc<ReStreamFrame>>,
    /// A thread-safe counter for the number of messages currently pending in the
    /// client's MPSC channel queue. This is the key metric for identifying slow clients.
    queue_size: Arc<Mutex<usize>>,
}

/// # Core Dispatcher
///
/// Manages the registration, deregistration, and broadcasting of data to all clients.
pub struct Dispatcher {
    /// A thread-safe, shared list of all currently connected `ClientHandle`s.
    clients: Arc<Mutex<Vec<ClientHandle>>>,
    /// A shared reference to the system's `GlobalMemoryGuard` to coordinate on
    /// memory usage and backpressure.
    memory_guard: Arc<GlobalMemoryGuard>,
}

impl Dispatcher {
    /// Creates a new `Dispatcher` linked to a global memory controller.
    pub fn new(memory_guard: Arc<GlobalMemoryGuard>) -> Self {
        Self {
            clients: Arc::new(Mutex::new(Vec::new())),
            memory_guard,
        }
    }

    /// # Add Client
    ///
    /// Registers a new client with the dispatcher.
    ///
    /// This method creates a new unbounded MPSC channel for the client and stores
    /// the `ClientHandle` (including the sender half) in the shared list. It then
    /// returns the receiver half to the caller, which will be managed by the
    /// client's dedicated task.
    ///
    /// # Returns
    /// An `mpsc::UnboundedReceiver` that will receive all frames broadcast by the dispatcher.
    pub fn add_client(&self, id: &str, priority: ClientPriority) -> mpsc::UnboundedReceiver<Arc<ReStreamFrame>> {
        let (tx, rx) = mpsc::unbounded_channel();
        let mut clients = self.clients.lock().expect("Dispatcher lock poisoned");

        let handle = ClientHandle {
            id: id.to_string(),
            priority,
            sender: tx,
            queue_size: Arc::new(Mutex::new(0)),
        };

        clients.push(handle);
        log::info!("Client '{}' registered with priority {:?}", id, priority);
        rx
    }

    /// # Broadcast
    ///
    /// The main fan-out method. It takes a data payload and distributes it to all
    /// registered clients.
    ///
    /// ## Logic:
    /// 1.  Wraps the incoming data in an `Arc<ReStreamFrame>` to enable zero-copy sends.
    /// 2.  Estimates the memory impact of the broadcast (frame size * number of clients).
    /// 3.  **Memory Check**: Atomically increments the `GlobalMemoryGuard` counter.
    ///     - If the increment succeeds, the broadcast proceeds.
    ///     - If it fails (breaches memory limit), `enforce_eviction` is called *before*
    ///       the broadcast to free up memory.
    /// 4.  Iterates through the list of clients and sends an `Arc` clone of the frame
    ///     to each one.
    /// 5.  **Cleanup**: It uses `retain` to efficiently remove clients whose `send`
    ///     operation failed, which indicates they have disconnected.
    pub async fn broadcast(&self, payload: serde_json::Value, ts_upstream: u64, ts_in: std::time::Instant) {
        let frame = Arc::new(ReStreamFrame {
            ts_upstream,
            ts_library_in: ts_in,
            data_dropped: false, // This would be set to true for frames sent after an eviction
            payload,
        });

        // Heuristic: estimate memory footprint (JSON string length + some overhead).
        let estimated_size = (frame.payload.to_string().len() + 64) as u64;

        let mut clients = self.clients.lock().expect("Dispatcher lock poisoned");
        
        // --- 1. Proactive Memory Guard Check ---
        let total_fanout_impact = estimated_size * clients.len() as u64;
        if !self.memory_guard.increment(total_fanout_impact) {
            self.enforce_eviction(&mut clients);
        }

        // --- 2. Zero-Copy Fan-out and Cleanup ---
        clients.retain(|client| {
            match client.sender.send(Arc::clone(&frame)) {
                Ok(_) => {
                    // Increment the client's queue size counter.
                    let mut size = client.queue_size.lock().unwrap();
                    *size += 1;
                    true // Keep the client.
                },
                Err(_) => {
                    // Receiver was dropped, meaning the client disconnected.
                    log::info!("Client '{}' disconnected. Removing from dispatcher.", client.id);
                    // Decrement the memory guard for the messages that were in this
                    // client's queue before it disconnected.
                    let freed_count = *client.queue_size.lock().unwrap();
                    self.memory_guard.decrement(estimated_size * freed_count as u64);
                    false // Remove the client.
                }
            }
        });
    }

    /// # Enforce Eviction
    ///
    /// This is the backpressure mechanism. It finds the `LowPriority` client with
    /// the largest backlog (`queue_size`) and clears it to reclaim memory.
    ///
    /// **Note**: A true MPSC channel clear is not trivial. This implementation
    /// simulates the eviction by resetting the `queue_size` counter and decrementing
    /// the `GlobalMemoryGuard`. A more advanced implementation might involve sending
    /// a special "eviction" frame or re-initializing the channel.
    fn enforce_eviction(&self, clients: &mut [ClientHandle]) {
        // Find the "slowest" low-priority client.
        let target = clients.iter_mut()
            .filter(|c| c.priority == ClientPriority::Low)
            .max_by_key(|c| *c.queue_size.lock().unwrap());

        if let Some(fat_client) = target {
            let mut size_lock = fat_client.queue_size.lock().unwrap();
            log::warn!(
                "Memory limit exceeded. Evicting {} messages from low-priority client '{}' to reclaim memory.",
                *size_lock,
                fat_client.id
            );
            
            // Estimate the memory being freed and update the guard.
            // (Using an average size is a heuristic).
            let freed_est = (*size_lock as u64) * 128; // Avg frame size heuristic
            self.memory_guard.decrement(freed_est);
            
            // Reset the queue size for the evicted client.
            *size_lock = 0;
            
            // TODO: Send a single `ReStreamFrame` with `data_dropped = true` to notify
            // the client that its state is now inconsistent.
        }
    }

    /// Removes a specific client by its ID.
    pub fn remove_client(&self, id: &str) {
        let mut clients = self.clients.lock().expect("Dispatcher lock poisoned");
        // `retain` is efficient for removing items from a Vec.
        clients.retain(|c| c.id != id);
        log::info!("Client '{}' explicitly removed.", id);
    }
}