//! # Nasdaq Market Data Modules
//!
//! Provides implementations for Nasdaq-specific market information,
//! including operational status and API connectivity.

/// API client for Nasdaq data sources.
pub mod apicall;

/// Real-time and status tracking for the Nasdaq market.
pub mod marketstatus;

/// Data feed implementations (e.g., Yahoo Finance).
pub mod datafeeds;
