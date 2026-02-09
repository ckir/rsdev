//! # Core Engine Module
//!
//! This module forms the heart of the `rsdev` real-time data processing engine.
//! It aggregates all the fundamental components required for managing data flow,
//! system state, and resource consumption. The components in this module are
//! designed to be high-performance, asynchronous, and thread-safe.
//!
//! ## Core Components:
//!
//! - **`dispatcher`**: The central, zero-copy broadcaster. It takes incoming data
//!   frames and efficiently distributes them to all subscribed clients (e.g.,
//!   WebSocket connections) with minimal overhead.
//!
//! - **`registry`**: A sophisticated subscription manager that uses a reference
//!   counting and "linger" mechanism. It prevents the rapid setup and teardown
//!   of upstream connections when clients quickly subscribe and unsubscribe to the
//!   same data source.
//!
//! - **`memory_guard`**: A system-wide memory management utility. It tracks the
//!   global memory footprint of data being processed and can enforce limits to
//!   prevent out-of-memory errors during periods of high data volume.
//!
//! - **`upstream_manager`**: The "brain" of the system. It monitors market status
//!   and orchestrates the overall `OperationMode` of the application, transitioning
//!   between `Streaming`, `FailoverPolling`, and `Idle` states.
//!
//! By declaring and re-exporting these components, the `core` module provides a
//! unified and clean public API for other parts of the application (like the
//! `servers` crate) to interact with the main engine.

#![doc(html_logo_url = "https://example.com/logo.png")] // Placeholder
#![forbid(unsafe_code)]
#![warn(missing_docs, rust_2018_idioms, unused_qualifications)]

/// Manages upstream subscriptions with a reference-counted linger mechanism.
pub mod registry;
/// A global memory manager to prevent out-of-memory errors.
pub mod memory_guard;
/// The central, zero-copy broadcaster for distributing data frames.
pub mod dispatcher;
/// The main state machine that orchestrates the system's operational mode.
pub mod upstream_manager;

// --- Public API Re-exports ---
// Make the primary structs from the core modules directly accessible.
pub use registry::Registry;
pub use memory_guard::{GlobalMemoryGuard, ClientPriority};
pub use dispatcher::{Dispatcher, ReStreamFrame};
pub use upstream_manager::{UpstreamManager, OperationMode};
