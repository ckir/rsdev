use chrono::{DateTime, Utc};

/// Returns the current UTC datetime formatted as an RFC 9557 string.
///
/// The format is `YYYY-MM-DDTHH:MM:SS.msZ`, e.g., "2024-02-09T12:34:56.789Z".
/// This format is commonly used for consistent timestamp representation.
pub fn current_datetime_rfc9557() -> String {
    let now: DateTime<Utc> = Utc::now();
    now.format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string()
}

/// Prints the type name of the given reference to standard output.
///
/// This is a utility function primarily useful for debugging and introspection
/// to dynamically determine the Rust type of a variable.
///
/// # Arguments
/// * `_` - A reference to any type `T`. The value itself is not used, only its type.
pub fn print_type<T>(_: &T) {
    println!("{:?}", std::any::type_name::<T>());
}