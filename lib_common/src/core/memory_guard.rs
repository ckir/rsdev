//! # Global Memory Guard
//! 
//! This module tracks the total heap memory consumed by all active client buffers.
//! It uses atomic counters for lock-free tracking of ingress and egress data weights.
//! 
//! When a client's buffer exceeds the global capacity, the `Dispatcher` uses this 
//! guard to trigger its priority-based eviction policy.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

/// Priority levels for client data streams.
/// Used by the Dispatcher to decide which buffers to evict first.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ClientPriority {
    /// Non-critical or legacy clients (Evicted first)
    Low = 0,
    /// Real-time trading or mission-critical clients
    High = 1,
}

/// A thread-safe controller for monitoring global memory consumption.
/// 
/// The guard doesn't block allocations; it simply acts as a shared counter 
/// that various tasks update as messages enter and leave the system.
pub struct GlobalMemoryGuard {
    /// The maximum allowed memory (in bytes) for all combined client channels.
    capacity: u64,
    /// The current total bytes held in memory.
    current_usage: Arc<AtomicU64>,
}

impl GlobalMemoryGuard {
    /// Creates a new memory guard with a fixed capacity.
    /// 
    /// # Example
    /// ```
    /// // Initialize with a 512MB limit
    /// let guard = GlobalMemoryGuard::new(512 * 1024 * 1024);
    /// ```
    pub fn new(max_bytes: u64) -> Self {
        Self {
            capacity: max_bytes,
            current_usage: Arc::new(AtomicU64::new(0)),
        }
    }

    /// Records an increase in memory usage (e.g., a new message arrived).
    /// 
    /// Returns `true` if the system is still within capacity.
    /// Returns `false` if the capacity has been exceeded, signaling that the 
    /// Dispatcher should begin the eviction process.
    pub fn increment(&self, bytes: u64) -> bool {
        // Use Relaxed ordering as we only care about the total sum, 
        // not strict synchronization between threads.
        let prev = self.current_usage.fetch_add(bytes, Ordering::Relaxed);
        prev + bytes <= self.capacity
    }

    /// Records a decrease in memory usage (e.g., a client processed a message).
    pub fn decrement(&self, bytes: u64) {
        // fetch_sub returns the previous value.
        self.current_usage.fetch_sub(bytes, Ordering::Relaxed);
    }

    /// Returns the current total memory usage in bytes.
    pub fn current_usage(&self) -> u64 {
        self.current_usage.load(Ordering::Relaxed)
    }

    /// Returns the maximum capacity of this guard.
    pub fn capacity(&self) -> u64 {
        self.capacity
    }

    /// Calculates what percentage of the capacity is currently in use.
    pub fn usage_percent(&self) -> f64 {
        let current = self.current_usage() as f64;
        let total = self.capacity as f64;
        (current / total) * 100.0
    }
}
