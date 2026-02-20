//! # Redis Cache Implementation
//!
//! Provides a synchronous wrapper for Redis key-value operations.

use redis::{Client, Commands, RedisResult};

/// A handler for Redis cache interactions.
pub struct CacheHandler {
    /// The internal Redis client instance.
    pub client: Client,
}

impl CacheHandler {
    /// Creates a new CacheHandler from a connection string.
    ///
    /// # Arguments
    /// * `url` - The redis URL (e.g., "redis://127.0.0.1/").
    pub fn new(url: &str) -> RedisResult<Self> {
        // Open the connection to the redis server
        let client = Client::open(url)?;
        Ok(Self { client })
    }

    /// Stores a string value in the cache.
    pub fn set_string(&self, key: &str, value: &str) -> RedisResult<()> {
        // Get a synchronous connection from the client
        let mut conn = self.client.get_connection()?;
        // Perform the SET operation
        let _: () = conn.set(key, value)?;
        Ok(())
    }
}