//! # Global Memory Guard and Backpressure Mechanism
//!
//! This module provides a high-performance, lock-free mechanism for tracking the
//! estimated heap memory consumed by all active client buffers in the system. It
//! acts as a centralized accountant, enabling a system-wide backpressure strategy
//! to prevent out-of-memory (OOM) errors during high-throughput periods.
//!
//! ## Core Functionality:
//!
//! - **Atomic Accounting**: It uses an `AtomicU64` for the `current_usage` counter.
//!   This allows multiple threads (e.g., from the `Dispatcher` and various client
//!   tasks) to increment and decrement the memory usage without requiring expensive
//!   mutexes, ensuring minimal contention and maximum performance.
//!
//! - **Centralized Control**: By holding a shared `Arc<GlobalMemoryGuard>`, different
//!   parts of the system can coordinate on memory usage. The `Dispatcher` increments
//!   the guard *before* broadcasting, and client-handling tasks would decrement it
//!   *after* processing a message.
//!
//! - **Enabling Eviction**: The guard itself does not block or evict. Its primary role
//!   is to signal when the memory `capacity` has been breached. The `increment` method
//!   returns `false` upon a breach, which serves as the trigger for the `Dispatcher`
//!   to activate its `enforce_eviction` policy, targeting low-priority clients.

#![doc(html_logo_url = "https://example.com/logo.png")] // Placeholder
#![forbid(unsafe_code)]
#![warn(missing_docs, rust_2018_idioms, unused_qualifications)]

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

/// # Client Priority
///
/// Defines priority levels for client data streams. This is a critical piece of
/// metadata that allows the `Dispatcher` to make intelligent decisions about which
/// clients to sacrifice when memory pressure is high.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ClientPriority {
    /// **Low Priority**: Represents non-critical clients, such as those for monitoring,
    /// logging, or free-tier users. These clients' buffers are the first to be
    /// targeted for eviction when the system is under memory pressure.
    Low = 0,
    /// **High Priority**: Represents mission-critical clients, such as those used
    /// for algorithmic trading, paid subscribers, or core system services. The
    /// dispatcher will avoid evicting messages from these clients' queues.
    High = 1,
}

/// # Global Memory Guard
///
/// A thread-safe, high-performance controller for monitoring and managing the
/// estimated global memory consumption of in-flight messages.
///
/// The guard acts as a shared counter that various asynchronous tasks can update
/// without blocking each other, providing a real-time view of the system's memory
/// state.
pub struct GlobalMemoryGuard {
    /// The maximum allowed memory (in bytes) for all combined client channels.
    /// This is the hard limit that triggers the backpressure mechanism.
    capacity: u64,
    /// An atomic counter for the current total bytes estimated to be held in all
    /// client message queues. Using `AtomicU64` is essential for lock-free updates
    /// from many concurrent tasks.
    current_usage: Arc<AtomicU64>,
}

impl GlobalMemoryGuard {
    /// Creates a new `GlobalMemoryGuard` with a fixed capacity.
    ///
    /// # Example
    /// ```rust
    /// use std::sync::Arc;
    /// use lib_common::core::memory_guard::GlobalMemoryGuard;
    ///
    /// // Initialize with a 512MB limit.
    /// let guard = Arc::new(GlobalMemoryGuard::new(512 * 1024 * 1024));
    /// ```
    pub fn new(max_bytes: u64) -> Self {
        Self {
            capacity: max_bytes,
            current_usage: Arc::new(AtomicU64::new(0)),
        }
    }

    /// # Increment Memory Usage
    ///
    /// Atomically records an increase in memory usage (e.g., when a new message
    /// is broadcast to clients). This is the primary method for "checking in" with
    /// the guard before committing to a memory-intensive operation.
    ///
    /// ## `Ordering::Relaxed`
    /// We use `Relaxed` ordering because we do not need to synchronize other memory
    /// operations around this atomic update. We are only interested in the eventual
    /// consistency of the `current_usage` counter itself. This provides the best
    /// performance as it imposes the fewest restrictions on the CPU's instruction
    /// reordering.
    ///
    /// # Returns
    /// - `true` if the new total usage is within the `capacity`.
    /// - `false` if the new total usage has exceeded the `capacity`, signaling to the
    ///   caller that corrective action (like eviction) is required.
    pub fn increment(&self, bytes: u64) -> bool {
        // `fetch_add` returns the *previous* value before the addition.
        let prev = self.current_usage.fetch_add(bytes, Ordering::Relaxed);
        // The check must be against the value *after* the addition.
        prev + bytes <= self.capacity
    }

    /// # Decrement Memory Usage
    ///
    /// Atomically records a decrease in memory usage (e.g., after a client has
    /// successfully processed a message from its queue).
    ///
    /// Like `increment`, this uses `Ordering::Relaxed` for maximum performance, as
    /// we only care about the correctness of the counter itself.
    pub fn decrement(&self, bytes: u64) {
        // `fetch_sub` ensures the subtraction is atomic. We don't need the previous
        // value here, so we ignore the result.
        self.current_usage.fetch_sub(bytes, Ordering::Relaxed);
    }

    /// Returns the current total estimated memory usage in bytes.
    ///
    /// Performs an atomic `load` with `Relaxed` ordering.
    pub fn current_usage(&self) -> u64 {
        self.current_usage.load(Ordering::Relaxed)
    }

    /// Returns the maximum capacity (in bytes) of this guard.
    pub fn capacity(&self) -> u64 {
        self.capacity
    }

    /// Calculates the current memory usage as a percentage of the total capacity.
    /// Useful for monitoring and diagnostics.
    pub fn usage_percent(&self) -> f64 {
        let current = self.current_usage() as f64;
        let total = self.capacity as f64;
        if total == 0.0 {
            0.0
        } else {
            (current / total) * 100.0
        }
    }
}
