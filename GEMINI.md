# Project: rsdev

## Overview
This repository contains various Rust utilities and monitoring tools designed for reliability and cloud infrastructure monitoring.

## Key Components

### `misc` Crate
Located in `misc/`, this crate contains standalone binaries for monitoring services.

#### 1. `monitor_postgres`
Monitors PostgreSQL instances defined in a central cloud configuration.
- **Purpose:** Ensures database availability and sends alerts upon failure.
- **Key Features:**
  - Supports multiple providers (Supabase, Neon, Aiven, Local).
  - Handles both connection strings (`dbUrl`) and connection objects (`dbConnection`).
  - Sends JSON-formatted alerts to Primary and Failover webhook endpoints.
- **Critical Implementation Details:**
  - **PgBouncer Compatibility:** Uses `simple_query` API to bypass prepared statements, which are incompatible with transaction poolers (e.g., Supabase port 6543).
  - **Unified TLS Strategy:**
    - Uses `rustls` for all connections to ensure cross-platform consistency and modern security.
    - **Supabase Specifics:**
      - Enforces `ssl_mode=require`.
      - Uses a custom `NoVerifier` to disable certificate verification for Supabase connections. This is a pragmatic solution to bypass root certificate trust issues on Windows/Linux environments while maintaining encryption.
    - **Standard Connections:**
      - Uses `rustls` with `webpki-roots` and `rustls-native-certs` for full verification.
      - Enforces ALPN `postgresql`.

#### 2. `monitor_redis`
Monitors Redis instances.
- **Purpose:** Verifies Redis connectivity and responsiveness.
- **Key Features:**
  - Performs `PING` checks.
  - Updates a `LASTCHECKED` timestamp key.
  - Masks passwords in logs for security.

#### 3. `monitor_net`
Monitors local internet connectivity.
- **Purpose:** Detects network outages and provides audible feedback.
- **Key Features:**
  - Checks connectivity to Cloudflare (1.1.1.1:53).
  - Plays `Disconnected.wav` via `rodio` upon connection loss.
  - Logs outage duration.

### `servers` Crate
Located in `servers/`, this crate contains server applications.

#### 1. `server_yahoo`
A standalone WebSocket proxy server for Yahoo Finance streaming data.
- **Purpose:** Multiplexes a single Yahoo Finance WebSocket connection to multiple downstream clients.
- **Key Features:**
  - **Single Upstream Connection:** Maintains one connection to Yahoo to avoid rate limits.
  - **Robust Connection Handling:** Implements exponential backoff for reconnections and heartbeat detection to manage stale connections.
  - **Client Command Acknowledgments:** Provides a request-response mechanism for client `subscribe`/`unsubscribe` commands, ensuring clients receive proper acknowledgments.
  - **Client Isolation:** Each client manages its own subscriptions independently.
  - **Efficient Broadcasting:** Decodes Protobuf messages once and broadcasts them to interested clients.
  - **Graceful Shutdown:** Handles SIGINT/SIGTERM to close connections cleanly.
  - **Logging:** Uses `fern` for file-based logging with daily rotation.

## Shared Libraries
- **`lib_common`**: Contains shared logic and utilities, now organized into specific modules for better maintainability and reusability.
  - **Cloud Configuration (`config_cloud`):** Primarily for loading and parsing encrypted cloud configuration JSON.
  - **System Configuration (`config_sys`):** Handles runtime configuration loading from multiple sources.
  - **Logging (`loggers/logrecord`, `loggers/loggerlocal`):** Provides structured logging capabilities based on `Logrecord` and a local logger (mimicking JavaScript's `LoggerLocal.mjs`) with features like colored TTY output, text-to-speech notifications, and timestamped file logging with rotation.
  - **System Information (`utils/misc/sys_info`):** Retrieves process and host-specific information.
  - **General Utilities (`utils/misc/utils`):** Contains common helper functions like datetime formatting.
  - **Dependency Management:** Adheres to workspace best practices with centralized dependency versioning in the top-level `Cargo.toml`.

## Tech Stack
- **Runtime:** `tokio` (Async I/O)
- **Database:** `tokio-postgres`, `redis`
- **TLS:** `rustls`, `tokio-postgres-rustls`
- **Logging:** `fern`, `log`
- **HTTP Client:** `reqwest`
- **Audio:** `rodio`
- **WebSockets:** `tokio-tungstenite`, `axum`
- **Protobuf:** `prost`
