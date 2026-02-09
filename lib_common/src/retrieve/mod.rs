//! # Data Retrieval Module
//!
//! This module provides a centralized location for generic data retrieval
//! clients and utilities, primarily focused on HTTP-based interactions.
//!
//! ## Purpose:
//! The goal of the `retrieve` module is to offer a consistent and robust way
//! to fetch data from external services, encapsulating common concerns such
//! as HTTP request building, error handling, and retry mechanisms. This
//! prevents duplication of networking logic across different ingestors or
//! API clients.
//!
//! ## Contained Modules:
//!
//! - **`ky_http`**: A generic HTTP `ApiClient` built on `reqwest` and
//!   `reqwest-middleware`, featuring automatic retries with exponential
//!   backoff. It serves as the foundation for many specific API clients
//!   (e.g., NASDAQ, CNN).
//!
//! By using the components within this module, other parts of the system
//! can focus on data parsing and business logic, delegating the complexities
//! of network communication to this well-tested and resilient layer.

#![doc(html_logo_url = "https://example.com/logo.png")] // Placeholder
#![forbid(unsafe_code)]
#![warn(missing_docs, rust_2018_idioms, unused_qualifications)]

/// Generic HTTP API client with retry middleware for resilient network requests.
pub mod ky_http;