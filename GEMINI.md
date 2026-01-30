# Project: rsdev

## Overview
This repository contains various Rust utilities and monitoring tools designed for reliability and cloud infrastructure monitoring.

## Key Components

### `misc` Crate
Located in `misc/`, this crate contains standalone binaries for monitoring services.

### `cli` Crate
Located in `cli/`, this crate contains various command-line utilities.

#### 1. `dir-to-yaml`
Exports a directory structure to YAML.
- **Purpose:** Provides a simple way to visualize directory structures.
- **Key Features:**
  - Exports to YAML.
  - Supports excluding files (`--no-files`).
  - Supports ignoring files from `.gitignore` (`--use-gitignore`).


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

## Shared Libraries
- **`lib_common`**: Contains shared logic, primarily for loading and parsing the cloud configuration JSON.

## Tech Stack
- **Runtime:** `tokio` (Async I/O)
- **Database:** `tokio-postgres`, `redis`
- **TLS:** `rustls`, `tokio-postgres-rustls`
- **Logging:** `fern`, `log`
- **HTTP Client:** `reqwest`
- **Audio:** `rodio`