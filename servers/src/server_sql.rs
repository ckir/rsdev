//! # SQL Execution HTTP Server
//!
//! A Rust HTTP server designed to execute SQL queries against a PostgreSQL database.
//! This server provides a `/sql` endpoint where clients can POST SQL statements,
//! which are then executed, and the results are returned as JSON.
//!
//! ## Key Features:
//! - **PostgreSQL Integration**: Connects to a PostgreSQL database using `deadpool_postgres`
//!   for efficient connection pooling and `tokio-postgres` for asynchronous query execution.
//! - **HTTP API**: Exposes a `POST /sql` endpoint that accepts raw SQL query strings
//!   in the request body.
//! - **Flexible Query Execution**: Utilizes `client.simple_query()` to handle multiple
//!   SQL statements, including DDL, DML, and simple `SELECT` queries, returning
//!   results as a JSON array of `RowData` and `CommandComplete` messages.
//! - **Robust Error Handling**: Implements a custom `AppError` enum and `IntoResponse`
//!   implementation to provide detailed and appropriate HTTP status codes and JSON
//!   error responses for various database, pool, configuration, and request body issues.
//! - **Configurable**: Database URL and server port are configurable via command-line
//!   arguments and environment variables using `clap`.
//! - **Structured Logging**: Integrates `tracing` for comprehensive logging of server
//!   operations, database interactions, and errors.
//!
//! This server acts as a powerful backend for applications requiring dynamic SQL
//! execution capabilities, particularly suitable for microservices architectures
//! or specialized data access layers.

use axum::{
    Json, Router,
    body::Bytes,
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::post,
};
use clap::Parser;
use deadpool_postgres::{Config as DeadpoolConfig, ManagerConfig, Pool, RecyclingMethod, Runtime};
use serde_json::json;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio_postgres::{NoTls, SimpleQueryMessage};
use tracing::debug;
use tracing::{error, info, warn};
use tracing_subscriber::{EnvFilter, FmtSubscriber};

/// # Application Configuration
///
/// Configuration struct for the SQL execution server, parsed from command-line arguments
/// and environment variables using `clap`.
#[derive(Parser, Debug)]
#[clap(
    author,
    version,
    about = "A Rust HTTP server to execute SQL queries against PostgreSQL."
)]
#[clap(long_about = None)]
struct AppConfig {
    /// PostgreSQL connection URL (e.g., postgres://user:pass@host:port/dbname).
    /// Can be provided via `--db-url` argument or `DATABASE_URL` environment variable.
    #[clap(
        long,
        env = "DATABASE_URL",
        help = "PostgreSQL connection URL (e.g., postgres://user:pass@host:port/dbname)"
    )]
    db_url: String,

    /// HTTP server port. Can be provided via `--port` argument or `PORT` environment variable.
    /// Defaults to 3000.
    #[clap(long, env = "PORT", default_value_t = 3000, help = "HTTP server port")]
    port: u16,
}

/// # Application Error
///
/// Custom error type for the SQL execution server, encompassing various
/// types of errors that can occur during operation.
#[derive(Debug)]
enum AppError {
    /// Error originating from `tokio_postgres`, related to database interaction.
    DatabaseError(tokio_postgres::Error),
    /// Error originating from `deadpool_postgres`, related to connection pool management.
    PoolError(deadpool_postgres::PoolError),
    /// Configuration-related errors, typically indicating invalid or missing settings.
    ConfigError(String),
    /// Error when the request body is not valid UTF-8.
    BodyUtf8Error(std::string::FromUtf8Error),
}

impl IntoResponse for AppError {
    /// Converts an `AppError` into an `axum::response::Response`, providing
    /// appropriate HTTP status codes and JSON error bodies to the client.
    fn into_response(self) -> Response {
        let (status, error_json) = match self {
            /// Handles `DatabaseError`s, extracting PostgreSQL-specific codes and messages
            /// to provide more granular error details and HTTP status codes.
            AppError::DatabaseError(e) => {
                error!("Database error: {:?}", e);
                if let Some(db_err) = e.as_db_error() {
                    let code = db_err.code().code();
                    let message = db_err.message().to_string();
                    let severity = db_err.severity().to_string();

                    let http_status = match code {
                        // Class 42: Syntax Error or Access Rule Violation
                        // Class 22: Data Exception
                        _ if code.starts_with("42") || code.starts_with("22") => {
                            StatusCode::BAD_REQUEST
                        }
                        // Class 23: Integrity Constraint Violation
                        _ if code.starts_with("23") => StatusCode::CONFLICT,
                        _ => StatusCode::INTERNAL_SERVER_ERROR,
                    };
                    (
                        http_status,
                        json!({
                            "error_type": "DatabaseExecutionError",
                            "message": message,
                            "pg_code": code,
                            "pg_severity": severity
                        }),
                    )
                } else {
                    // Error from tokio-postgres client library (e.g., empty query, connection issue)
                    let err_string = e.to_string();
                    let http_status = if err_string.contains("empty query")
                        || err_string.to_lowercase().contains("syntax error")
                    {
                        // Heuristic for client-side errors
                        StatusCode::BAD_REQUEST
                    } else {
                        StatusCode::INTERNAL_SERVER_ERROR
                    };
                    (
                        http_status,
                        json!({
                            "error_type": "DatabaseClientError",
                            "message": "Error communicating with database or processing query client-side.",
                            "detail": err_string
                        }),
                    )
                }
            }
            /// Handles `PoolError`s, indicating issues with acquiring a database connection from the pool.
            AppError::PoolError(e) => {
                error!("Database pool error: {}", e);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    json!({
                        "error_type": "DatabasePoolError",
                        "message": "Failed to get connection from pool. The database might be unavailable.",
                        "detail": e.to_string()
                    }),
                )
            }
            /// Handles `ConfigError`s, typically for invalid application or database configurations.
            AppError::ConfigError(e_msg) => {
                error!("Configuration error: {}", e_msg);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    json!({
                        "error_type": "ConfigurationError",
                        "message": "Invalid application or database configuration.",
                        "detail": e_msg
                    }),
                )
            }
            /// Handles `BodyUtf8Error`s, for when the request body is not valid UTF-8.
            AppError::BodyUtf8Error(e) => {
                error!("Request body UTF-8 error: {}", e);
                (
                    StatusCode::BAD_REQUEST,
                    json!({
                        "error_type": "InvalidRequestBody",
                        "message": "Request body is not valid UTF-8.",
                        "detail": e.to_string()
                    }),
                )
            }
        };
        (status, Json(error_json)).into_response()
    }
}

impl std::fmt::Display for AppError {
    /// Implements the `Display` trait for `AppError`, providing a user-friendly
    /// string representation of the error.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AppError::DatabaseError(e) => write!(f, "Database error: {}", e),
            AppError::PoolError(e) => write!(f, "Pool error: {}", e),
            AppError::ConfigError(s) => write!(f, "Configuration error: {}", s),
            AppError::BodyUtf8Error(e) => write!(f, "Body UTF-8 error: {}", e),
        }
    }
}

impl std::error::Error for AppError {
    /// Implements the `Error` trait for `AppError`, allowing it to be used
    /// with `?` operator and providing a source for error chaining.
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            AppError::DatabaseError(e) => Some(e),
            AppError::PoolError(e) => Some(e),
            AppError::ConfigError(_) => None,
            AppError::BodyUtf8Error(e) => Some(e),
        }
    }
}

// Type alias for the database pool, wrapped in Arc for sharing
/// Type alias for the database connection pool, wrapped in `Arc` for shared,
/// thread-safe access across asynchronous tasks.
type DbPool = Arc<Pool>;

#[tokio::main]
/// # Main Entry Point
///
/// Initializes and runs the SQL execution HTTP server.
///
/// This function performs the following steps:
/// 1.  Initializes the `tracing` subscriber for structured logging.
/// 2.  Parses application configuration from command-line arguments and environment variables.
/// 3.  Configures and creates a `deadpool_postgres` connection pool to the PostgreSQL database.
/// 4.  Builds the `axum` router, defining the `/sql` endpoint.
/// 5.  Starts the HTTP server, listening for incoming requests.
///
/// # Returns
/// An `anyhow::Result<()>` indicating success or failure of the server operation.
async fn main() -> anyhow::Result<()> {
    /// Initializes the `tracing` subscriber to process and format logs.
    /// It uses `EnvFilter` to set log levels based on the `RUST_LOG` environment variable.
    let subscriber = FmtSubscriber::builder()
        .with_env_filter(EnvFilter::from_default_env())
        .finish();
    tracing::subscriber::set_global_default(subscriber)
        .expect("Failed to set global default tracing subscriber");

    /// Parses command-line arguments and environment variables into `AppConfig`.
    let app_config = AppConfig::parse();
    info!(
        "Configuration loaded: DB URL (hidden), Port: {}",
        app_config.port
    );

    /// Creates a `deadpool_postgres` configuration, setting the database URL
    /// and connection manager recycling method.
    let mut pg_pool_config = DeadpoolConfig::new();
    pg_pool_config.url = Some(app_config.db_url.clone()); // The URL itself
    pg_pool_config.manager = Some(ManagerConfig {
        recycling_method: RecyclingMethod::Fast, // Recommended for tokio-postgres
    });
    // Other pool settings like max_size, timeouts can be configured here on pg_pool_config.pool if needed.
    // pg_pool_config.pool = Some(deadpool_postgres::PoolConfig::new(10)); // Example: max_size = 10

    /// Creates the PostgreSQL connection pool using the defined configuration.
    let pool = pg_pool_config
        .create_pool(Some(Runtime::Tokio1), NoTls)
        .map_err(|e| AppError::ConfigError(format!("Failed to create database pool: {}", e)))?;
    info!("Database connection pool created successfully.");

    /// Builds the `axum` application router, registering the `execute_sql_handler` for the `/sql` path.
    /// The database pool is shared across handlers using `with_state`.
    let app = Router::new()
        .route("/sql", post(execute_sql_handler))
        .with_state(Arc::new(pool));

    /// Defines the socket address for the HTTP server to listen on.
    let addr = SocketAddr::from(([0, 0, 0, 0], app_config.port));
    info!("Starting HTTP server on http://{}", addr);

    /// Starts the `axum` server, binding to the specified address and serving the application.
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app.into_make_service()).await?;

    Ok(())
}

/// # Execute SQL Handler
///
/// An `axum` handler for the `POST /sql` endpoint.
///
/// This handler receives SQL statements as raw bytes in the request body,
/// executes them against the PostgreSQL database using a connection from the pool,
/// and returns the results as a JSON array.
///
/// # Arguments
/// * `pool` - An `axum::extract::State` containing the shared `DbPool`.
/// * `body` - `axum::body::Bytes` representing the raw request body, expected to be a UTF-8 SQL string.
///
/// # Returns
/// A `Result` containing either an `impl IntoResponse` (HTTP 200 OK with JSON results)
/// or an `AppError` on failure.
async fn execute_sql_handler(
    State(pool): State<DbPool>,
    body: Bytes, // Use Bytes for robust body handling, expects UTF-8 SQL string
) -> Result<impl IntoResponse, AppError> {
    /// Converts the request body bytes to a UTF-8 string, returning an `AppError` if decoding fails.
    let sql_body = String::from_utf8(body.to_vec()).map_err(AppError::BodyUtf8Error)?;
    /// Trims whitespace from the SQL query string.
    let trimmed_sql = sql_body.trim();

    info!("Received SQL query (length: {} chars)", trimmed_sql.len());
    if trimmed_sql.len() > 200 {
        // Avoid logging very long queries directly
        debug!("Trimmed SQL query: {}...", &trimmed_sql[..200]);
    } else {
        debug!("Trimmed SQL query: {}", trimmed_sql);
    }

    /// Checks for an empty SQL query after trimming and logs a warning if found.
    if trimmed_sql.is_empty() {
        warn!("Received empty SQL query string after trimming.");
        // tokio-postgres's simple_query will error on an empty string.
        // This will be caught by map_err(AppError::DatabaseError) below.
        // Alternatively, one could return a specific BAD_REQUEST here:
        // return Err(AppError::ConfigError("Empty SQL query provided".to_string())); // Using ConfigError as a placeholder for generic bad request
    }

    /// Acquires a client connection from the `deadpool_postgres` pool.
    let client = pool.get().await.map_err(AppError::PoolError)?;
    debug!("Acquired DB client from pool.");

    /// Executes the SQL statements using `client.simple_query()`.
    /// This method can handle multiple SQL statements and returns a vector of `SimpleQueryMessage`s.
    let messages = client
        .simple_query(trimmed_sql)
        .await
        .map_err(AppError::DatabaseError)?;
    debug!(
        "SQL query executed, received {} messages from database.",
        messages.len()
    );

    /// Collects the processed query results into a JSON array.
    let mut results_json: Vec<serde_json::Value> = Vec::new();

    /// Iterates through the `SimpleQueryMessage`s, converting rows and command completions to JSON.
    for msg in messages {
        match msg {
            /// Handles `SimpleQueryMessage::Row`, converting row values into a JSON array of strings.
            SimpleQueryMessage::Row(row) => {
                let mut json_row_values = Vec::new();
                for i in 0..row.len() {
                    // SimpleRow provides access by index -> Option<&str>
                    json_row_values.push(row.get(i).map_or(serde_json::Value::Null, |s| {
                        serde_json::Value::String(s.to_string())
                    }));
                }
                results_json.push(json!({
                    "type": "RowData",
                    "values": serde_json::Value::Array(json_row_values)
                }));
            }
            /// Handles `SimpleQueryMessage::CommandComplete`, adding the command tag to the JSON results.
            SimpleQueryMessage::CommandComplete(tag) => {
                results_json.push(json!({
                    "type": "CommandComplete",
                    "tag": tag // e.g., "INSERT 0 1", "SELECT 5", "UPDATE 10"
                }));
            }
            // simple_query is documented to primarily return Row and CommandComplete.
            // Other protocol messages (like ReadyForQuery) are usually filtered by the high-level API.
            /// Logs a warning for any unexpected `SimpleQueryMessage` variants.
            _ => {
                // This case should ideally not be hit with simple_query's current implementation.
                warn!(
                    "Ignoring unexpected SimpleQueryMessage variant during processing: {:?}",
                    msg
                );
            }
        }
    }
    info!(
        "Processed {} relevant messages into JSON response.",
        results_json.len()
    );
    Ok((StatusCode::OK, Json(serde_json::Value::Array(results_json))))
}
