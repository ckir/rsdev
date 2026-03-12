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
    /// Error occurring when failing to establish a connection to the database.
    #[error("Failed to connect to database: {0}")]
    ConnectionError(String),
    /// Error occurring during the execution of a SQL query.
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
    ///
    /// # Errors
    /// Returns a `DbError::ConnectionError` if the pool cannot be initialized.
    pub async fn new(database_url: &str, max_connections: u32) -> Result<Self, DbError> {
        // Explicitly use PgPoolOptions to resolve type inference ambiguities
        let pool = PgPoolOptions::new()
            .max_connections(max_connections)
            .acquire_timeout(Duration::from_secs(3))
            .connect(database_url)
            .await
            .map_err(|e: sqlx::Error| DbError::ConnectionError(e.to_string()))?;

        Ok(Self { pool })
    }

    /// Checks the health of the database connection by running a simple query.
    ///
    /// # Errors
    /// Returns a `DbError::QueryError` if the ping query fails.
    pub async fn ping(&self) -> Result<(), DbError> {
        // Execute a raw SQL health check
        sqlx::query("SELECT 1")
            .execute(&self.pool)
            .await
            .map_err(|e: sqlx::Error| DbError::QueryError(e.to_string()))?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_database_connection_failure() {
        // // Test with an invalid URL to ensure it returns a ConnectionError
        let result = Database::new(
            "postgres://invalid_user:invalid_pass@localhost/invalid_db",
            5,
        )
        .await;
        assert!(result.is_err());
        match result {
            Err(DbError::ConnectionError(_)) => (),
            _ => panic!("Expected ConnectionError"),
        }
    }
}
