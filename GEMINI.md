# Project: rsdev

## Overview
This repository contains various Rust utilities and monitoring tools designed for reliability, high-performance market data distribution, and cloud infrastructure monitoring.

## Key Components

### `lib_common` (The Core Engine)
The heart of the workspace, providing a high-performance framework for real-time data.

#### 1. `core` Module (The Gateway Chassis)
- **Registry (`registry.rs`):** Manages symbol lifecycle with reference counting and a "Linger" logic (using `CancellationToken`) to prevent redundant upstream resubscriptions.
- **Memory Guard (`memory_guard.rs`):** A lock-free `AtomicU64` tracker that monitors global heap consumption across all client buffers to prevent OOM (Out of Memory) crashes.
- **Dispatcher (`dispatcher.rs`):** A Zero-Copy fan-out engine using `Arc<T>` to broadcast messages to multiple clients with priority-based eviction for slow consumers.
- **Upstream Manager (`upstream_manager.rs`):** The "Conductor" that monitors market hours and coordinates failover between Streaming and Polling modes.

#### 2. `ingestors` Module (Data Acquisition)
- **Yahoo WSS:** High-speed WebSocket client with Protobuf v2 decoding and "Silent Failure" detection via inactivity timeouts.
- **CNN Polling:** A self-scheduling REST client for macro indicators (e.g., Fear & Greed) that determines its own next-poll interval based on volatility.

### `servers` Crate
Located in `servers/`, this crate contains the gateway and proxy applications.

#### 1. `restream` (Binary: `servers/src/restream.rs`)
The flagship data distribution gateway.
- **Purpose:** Centralizes market data flow to avoid upstream rate limits and provides low-latency distribution to local internal tools.
- **Telemetry:** Implements **Triple-Timestamping** (`ts_upstream`, `ts_library_in`, `ts_library_out`) for microsecond-level performance profiling.

#### 2. `server_yahoo`
A standalone WebSocket proxy for legacy Yahoo Finance streaming support.

### `misc` Crate
Located in `misc/`, contains standalone monitoring binaries.

#### 1. `monitor_postgres`
- **PgBouncer Compatibility:** Uses `simple_query` to support transaction poolers (Supabase/Neon).
- **Unified TLS:** Standardizes on `rustls` with custom verifiers for Supabase to ensure cross-platform encryption consistency.

#### 2. `monitor_redis`
Verifies Redis connectivity, performs `PING` checks, and updates heartbeat timestamps.

#### 3. `monitor_net`
Detects network outages with audible feedback via `rodio` and logs downtime duration.

## Shared Libraries (Utilities)
- **Cloud Config:** Encrypted JSON configuration loading for secure environment management.
- **Logging:** Structured logging (`Logrecord`) with TTY colors, TTS notifications, and daily file rotation.
- **API Client:** Ergonomic HTTP client (`ky_http`) with built-in retries and structured `ApiResponse` handling.

## Tech Stack
- **Runtime:** `tokio` (Async I/O)
- **Serialization:** `serde`, `prost` (Protobuf)
- **Networking:** `tokio-tungstenite` (WSS), `reqwest` (HTTP)
- **Database:** `tokio-postgres`, `redis`
- **TLS:** `rustls`, `tokio-postgres-rustls`
- **Audio:** `rodio`