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
    /// Creates a new `CacheHandler` from a connection string.
    ///
    /// # Arguments
    /// * `url` - The redis URL (e.g., "redis://127.0.0.1/").
    ///
    /// # Errors
    /// Returns a `RedisResult` if the client fails to open the URL.
    pub fn new(url: &str) -> RedisResult<Self> {
        // Open the connection to the redis server
        let client = Client::open(url)?;
        Ok(Self { client })
    }

    /// Stores a string value in the cache.
    ///
    /// # Arguments
    /// * `key` - The cache key.
    /// * `value` - The string value to store.
    ///
    /// # Errors
    /// Returns a `RedisResult` if the connection fails or the SET operation fails.
    pub fn set_string(&self, key: &str, value: &str) -> RedisResult<()> {
        // Get a synchronous connection from the client
        let mut conn = self.client.get_connection()?;
        // Perform the SET operation
        let _: () = conn.set(key, value)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cache_handler_new_invalid_url() {
        // // Test with an invalid URL to ensure it returns an error
        let result = CacheHandler::new("invalid_url");
        assert!(result.is_err());
    }

    #[test]
    fn test_cache_handler_set_string_failure() {
        // // Even with a valid-looking URL, if Redis is not running, it should fail
        let handler = CacheHandler::new("redis://127.0.0.1:6379/").unwrap();
        let result = handler.set_string("test_key", "test_value");
        assert!(result.is_err());
    }
}
