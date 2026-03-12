//! # General Utilities
//!
//! Provides miscellaneous helper functions used across the library.

use chrono::{DateTime, Utc};

/// Returns the current UTC date and time formatted according to RFC 9557.
///
/// The format is `YYYY-MM-DDTHH:MM:SS.sssZ`.
pub fn current_datetime_rfc9557() -> String {
    // // Capture current time in UTC
    let now: DateTime<Utc> = Utc::now();
    // // Format with milliseconds and 'Z' suffix
    now.format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_current_datetime_rfc9557_format() {
        let formatted = current_datetime_rfc9557();
        // // Verify format matches something like 2023-10-27T10:00:00.000Z
        assert!(formatted.contains('T'));
        assert!(formatted.ends_with('Z'));
        assert_eq!(formatted.len(), 24); // YYYY-MM-DDTHH:MM:SS.sssZ is 24 chars
    }
}
