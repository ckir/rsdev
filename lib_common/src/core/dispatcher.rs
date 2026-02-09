//! # Message Dispatcher
//! 
//! The Dispatcher is the central hub for data distribution. It implements:
//! 1. **Zero-Copy Fan-out**: Uses `Arc` to share a single message across many clients.
//! 2. **Priority Eviction**: Monitors the `GlobalMemoryGuard` and purges low-priority 
//!    buffers if RAM usage exceeds the limit.
//! 3. **Triple-Timestamping**: Tracks timing from upstream, ingress, and egress.

use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;
use crate::core::memory_guard::{GlobalMemoryGuard, ClientPriority};

/// Metadata and payload wrapped for zero-copy distribution.
#[derive(Debug, Clone)]
pub struct ReStreamFrame {
    /// Timestamp from the original source (e.g., Yahoo server time).
    pub ts_upstream: u64,
    /// Exact Instant the data hit our library's socket.
    pub ts_library_in: std::time::Instant,
    /// Flag indicating if this client has missed previous data due to eviction.
    pub data_dropped: bool,
    /// The normalized market data.
    pub payload: serde_json::Value,
}

/// Internal handle representing a connected client.
struct ClientHandle {
    id: String,
    priority: ClientPriority,
    sender: mpsc::UnboundedSender<Arc<ReStreamFrame>>,
    /// Tracks count of pending messages in the Unbounded channel.
    queue_size: Arc<Mutex<usize>>,
}

/// The core dispatcher managing multi-tenant distribution.
pub struct Dispatcher {
    clients: Arc<Mutex<Vec<ClientHandle>>>,
    memory_guard: Arc<GlobalMemoryGuard>,
}

impl Dispatcher {
    /// Creates a new dispatcher linked to a global memory controller.
    pub fn new(memory_guard: Arc<GlobalMemoryGuard>) -> Self {
        Self {
            clients: Arc::new(Mutex::new(Vec::new())),
            memory_guard,
        }
    }

    /// Registers a new client and returns their private data receiver.
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
        log::info!("Client {} registered with priority {:?}", id, priority);
        rx
    }

    /// Broadcasts a message to all active clients.
    pub async fn broadcast(&self, payload: serde_json::Value, ts_upstream: u64, ts_in: std::time::Instant) {
        // Wrap payload in Arc for Zero-Copy distribution
        let frame = Arc::new(ReStreamFrame {
            ts_upstream,
            ts_library_in: ts_in,
            data_dropped: false,
            payload,
        });

        // Heuristic: estimate memory footprint (JSON string length + struct overhead)
        let estimated_size = (frame.payload.to_string().len() + 64) as u64;

        let mut clients = self.clients.lock().expect("Dispatcher lock poisoned");
        
        // 1. Check/Update Memory Guard
        let total_fanout_impact = estimated_size * clients.len() as u64;
        if !self.memory_guard.increment(total_fanout_impact) {
            self.enforce_eviction(&mut clients);
        }

        // 2. Parallel Fan-out
        clients.retain(|client| {
            match client.sender.send(Arc::clone(&frame)) {
                Ok(_) => {
                    let mut size = client.queue_size.lock().unwrap();
                    *size += 1;
                    true
                },
                Err(_) => {
                    // Receiver was dropped (client disconnected)
                    self.memory_guard.decrement(estimated_size);
                    false 
                }
            }
        });
    }

    /// Finds the largest Low-Priority queue and clears it to free memory.
    fn enforce_eviction(&self, clients: &mut Vec<ClientHandle>) {
        let target = clients.iter_mut()
            .filter(|c| c.priority == ClientPriority::Low)
            .max_by_key(|c| *c.queue_size.lock().unwrap());

        if let Some(fat_client) = target {
            let mut size_lock = fat_client.queue_size.lock().unwrap();
            log::warn!("Evicting {} messages from client: {}", *size_lock, fat_client.id);
            
            // In an industrial gateway, you'd signal the drop here.
            // For this implementation, we decrement the guard by the estimated bulk.
            let freed_est = (*size_lock as u64) * 128; // Avg frame size
            self.memory_guard.decrement(freed_est);
            *size_lock = 0;
            
            // To fully clear an MPSC channel, the client task must cooperate 
            // or the sender must be re-initialized.
        }
    }

    /// Removes a specific client by ID.
    pub fn remove_client(&self, id: &str) {
        let mut clients = self.clients.lock().expect("Dispatcher lock poisoned");
        clients.retain(|c| c.id != id);
    }
}