# Development Rules & Guidelines

## General Philosophy
- **Reliability First:** Monitoring tools must be more robust than the systems they monitor. Handle errors gracefully and log them clearly.
- **Async by Default:** Use `tokio` for all I/O-bound tasks to ensure responsiveness, especially when monitoring multiple services.
- **Secure Logging:** Never log raw connection strings or passwords. Always use masking functions (e.g., `mask_url_password`).

## PostgreSQL Monitoring (`monitor_postgres`)
1.  **Connection Protocol:**
    - ALWAYS use `client.simple_query()` for health checks.
    - NEVER use `client.query_one()` or other methods that rely on prepared statements.
    - **Reason:** Supabase and other cloud providers often use PgBouncer in "transaction" mode, which does not support prepared statements.

2.  **TLS / SSL Handling:**
    - **Supabase:** Must use `native-tls` with `danger_accept_invalid_certs(true)`.
        - *Context:* Windows trust stores often reject Supabase's certificate chain. Disabling verification is a pragmatic compromise for this specific monitoring context to ensure connectivity.
    - **Other Providers:** Use `rustls` with `ssl_mode=require`.
        - Configure ALPN to `postgresql`.
        - Load system native certs (`rustls-native-certs`) and WebPKI roots (`webpki-roots`).

3.  **Configuration:**
    - Prioritize `dbUrl` (connection string) over `dbConnection` objects.
    - Respect the `active` flag in the configuration.

## Redis Monitoring (`monitor_redis`)
- Use the `redis` crate with `tokio` compatibility.
- Perform a write operation (e.g., `SET LASTCHECKED`) in addition to `PING` to verify write availability.

## Network Monitoring (`monitor_net`)
- Use a reliable, high-availability target for connectivity checks (e.g., Cloudflare DNS `1.1.1.1`).
- Audio alerts should be non-blocking. Run audio playback in `tokio::task::spawn_blocking`.

## Logging
- Use `fern` for logging configuration.
- Log to both `stderr` and a file (e.g., `monitor_postgres.log`).
- Format: `[YYYY-MM-DD HH:MM:SS][Target][Level] Message`

## Dependencies
- Prefer pure-Rust implementations where possible (e.g., `rustls`), but fall back to native bindings (`native-tls`) when platform-specific compatibility issues arise (as seen with Supabase on Windows).
