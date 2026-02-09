//! # CNN Business API Integration Module
//!
//! This module provides a dedicated interface for interacting with various
//! CNN Business APIs, primarily focusing on financial indicators such as the
//! Fear & Greed Index. It encapsulates the client logic and data structures
//! required to fetch and process data from these sources.
//!
//! ## Contained Modules:
//!
//! - **`apicallcnn`**: Implements the low-level HTTP client for CNN Business APIs,
//!   handling request building, retry logic, and error handling.
//!
//! - **`fearandgreed`**: Defines the data models and higher-level client logic
//!   specifically for the CNN Fear & Greed Index, including data parsing and
//!   timestamp normalization.
//!
//! The goal of this module is to provide a robust and structured way to
//! integrate CNN Business data into the `rsdev` ecosystem, ensuring reliable
//! data ingestion and normalization.

#![doc(html_logo_url = "https://example.com/logo.png")] // Placeholder
#![forbid(unsafe_code)]
#![warn(missing_docs, rust_2018_idioms, unused_qualifications)]

/// Client for making HTTP requests to CNN Business APIs.
pub mod apicallcnn;
/// Data models and client logic for the CNN Fear & Greed Index.
pub mod fearandgreed;