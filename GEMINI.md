# rsdev2: Unified Market Data Gateway

## Project Overview
A high-performance, multi-tenant market data hub built in Rust. The system ingests real-time financial data, processes it with zero-copy efficiency, and redistributes it via secure WebSockets (WSS). It features a sophisticated backpressure system and global memory management to maintain stability during high-volatility market events.



## Architecture: The "Slim" Engine
The project is organized into a modular workspace to separate core logic, edge delivery, and diagnostic tooling.

### 1. lib_common (The Brain)
The foundational crate housing all business logic and infrastructure.
- core/dispatcher.rs: High-speed async broadcaster using MPSC channels.
- core/memory_guard.rs: Real-time tracking of global memory footprint with priority-based eviction.
- core/registry.rs: Centralized state management for upstream health and client sessions.
- ingestors/: Provider-specific workers (e.g., Yahoo) that normalize raw streams.

### 2. servers (The Edge & Delivery Layer)
The public-facing gateway and specialized delivery services.
- restream.rs: The primary production server. Integrates Axum, Rustls (TLS), and the Dispatcher.
- redis2ws.rs: A bridge utility that subscribes to Redis Pub/Sub channels and broadcasts to WebSockets.
- server_sql.rs / server_log.rs: Data persistence and logging proxies for streaming data to databases or long-term storage.
- server_speak.rs: System-to-audio engine (likely for real-time market alerts or text-to-speech notifications).
- server_dummy.rs: A testing/mocking server for local development without live market feeds.



### 3. cli (The Tooling Suite)
Administrative utilities for configuration management and security.
- dir-to-yaml.rs / j5-to-yaml.rs: Configuration transpilers.
- j5-format.rs / j5-to-json.rs: JSON5 validation and normalization tools.
- rs_encrypt.rs: Encryption/decryption for sensitive configuration secrets.
- js-paths.rs / zip.rs: Path resolution and deployment bundling utilities.

### 4. misc (Diagnostics & Monitoring)
Specialized tools for system health and hardware verification.
- monitor_net.rs: Real-time network throughput and latency tracking.
- monitor_postgres.rs / monitor_redis.rs: Health checks and connection state monitoring.
- audio_test.rs: Hardware verification for system alerts.



### 5. vendor (The Stability Layer)
In-house copies of critical dependencies to ensure build reproducibility.
- json5format: Local JSON5 parsing engine used by CLI tools.
- pmdaemon: Process monitoring logic to ensure 24/7 server uptime.

## Data Lifecycle
1. INGRESS: Ingestors in lib_common connect to upstream providers and normalize data.
2. GUARD: GlobalMemoryGuard validates the impact of the data frame on system resources.
3. DISPATCH: Dispatcher broadcasts the frame via zero-copy Arcs to active channels.
4. EGRESS: Axum WebSocket handlers (or redis2ws bridge) stream the data to end-clients over WSS.



## Key Developer Workflows

# Build everything
cargo build --release

# Launch the Secure Gateway
cargo run -p servers --bin restream

# Start the Redis-to-WS Bridge
cargo run -p servers --bin redis2ws

# Encrypt Secrets for Config
cargo run -p cli --bin rs_encrypt -- --encrypt "my_database_password"

## Implementation Notes
- Crypto Provider: For Rustls 0.23, main() must initialize the 'ring' provider.
- Timezone: The system is NY-Time aware (US/Eastern) for market status and restarts.
- Platform Support: Fully compatible with Windows (development) and Linux (production).