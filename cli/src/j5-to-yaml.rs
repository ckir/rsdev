//! # `j5-to-yaml`: JSON5 to Clean YAML Converter
//!
//! This command-line interface (CLI) tool facilitates the conversion of JSON5
//! (JavaScript Object Notation, Fifth Edition) formatted files into a clean
//! YAML (YAML Ain't Markup Language) representation. It's designed to be a
//! robust utility for developers working with configuration files or data
//! definitions that benefit from JSON5's human-friendly syntax but need to be
//! consumed by systems or tools that prefer YAML.
//!
//! ## Key Features:
//! - **JSON5 Parsing**: Leverages the `json5` crate to accurately parse
//!   JSON5 syntax, including comments, unquoted keys, and trailing commas.
//! - **YAML Conversion**: Utilizes `serde_yml` to serialize the parsed
//!   data into standard YAML.
//! - **Block Scalar Handling**: Automatically detects multi-line strings
//!   and converts them into YAML's literal block scalar style (`|`),
//!   enhancing readability for embedded code, scripts, or lengthy text.
//! - **Output Validation**: Includes a critical step to verify that the
//!   generated YAML is syntactically correct, catching potential issues
//!   before output.
//! - **File-based Output**: Writes the converted YAML directly to a specified
//!   output file.
//!
//! ## Usage
//!
//! ```bash
//! j5-to-yaml --input <PATH_TO_JSON5_FILE> --output <PATH_TO_YAML_FILE>
//! ```
//!
//! This tool is particularly useful in environments where configuration is
//! authored in JSON5 but deployed or processed as YAML, ensuring a seamless
//! and validated conversion process.

#![doc(html_logo_url = "https://example.com/logo.png")] // Placeholder for consistency
#![forbid(unsafe_code)]
#![warn(missing_docs, rust_2018_idioms, unused_qualifications)]

use clap::Parser;
use std::fs;
use std::path::PathBuf;

/// # Command Line Arguments
///
/// Defines the command-line arguments for the `j5-to-yaml` tool,
/// using `clap` for parsing and help generation.
/// # Command Line Arguments
///
/// Defines the command-line arguments for the `j5-to-yaml` tool,
/// using `clap` for parsing and help generation.
#[derive(Parser, Debug)]
#[command(author, version, about = "Converts JSON5 to Clean YAML with Block Scalars")]
struct Args {
    /// Path to the input .json5 file that needs to be converted.
    #[arg(short, long)]
    input: PathBuf,

    /// Path to the output .yaml file where the converted content will be saved.
    /// This argument is mandatory.
    #[arg(short, long)]
    output: PathBuf,
}

/// # Main Entry Point
///
/// This is the `main` function for the `j5-to-yaml` command-line tool.
/// It orchestrates the entire conversion process from JSON5 to YAML,
/// including reading input, parsing, serialization, and crucially, validation.
///
/// ## Workflow:
/// 1.  **Parse Arguments**: Uses `clap` to parse command-line arguments.
/// 2.  **Read JSON5**: Reads the content of the input JSON5 file. If reading fails,
///     it prints an error and exits.
/// 3.  **Parse JSON5**: Parses the JSON5 content into a `serde_json::Value`. The
///     `json5` crate is specifically chosen for its robust handling of JSON5 syntax.
///     Any parsing errors result in an exit.
/// 4.  **Convert to YAML**: Serializes the `serde_json::Value` into a YAML string
///     using `serde_yml`. A key benefit here is `serde_yml`'s automatic detection
///     and use of YAML's literal block scalar style (`|`) for multi-line strings,
///     which significantly improves readability.
/// 5.  **Verify YAML**: A critical step where the generated YAML string is immediately
///     parsed back to ensure its validity. This catches potential issues early.
/// 6.  **Write to File**: Writes the validated YAML string to the specified output file.
///     If writing fails, it panics.
///
/// Error handling is designed to be user-friendly, providing clear messages and
/// exiting with a non-zero status code on failure.
/// # Main Entry Point
///
/// This is the `main` function for the `j5-to-yaml` command-line tool.
/// It orchestrates the entire conversion process from JSON5 to YAML,
/// including reading input, parsing, serialization, and crucially, validation.
///
/// ## Workflow:
/// 1.  **Parse Arguments**: Uses `clap` to parse command-line arguments.
/// 2.  **Read JSON5**: Reads the content of the input JSON5 file. If reading fails,
///     it prints an error and exits.
/// 3.  **Parse JSON5**: Parses the JSON5 content into a `serde_json::Value`. The
///     `json5` crate is specifically chosen for its robust handling of JSON5 syntax.
///     Any parsing errors result in an exit.
/// 4.  **Convert to YAML**: Serializes the `serde_json::Value` into a YAML string
///     using `serde_yml`. A key benefit here is `serde_yml`'s automatic detection
///     and use of YAML's literal block scalar style (`|`) for multi-line strings,
///     which significantly improves readability.
/// 5.  **Verify YAML**: A critical step where the generated YAML string is immediately
///     parsed back to ensure its validity. This catches potential issues early.
/// 6.  **Write to File**: Writes the validated YAML string to the specified output file.
///     If writing fails, it panics.
///
/// Error handling is designed to be user-friendly, providing clear messages and
/// exiting with a non-zero status code on failure.
fn main() {
    let args = Args::parse();

    /// 1. Reads the content of the input JSON5 file. Exits with an error message if the file cannot be read.
    let input_str = fs::read_to_string(&args.input).unwrap_or_else(|err| {
        eprintln!("❌ Failed to read input file '{}': {}", args.input.display(), err);
        std::process::exit(1);
    });

    /// 2. Parses the JSON5 content into a `serde_json::Value`. Exits with an error message if parsing fails.
    let data: serde_json::Value = json5::from_str(&input_str).unwrap_or_else(|err| {
        eprintln!("❌ JSON5 Parse Error in '{}': {}", args.input.display(), err);
        std::process::exit(1);
    });

    /// 3. Converts the intermediate `serde_json::Value` to a YAML string.
    /// `serde_yml` handles automatic detection of block scalars for multi-line strings.
    let yaml_output = serde_yml::to_string(&data).unwrap_or_else(|err| {
        eprintln!("❌ YAML Serialization Error: {}", err);
        std::process::exit(1);
    });

    /// 4. Verifies the generated YAML by attempting to parse it back into a `serde_json::Value`.
    /// Exits if the generated YAML is not syntactically correct.
    match serde_yml::from_str::<serde_json::Value>(&yaml_output) {
        Ok(_) => println!("✅ Verification Successful: Output is valid YAML."),
        Err(e) => {
            eprintln!("⚠️ Verification Failed: The generated YAML is invalid! Error: {}", e);
            std::process::exit(1); // Exit if the generated YAML is not valid.
        }
    }

    /// 5. Writes the validated YAML string to the specified output file.
    /// Exits with an error message if the file cannot be written.
    fs::write(&args.output, &yaml_output).unwrap_or_else(|err| {
        eprintln!("❌ Failed to write output to file '{}': {}", args.output.display(), err);
        std::process::exit(1);
    });
    
    println!("🚀 Conversion complete! Saved to '{}'.", args.output.display());
}