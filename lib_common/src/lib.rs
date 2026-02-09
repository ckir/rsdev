//! # `lib_common`: The Core Infrastructure and Business Logic Hub
//!
//! `lib_common` is the foundational crate for the `rsdev` project. It houses all the
//! essential, reusable components, business logic, and infrastructure services
//! that are shared across the various binaries (servers, CLIs, tools) in the workspace.
//!
//! The primary goal of this crate is to enforce separation of concerns, keeping the
//! core, platform-agnostic logic isolated from the presentation/delivery layers (like
//! the Axum web server or CLI tools).
//!
//! ## Key Modules and Responsibilities:
//!
//! - **`core`**: The heart of the real-time engine.
//!   - `dispatcher`: A high-performance, zero-copy message broadcaster.
//!   - `registry`: Manages the state and health of upstream data sources.
//!   - `upstream_manager`: Orchestrates the system's operational modes (e.g., `Streaming` vs. `Idle`).
//!   - `memory_guard`: A global memory manager to prevent OOM errors during high-volume periods.
//!
//! - **`ingestors`**: A collection of data source clients.
//!   - `yahoo_wss`: A resilient WebSocket client for Yahoo Finance's real-time stream.
//!   - `cnn_polling`: A self-scheduling REST client for polling data (e.g., CNN Fear & Greed).
//!
//! - **`markets`**: Modules for interacting with specific financial market APIs.
//!   - `nasdaq`: Tools for checking NASDAQ market status.
//!   - `cnn`: Tools for fetching data from CNN Business APIs.
//!
//! - **`loggers`**: Provides structured, application-wide logging services.
//!   - `loggerlocal`: An async logger for sending logs to a central service or file.
//!
//! - **`config_cloud` / `config_sys`**: Utilities for loading configuration from remote
//!   services and the local system.
//!
//! - **`retrieve`**: Generic HTTP clients and utilities for data retrieval.
//!   - `ky_http`: A pre-configured `reqwest` client.
//!
//! - **`utils`**: A collection of miscellaneous utility functions, including system
//!   information gathering and alert mechanisms.
//!
//! By re-exporting key structs and functions at the top level, `lib_common` provides a
//! clean, public API for other crates in the workspace to consume.

#![doc(html_logo_url = "https://example.com/logo.png")] // Placeholder
#![forbid(unsafe_code)]
#![warn(missing_docs, rust_2018_idioms, unused_qualifications)]

// Re-export the `beep` function for creating audible alerts.
pub use actually_beep::beep_with_hz_and_millis;

// --- Module Declarations ---
// These declarations define the module tree of the library.

/// Configuration loading from remote (cloud) sources.
pub mod config_cloud;
/// Configuration loading from the local system environment.
pub mod config_sys;
/// Asynchronous logging infrastructure.
pub mod loggers;
/// Miscellaneous utilities, including system info and alerts.
pub mod utils;
/// Generic data retrieval clients (e.g., HTTP).
pub mod retrieve;
/// Logic specific to financial market APIs (e.g., NASDAQ, CNN).
pub mod markets;
/// The core real-time engine components (dispatcher, registry, etc.).
pub mod core;
/// Data source clients (e.g., WebSocket ingestors, REST pollers).
pub mod ingestors;


// --- Public API Re-exports ---
// This section makes key components from the modules above easily accessible
// to other crates that depend on `lib_common`. For example, a server binary can
// use `lib_common::get_cloud_config()` instead of needing to know the internal
// module path `lib_common::config_cloud::get_cloud_config()`.

pub use config_cloud::*;
pub use config_sys::*;
pub use loggers::logrecord::*;
pub use loggers::loggerlocal::*;
pub use utils::misc::sys_info::*;
pub use utils::misc::utils::*;
pub use retrieve::ky_http::*;
pub use markets::nasdaq::apicall::*;
pub use markets::cnn::fearandgreed::*;


/// The semantic version of the `lib_common` crate, pulled from `Cargo.toml`.
/// This is useful for logging the version on startup or for diagnostics.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
