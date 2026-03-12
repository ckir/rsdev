//! # Logging and Tracing Modules
//!
//! This module provides structured logging utilities, including a standardized
//! log record format and local logger initializers.

/// Standard log record structure for cross-service logging.
pub mod logrecord;

/// Local logger initialization and management.
pub mod loggerlocal;
