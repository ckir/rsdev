//! # `j5-to-json`: JSON5 to JSON Converter
//!
//! This command-line interface (CLI) tool provides a straightforward way to
//! convert a JSON5 document into a standard JSON format. It's useful for
//! processing configuration files, data interchange, or when JSON5's extended
//! syntax is used in development but standard JSON is required for production
//! systems or tools that don't support JSON5.
//!
//! ## Key Features:
//! - **JSON5 Parsing**: Robustly parses JSON5 input, handling features like
//!   comments, unquoted keys, and trailing commas.
//! - **JSON Output**: Converts the parsed JSON5 structure into valid JSON.
//! - **Output Flexibility**: Can print the resulting JSON to standard output
//!   or save it to a specified file.
//! - **Formatting Options**: Supports both pretty-printed (human-readable)
//!   and minified (compact) JSON output.
//!
//! ## Usage
//!
//! ```bash
//! j5-to-json --input <PATH_TO_JSON5_FILE> [--output <PATH_TO_JSON_FILE>] [--minify]
//! ```
//!
//! This tool ensures interoperability between systems that prefer JSON5 for
//! authoring and those that require strict JSON.

#![doc(html_logo_url = "https://example.com/logo.png")] // Placeholder for consistency
#![forbid(unsafe_code)]
#![warn(missing_docs, rust_2018_idioms, unused_qualifications)]

use clap::Parser;
use std::fs;
use std::io::{self, Write};
use std::path::PathBuf;

/// # Command Line Arguments
///
/// Defines the command-line arguments and options for the `j5-to-json` tool,
/// using `clap` for parsing and help generation.
/// # Command Line Arguments
///
/// Defines the command-line arguments and options for the `j5-to-json` tool,
/// using `clap` for parsing and help generation.
#[derive(Parser, Debug)]
#[command(
    version, // Automatically pulls version from Cargo.toml.
    about, // Short description.
    long_about = "This tool converts a JSON5 file to a standard JSON file. It can either save the output to a specified file or print it to standard output. You can also choose between pretty-printed and minified output." // Detailed description.
)]
struct Args {
    /// Path to the input JSON5 file. This argument is mandatory.
    #[arg(short, long)]
    input: PathBuf,

    /// Optional path to the output JSON file. If this argument is not provided,
    /// the converted JSON will be printed to standard output.
    #[arg(short, long)]
    output: Option<PathBuf>,

    /// If this flag is present, the output JSON will be minified (without
    /// extra whitespace for pretty-printing). By default, the output is pretty-printed.
    #[arg(short, long)]
    minify: bool,
}

/// # Main Entry Point
///
/// This is the `main` function for the `j5-to-json` command-line tool.
/// It orchestrates the JSON5 parsing, JSON serialization, and output handling.
///
/// ## Workflow:
/// 1.  **Parse Arguments**: Uses `clap` to parse the command-line arguments into an `Args` struct.
/// 2.  **Read JSON5**: Reads the content of the specified input JSON5 file.
/// 3.  **Parse JSON5**: Uses `serde_json5` to parse the JSON5 content into a `serde_json::Value`
///     representation. Any parsing errors are reported, and the program exits.
/// 4.  **Serialize JSON**:
///     -   If the `--minify` flag is set, it serializes the `serde_json::Value` into
///         a compact, minified JSON string.
///     -   Otherwise, it pretty-prints the JSON with indentation for readability.
///     -   Serialization errors are reported, and the program exits.
/// 5.  **Write Output**:
///     -   If an `--output` file path was provided, the JSON string is written to that file.
///     -   If no output file was specified, the JSON string is printed to standard output.
///
/// # Returns
/// A `Result<()>` indicating success or an `io::Result` if any file I/O operation fails.
/// The program exits early on JSON5 parsing or JSON serialization errors.
/// # Main Entry Point
///
/// This is the `main` function for the `j5-to-json` command-line tool.
/// It orchestrates the JSON5 parsing, JSON serialization, and output handling.
///
/// ## Workflow:
/// 1.  **Parse Arguments**: Uses `clap` to parse the command-line arguments into an `Args` struct.
/// 2.  **Read JSON5**: Reads the content of the specified input JSON5 file.
/// 3.  **Parse JSON5**: Uses `serde_json5` to parse the JSON5 content into a `serde_json::Value`
///     representation. Any parsing errors are reported, and the program exits.
/// 4.  **Serialize JSON**:
///     -   If the `--minify` flag is set, it serializes the `serde_json::Value` into
///         a compact, minified JSON string.
///     -   Otherwise, it pretty-prints the JSON with indentation for readability.
///     -   Serialization errors are reported, and the program exits.
/// 5.  **Write Output**:
///     -   If an `--output` file path was provided, the JSON string is written to that file.
///     -   If no output file was specified, the JSON string is printed to standard output.
///
/// # Returns
/// A `Result<()>` indicating success or an `io::Result` if any file I/O operation fails.
/// The program exits early on JSON5 parsing or JSON serialization errors.
fn main() -> io::Result<()> {
    /// Parses command-line arguments into an `Args` struct.
    let args = Args::parse();

    /// 1. Reads the content of the input JSON5 file.
    let json5_content = fs::read_to_string(&args.input)?;

    /// 2. Parses the JSON5 content into a `serde_json::Value`. Exits on parsing errors.
    let json_value: serde_json::Value = match serde_json5::from_str(&json5_content) {
        Ok(value) => value,
        Err(e) => {
            eprintln!("Error parsing JSON5 from '{}': {}", args.input.display(), e);
            std::process::exit(1);
        }
    };

    /// 3. Serializes the `serde_json::Value` into a JSON string,
    /// either minified or pretty-printed based on the `minify` flag. Exits on serialization errors.
    let json_output = if args.minify {
        match serde_json::to_string(&json_value) {
            Ok(json) => json,
            Err(e) => {
                eprintln!("Error serializing to minified JSON: {}", e);
                std::process::exit(1);
            }
        }
    } else {
        match serde_json::to_string_pretty(&json_value) {
            Ok(json) => json,
            Err(e) => {
                eprintln!("Error serializing to pretty JSON: {}", e);
                std::process::exit(1);
            }
        }
    };

    /// 4. Writes the generated JSON output to either a specified file or standard output.
    if let Some(output_path) = args.output {
        fs::write(&output_path, json_output.as_bytes())?;
        println!("Successfully converted '{}' and saved to '{}'.", args.input.display(), output_path.display());
    } else {
        io::stdout().write_all(json_output.as_bytes())?;
    }

    Ok(())
}
