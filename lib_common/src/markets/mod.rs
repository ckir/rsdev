//! # Financial Market APIs Module
//!
//! This module groups together all logic and client implementations related to
//! specific financial market data providers and their APIs. Its purpose is to
//! abstract the details of interacting with external market services, providing
//! normalized data or status information to the rest of the system.
//!
//! ## Contained Modules:
//!
//! - **`nasdaq`**: Contains client implementations and data models for fetching
//!   market status and other data from the NASDAQ API. This includes logic for
//!   browser-mimicking headers and robust retry mechanisms.
//!
//! - **`cnn`**: Houses clients and data structures for interacting with CNN
//!   Business APIs, such as fetching the Fear & Greed Index.
//!
//! By centralizing these market-specific clients, this module ensures a clean
//! separation of concerns and facilitates easier integration of new data providers
//! in the future.

#![doc(html_logo_url = "https://example.com/logo.png")] // Placeholder
#![forbid(unsafe_code)]
#![warn(missing_docs, rust_2018_idioms, unused_qualifications)]

/// Client for interacting with NASDAQ APIs, including market status.
pub mod nasdaq;
/// Client for interacting with CNN Business APIs, such as the Fear & Greed Index.
pub mod cnn;