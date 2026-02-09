//! # Data Ingestors Module
//!
//! This module serves as the central hub for all data ingestion clients in the
//! `rsdev` project. Each submodule represents a specific client for a particular
//! data source, handling the unique logic required to connect to, receive data from,
//! and manage the lifecycle of that source.
//!
//! ## Purpose:
//! The primary role of the `ingestors` module is to abstract the complexities of
//! different data source protocols (e.g., WebSocket, REST API polling) behind a
//! consistent interface. These ingestors are the "front door" for all external
//! market data entering the system.
//!
//! ## Contained Modules:
//! - **`yahoo_wss`**: A resilient, state-aware WebSocket client for the real-time
//!   Yahoo Finance protobuf stream.
//! - **`cnn_polling`**: A self-scheduling REST client for periodically polling
//!   data, such as the CNN Fear & Greed Index.
//!
//! This file also re-exports the primary structs (e.g., `YahooWssIngestor`,
//! `CnnPollingPlugin`) to provide a clean public API for other parts of the
//! application, allowing them to be easily accessed via `lib_common::ingestors::...`.

#![doc(html_logo_url = "https://example.com/logo.png")] // Placeholder
#![forbid(unsafe_code)]
#![warn(missing_docs, rust_2018_idioms, unused_qualifications)]

/// The WebSocket client for Yahoo Finance's real-time data stream.
pub mod yahoo_wss;
/// The self-scheduling REST client for the CNN Fear & Greed Index.
pub mod cnn_polling;

// --- Public API Re-exports ---
// Make the primary ingestor structs directly accessible under the `ingestors` namespace.
pub use yahoo_wss::{YahooWssIngestor, YahooConfig};
pub use cnn_polling::CnnPollingPlugin;
