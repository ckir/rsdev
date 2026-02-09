# Development Rules & Guidelines

## General Philosophy
- **Reliability First:** Monitoring tools must be more robust than the systems they monitor. Handle errors gracefully and log them clearly.
- **Async by Default:** Use `tokio` for all I/O-bound tasks to ensure responsiveness, especially when monitoring multiple services.
- **Secure Logging:** Never log raw connection strings or passwords. Always use masking functions (e.g., `mask_url_password`).

## Local Development with `act`
- When running GitHub Actions workflows locally using `act`, use the `--artifact-server-path` flag to persist artifacts:
  ```bash
  act --artifact-server-path ./artifacts
  ```
  This will store generated artifacts in the specified directory (e.g., `./artifacts`).

## PostgreSQL Monitoring (`monitor_postgres`)
1.  **Connection Protocol:**
    - ALWAYS use `client.simple_query()` for health checks.
    - NEVER use `client.query_one()` or other methods that rely on prepared statements.
    - **Reason:** Supabase and other cloud providers often use PgBouncer in "transaction" mode, which does not support prepared statements.

2.  **TLS / SSL Handling:**
    - **Unified Backend:** Use `rustls` for all connections. Do NOT use `native-tls`.
    - **Supabase Strategy:**
        - Detect Supabase URLs (containing "supabase.co" or "supabase.com").
        - Force `ssl_mode=require`.
        - **Disable Certificate Verification:** Use a custom `NoVerifier` to bypass certificate validation. This is necessary to resolve persistent trust store issues on Windows/Linux environments while ensuring the connection remains encrypted.
    - **Standard Strategy:**
        - For non-Supabase connections, use standard `rustls` verification with `webpki-roots` and `rustls-native-certs`.
        - Enforce ALPN `postgresql`.

3.  **Configuration:**
    - Prioritize `dbUrl` (connection string) over `dbConnection` objects.
    - Respect the `active` flag in the configuration.

## Redis Monitoring (`monitor_redis`)
- Use the `redis` crate with `tokio` compatibility.
- Perform a write operation (e.g., `SET LASTCHECKED`) in addition to `PING` to verify write availability.

## Network Monitoring (`monitor_net`)
- Use a reliable, high-availability target for connectivity checks (e.g., Cloudflare DNS `1.1.1.1`).
- Audio alerts should be non-blocking. Run audio playback in `tokio::task::spawn_blocking`.

## Yahoo Finance Server (`server_yahoo`)
- **Architecture:**
    - Maintain a single upstream WebSocket connection to Yahoo.
    - Use `axum` for the downstream WebSocket server.
    - Use `tokio::sync::broadcast` to distribute decoded Protobuf messages.
    - Manage state (subscriptions) in a thread-safe `AppState` struct.
- **Protobuf:**
    - Use `prost` and `prost-build` for handling Yahoo's PricingData messages.
    - Ensure the `.proto` file is kept in sync with Yahoo's format.

## Logging
- Use `fern` for logging configuration.
- Log to both `stderr` and a daily rotating file (e.g., `monitor_postgres_YYYY-MM-DD.log`).
- Format: `[YYYY-MM-DD HH:MM:SS][Target][Level] Message`

## Dependencies
- Prefer pure-Rust implementations (`rustls`) over native bindings (`native-tls`) to ensure consistent behavior across platforms and avoid system dependency issues (like OpenSSL versions).

## Documentation Standards
- **Granular Comments (///):** As of [Δευτέρα 9 Φεβρουαρίου 2026], granular `///` documentation comments have been added to all public structs, their fields, and `Default` implementations within `common/logrecord.rs`. This provides detailed explanations for each item.
- **Top-level Comments (//!):** All `.rs` files within the `lib_common` and `servers` crates have been reviewed, and top-level `//!` documentation comments have been added where missing, summarizing the overall purpose and functionality of each module.