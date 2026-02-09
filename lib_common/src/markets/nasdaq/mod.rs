//! # NASDAQ API Integration Module
//!
//! This module provides a dedicated interface for interacting with various
//! NASDAQ APIs, primarily focusing on market status and data feeds. It encapsulates
//! the client logic and data structures required to fetch and process data
//! from these critical financial market sources.
//!
//! ## Contained Modules:
//!
//! - **`apicall`**: Implements the low-level HTTP client for NASDAQ APIs,
//!   handling request building, browser-mimicking headers, retry logic, and
//!   NASDAQ-specific error handling (e.g., `rCode` status within JSON responses).
//!
//! - **`datafeeds`**: (Planned/Placeholder) This module is intended to contain
//!   logic for interacting with specific NASDAQ data feeds, such as real-time
//!   quotes or historical data.
//!
//! - **`marketstatus`**: Defines the data models and higher-level client logic
//!   specifically for querying the NASDAQ market's open/closed status, which is
//!   crucial for orchestrating data ingestion workflows.
//!
//! The goal of this module is to provide a robust and structured way to
//! integrate NASDAQ data into the `rsdev` ecosystem, ensuring reliable
//! data ingestion and normalization.

#![doc(html_logo_url = "https://example.com/logo.png")] // Placeholder
#![forbid(unsafe_code)]
#![warn(missing_docs, rust_2018_idioms, unused_qualifications)]

/// Client for making HTTP requests to NASDAQ APIs, with browser-mimicking headers and retry logic.
pub mod apicall;
/// (Placeholder) Logic for interacting with specific NASDAQ data feeds.
pub mod datafeeds;
/// Data models and client logic for querying NASDAQ market status.
pub mod marketstatus;