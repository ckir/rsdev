//! # Connections Module
//!
//! This module handles persistent connections to external services 
//! including databases and caching layers.

/// Module for PostgreSQL database connection pooling and management.
pub mod db_postgres;

/// Module for Redis cache operations and connection handling.
pub mod cache_redis;