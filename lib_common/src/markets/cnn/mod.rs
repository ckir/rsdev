//! # CNN Market Data Modules
//!
//! Provides implementations for CNN-specific market data APIs, 
//! including the Fear and Greed index.

/// API client for CNN data sources.
pub mod apicallcnn;

/// Fear and Greed index retrieval and normalization.
pub mod fearandgreed;
