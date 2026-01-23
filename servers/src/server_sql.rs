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

// Configuration struct using clap
#[derive(Parser, Debug)]
#[clap(
    author,
    version,
    about = "A Rust HTTP server to execute SQL queries against PostgreSQL."
)]
#[clap(long_about = None)]
struct AppConfig {
    #[clap(
        long,
        env = "DATABASE_URL",
        help = "PostgreSQL connection URL (e.g., postgres://user:pass@host:port/dbname)"
    )]
    db_url: String,

    #[clap(long, env = "PORT", default_value_t = 3000, help = "HTTP server port")]
    port: u16,
}

// Custom error type for the application
#[derive(Debug)]
enum AppError {
    DatabaseError(tokio_postgres::Error),
    PoolError(deadpool_postgres::PoolError),
    ConfigError(String),
    BodyUtf8Error(std::string::FromUtf8Error),
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, error_json) = match self {
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
type DbPool = Arc<Pool>;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize tracing subscriber for logging (reads RUST_LOG environment variable)
    let subscriber = FmtSubscriber::builder()
        .with_env_filter(EnvFilter::from_default_env())
        .finish();
    tracing::subscriber::set_global_default(subscriber)
        .expect("Failed to set global default tracing subscriber");

    // Parse command-line arguments and environment variables
    let app_config = AppConfig::parse();
    info!(
        "Configuration loaded: DB URL (hidden), Port: {}",
        app_config.port
    );

    // Create database configuration for deadpool-postgres
    let mut pg_pool_config = DeadpoolConfig::new();
    pg_pool_config.url = Some(app_config.db_url.clone()); // The URL itself
    pg_pool_config.manager = Some(ManagerConfig {
        recycling_method: RecyclingMethod::Fast, // Recommended for tokio-postgres
    });
    // Other pool settings like max_size, timeouts can be configured here on pg_pool_config.pool if needed.
    // pg_pool_config.pool = Some(deadpool_postgres::PoolConfig::new(10)); // Example: max_size = 10

    // Create database pool
    let pool = pg_pool_config
        .create_pool(Some(Runtime::Tokio1), NoTls)
        .map_err(|e| AppError::ConfigError(format!("Failed to create database pool: {}", e)))?;
    info!("Database connection pool created successfully.");

    // Build Axum application router
    // The pool is wrapped in Arc automatically by with_state if not already
    let app = Router::new()
        .route("/sql", post(execute_sql_handler))
        .with_state(Arc::new(pool));

    // Define server address
    let addr = SocketAddr::from(([0, 0, 0, 0], app_config.port));
    info!("Starting HTTP server on http://{}", addr);

    // Run the Axum server
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app.into_make_service()).await?;

    Ok(())
}

async fn execute_sql_handler(
    State(pool): State<DbPool>,
    body: Bytes, // Use Bytes for robust body handling, expects UTF-8 SQL string
) -> Result<impl IntoResponse, AppError> {
    let sql_body = String::from_utf8(body.to_vec()).map_err(AppError::BodyUtf8Error)?;
    let trimmed_sql = sql_body.trim();

    info!("Received SQL query (length: {} chars)", trimmed_sql.len());
    if trimmed_sql.len() > 200 {
        // Avoid logging very long queries directly
        debug!("Trimmed SQL query: {}...", &trimmed_sql[..200]);
    } else {
        debug!("Trimmed SQL query: {}", trimmed_sql);
    }

    if trimmed_sql.is_empty() {
        warn!("Received empty SQL query string after trimming.");
        // tokio-postgres's simple_query will error on an empty string.
        // This will be caught by map_err(AppError::DatabaseError) below.
        // Alternatively, one could return a specific BAD_REQUEST here:
        // return Err(AppError::ConfigError("Empty SQL query provided".to_string())); // Using ConfigError as a placeholder for generic bad request
    }

    // Get a client from the pool. Deadpool handles retries and reconnections.
    let client = pool.get().await.map_err(AppError::PoolError)?;
    debug!("Acquired DB client from pool.");

    // Execute the SQL statements using simple_query
    // simple_query can handle multiple SQL statements separated by semicolons.
    // It returns a Vec of messages, primarily Rows and CommandComplete.
    // Note: For SELECT queries, simple_query returns rows as arrays of strings,
    // without column names. If named columns are essential, a single SELECT statement
    // should be executed with `client.query()` instead, which requires more complex handling
    // if multiple, mixed-type statements are allowed in one request.
    let messages = client
        .simple_query(trimmed_sql)
        .await
        .map_err(AppError::DatabaseError)?;
    debug!(
        "SQL query executed, received {} messages from database.",
        messages.len()
    );

    let mut results_json: Vec<serde_json::Value> = Vec::new();

    for msg in messages {
        match msg {
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
            SimpleQueryMessage::CommandComplete(tag) => {
                results_json.push(json!({
                    "type": "CommandComplete",
                    "tag": tag // e.g., "INSERT 0 1", "SELECT 5", "UPDATE 10"
                }));
            }
            // simple_query is documented to primarily return Row and CommandComplete.
            // Other protocol messages (like ReadyForQuery) are usually filtered by the high-level API.
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
