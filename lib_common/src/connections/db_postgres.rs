//! # PostgreSQL Connection Manager
//!
//! Provides a managed connection pool for PostgreSQL using the `sqlx` crate.
//! Supports connection pooling, health checks, and basic query execution.

use sqlx::postgres::{PgPool, PgPoolOptions};
use std::time::Duration;
use thiserror::Error;

/// Custom error types for Database operations.
#[derive(Debug, Error)]
pub enum DbError {
    #[error("Failed to connect to database: {0}")]
    ConnectionError(String),
    #[error("Query execution failed: {0}")]
    QueryError(String),
}

/// A wrapper around the PostgreSQL connection pool.
pub struct Database {
    /// The underlying sqlx connection pool.
    pub pool: PgPool,
}

impl Database {
    /// Creates a new connection pool for the specified database URL.
    ///
    /// # Arguments
    /// * `database_url` - The full connection string (e.g., "postgres://user:pass@host/db").
    /// * `max_connections` - Maximum number of concurrent connections in the pool.
    pub async fn new(database_url: &str, max_connections: u32) -> Result<Self, DbError> {
        // // Explicitly use PgPoolOptions to resolve type inference ambiguities
        let pool = PgPoolOptions::new()
            .max_connections(max_connections)
            .acquire_timeout(Duration::from_secs(3))
            .connect(database_url)
            .await
            .map_err(|e: sqlx::Error| DbError::ConnectionError(e.to_string()))?;

        Ok(Self { pool })
    }

    /// Checks the health of the database connection by running a simple query.
    pub async fn ping(&self) -> Result<(), DbError> {
        // // Execute a raw SQL health check
        sqlx::query("SELECT 1")
            .execute(&self.pool)
            .await
            .map_err(|e: sqlx::Error| DbError::QueryError(e.to_string()))?;
            
        Ok(())
    }
}