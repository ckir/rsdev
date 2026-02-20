//! # lib_common
//!
//! The central utility library for the rsdev workspace.
//! This library uses a modular, folder-based structure gated by features.

// // Warn if dependencies are included but not used by any active features
#![warn(unused_crate_dependencies)]

// // --- Explicitly Link Crate Dependencies to Satisfy Lints ---
// // These statements ensure the compiler recognizes the crates are intentionally 
// // included when their corresponding features are enabled.

#[cfg(feature = "loggers")]
use fern as _;
#[cfg(feature = "configs")]
use json5 as _;
#[cfg(feature = "loggers")]
use log as _;
#[cfg(feature = "utils")]
use sha2 as _;
#[cfg(feature = "connections")]
use tokio_postgres as _;
#[cfg(feature = "retrieve")]
use tokio_tungstenite as _;
#[cfg(feature = "loggers")]
use tracing as _;
#[cfg(feature = "retrieve")]
use url as _;
#[cfg(feature = "utils")]
use uuid as _;

// // --- Modular Exports ---

/// Configuration loading and parsing modules (Cloud and System).
#[cfg(feature = "configs")]
pub mod configs;

/// Database and Cache connection management modules (Postgres, Redis).
#[cfg(feature = "connections")]
pub mod connections;

/// Logging and tracing initialization modules.
#[cfg(feature = "loggers")]
pub mod loggers;

/// Market-specific API implementations (Nasdaq, CNN).
#[cfg(feature = "markets")]
pub mod markets;

/// Generic HTTP retrieval utilities.
#[cfg(feature = "retrieve")]
pub mod retrieve;

/// Miscellaneous system utilities and helper functions.
#[cfg(feature = "utils")]
pub mod utils;