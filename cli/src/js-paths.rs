//! # `js-paths`: Extract All Paths from JSON/JSON5
//!
//! This command-line interface (CLI) tool is designed to parse a JSON or JSON5
//! file and output every possible "path" to a value within its structure.
//! A path is represented in a dot-notation format (e.g., `user.address.street`,
//! `items.0.name`).
//!
//! ## Purpose:
//! This utility is invaluable for:
//! - **Data Exploration**: Quickly understanding the structure of an unknown
//!   JSON/JSON5 document.
//! - **Schema Generation**: As a preliminary step for generating schemas by
//!   identifying all potential data fields.
//! - **Configuration Analysis**: Verifying configuration structures or identifying
//!   all configurable keys.
//! - **Debugging**: Pinpointing the location of specific data points within
//!   complex nested objects and arrays.
//!
//! The tool leverages `serde_json5` for robust parsing of both JSON and JSON5.
//! Paths are printed to standard output, making it easy to pipe to other tools.

#![doc(html_logo_url = "https://example.com/logo.png")] // Placeholder for consistency
#![forbid(unsafe_code)]
#![warn(missing_docs, rust_2018_idioms, unused_qualifications)]

use anyhow::{Context, Result};
use clap::Parser;
use serde_json::Value;
use std::fs;
use std::path::PathBuf;

/// # Command Line Arguments
///
/// Defines the command-line arguments for the `js-paths` tool,
/// using `clap` for parsing and help generation.
/// # Command Line Arguments
///
/// Defines the command-line arguments for the `js-paths` tool,
/// using `clap` for parsing and help generation.
#[derive(Parser, Debug)]
#[command(
    name = "js-paths",
    version, // Automatically pulls version from Cargo.toml.
    about = "Output all paths in a JSON/JSON5 file" // Short description.
)]
struct Args {
    /// Path to the input JSON or JSON5 file. This argument is mandatory.
    #[arg(required = true)]
    input: PathBuf,
}

/// # Main Entry Point
///
/// This is the `main` function for the `js-paths` command-line tool.
/// It orchestrates reading the input file, parsing its content as JSON5,
/// and then recursively printing all paths found within the JSON structure.
///
/// ## Workflow:
/// 1.  **Parse Arguments**: Uses `clap` to parse command-line arguments.
/// 2.  **Read File**: Reads the content of the specified input file into a string.
///     Includes `anyhow` for context-rich error reporting if the file cannot be read.
/// 3.  **Parse JSON/JSON5**: Attempts to parse the file content using `serde_json5::from_str`.
///     This allows it to handle both standard JSON and the more permissive JSON5 format.
///     `anyhow` is used to provide helpful error messages on parsing failure.
/// 4.  **Print Paths**: Calls the recursive `print_paths` function to traverse the
///     parsed `serde_json::Value` structure and output all unique paths.
///
/// # Returns
/// A `Result<()>` indicating the overall success or failure of the CLI operation.
/// # Main Entry Point
///
/// This is the `main` function for the `js-paths` command-line tool.
/// It orchestrates reading the input file, parsing its content as JSON5,
/// and then recursively printing all paths found within the JSON structure.
///
/// ## Workflow:
/// 1.  **Parse Arguments**: Uses `clap` to parse command-line arguments.
/// 2.  **Read File**: Reads the content of the specified input file into a string.
///     Includes `anyhow` for context-rich error reporting if the file cannot be read.
/// 3.  **Parse JSON/JSON5**: Attempts to parse the file content using `serde_json5::from_str`.
///     This allows it to handle both standard JSON and the more permissive JSON5 format.
///     `anyhow` is used to provide helpful error messages on parsing failure.
/// 4.  **Print Paths**: Calls the recursive `print_paths` function to traverse the
///     parsed `serde_json::Value` structure and output all unique paths.
///
/// # Returns
/// A `Result<()>` indicating the overall success or failure of the CLI operation.
fn main() -> Result<()> {
    /// Parses command-line arguments into an `Args` struct.
    let args = Args::parse();

    /// Reads the entire content of the input file into a string.
    let content = fs::read_to_string(&args.input)
        .context(format!("Failed to read file: {:?}", args.input))?;

    /// Parses the content as JSON5 (which also supports standard JSON), handling potential errors.
    let v: Value = serde_json5::from_str(&content).context("Failed to parse JSON/JSON5")?;

    /// Initiates the recursive path printing from the root of the parsed JSON structure.
    print_paths(&v, String::new());

    Ok(())
}

/// # Recursively Print JSON Paths
///
/// Traverses a `serde_json::Value` structure and prints every path to a primitive value
/// or an empty object/array.
///
/// ## Logic:
/// - **Base Case**: If the `current_path` is not empty, it means we've reached a leaf
///   node or an object/array itself, so its path is printed.
/// - **Object Traversal**: For a JSON object, it iterates through its key-value pairs.
///   Keys are sorted lexicographically to ensure deterministic output, which is crucial
///   for consistent scripting and testing. It then recursively calls itself with a
///   new path appending the current key.
/// - **Array Traversal**: For a JSON array, it iterates through its elements. It constructs
///   paths using array indices (e.g., `parent.0`, `parent.1`) and recursively calls itself.
/// - **Primitive Values**: When a primitive value (string, number, boolean, null) is
///   encountered, its path is printed (if not already printed by the base case).
///
/// ## Arguments
/// * `v` - A reference to the current `serde_json::Value` being processed.
/// * `current_path` - The dot-separated path string built up to the current value.
/// # Recursively Print JSON Paths
///
/// Traverses a `serde_json::Value` structure and prints every path to a primitive value
/// or an empty object/array.
///
/// ## Logic:
/// - **Base Case**: If the `current_path` is not empty, it means we've reached a leaf
///   node or an object/array itself, so its path is printed.
/// - **Object Traversal**: For a JSON object, it iterates through its key-value pairs.
///   Keys are sorted lexicographically to ensure deterministic output, which is crucial
///   for consistent scripting and testing. It then recursively calls itself with a
///   new path appending the current key.
/// - **Array Traversal**: For a JSON array, it iterates through its elements. It constructs
///   paths using array indices (e.g., `parent.0`, `parent.1`) and recursively calls itself.
/// - **Primitive Values**: When a primitive value (string, number, boolean, null) is
///   encountered, its path is printed (if not already printed by the base case).
///
/// ## Arguments
/// * `v` - A reference to the current `serde_json::Value` being processed.
/// * `current_path` - The dot-separated path string built up to the current value.
fn print_paths(v: &Value, current_path: String) {
    /// Prints the current path if it's not the root (empty string).
    /// This captures paths to objects and arrays themselves, not just their children.
    if !current_path.is_empty() {
        println!("{}", current_path);
    }

    match v {
        /// If the value is an object, iterate through its sorted keys.
        Value::Object(map) => {
            // Collect and sort keys to ensure deterministic output order.
            let mut keys: Vec<_> = map.keys().collect();
            keys.sort();

            for k in keys {
                let val = &map[k];
                // Construct the new path by appending the current key.
                let new_path = if current_path.is_empty() {
                    k.to_string()
                } else {
                    format!("{}.{}", current_path, k)
                };
                /// Recursively calls `print_paths` for the child value.
                print_paths(val, new_path);
            }
        }
        /// If the value is an array, iterate through its elements by index.
        Value::Array(arr) => {
            for (i, val) in arr.iter().enumerate() {
                // Construct the new path by appending the current array index.
                let new_path = if current_path.is_empty() {
                    i.to_string()
                } else {
                    format!("{}.{}", current_path, i)
                };
                /// Recursively calls `print_paths` for the child value.
                print_paths(val, new_path);
            }
        }
        // Primitive values (string, number, boolean, null) don't have further nested paths.
        _ => {}
    }
}
