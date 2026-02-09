//! # `j5-format`: A Command Line Interface (CLI) Tool to Format JSON5 Documents
//!
//! This CLI tool is designed to format [JSON5](https://json5.org) documents,
//! also known as "JSON for Humans," into a consistent and readable style.
//! A key feature is its ability to preserve comments, which are a valuable
//! aspect of JSON5 but often lost in standard JSON processing.
//!
//! ## Overview
//!
//! The tool parses JSON5 documents, applies a set of formatting rules (which
//! can be configured via command-line options), and then outputs the
//! formatted result. It can optionally overwrite the original files.
//!
//! ## Usage
//!
//! ```text
//! j5-format [FLAGS] [OPTIONS] [files]...
//!
//! FLAGS:
//! -h, --help                  Prints help information
//! -n, --no_trailing_commas    Suppress trailing commas (otherwise added by default)
//! -o, --one_element_lines     Objects or arrays with a single child should collapse to a
//!                             single line; no trailing comma
//! -r, --replace               Replace (overwrite) the input file with the formatted result
//! -s, --sort_arrays           Sort arrays of primitive values (string, number, boolean, or
//!                             null) lexicographically
//! -V, --version               Prints version information
//!
//! OPTIONS:
//! -i, --indent <indent>    Indent by the given number of spaces [default: 4]
//!
//! ARGS:
//! <files>...    Files to format (use "-" for stdin)
//! ```
//!
//! For more details on the underlying formatting engine, refer to the
//! `json5format` crate's documentation.

#![warn(missing_docs)] // Ensure all public items are documented.

use anyhow::Result;
use json5format::*; // Import all necessary items from the core formatting library.
use std::fs;
use std::io;
use std::io::{Read, Write};
use std::path::PathBuf;
use structopt::StructOpt; // Used for parsing command-line arguments.

/// # Parse Documents
///
/// Parses each file specified in the `files` vector and returns a vector of
/// `ParsedDocument` objects. This function ensures that all input documents
/// are syntactically valid JSON5 before any formatting is attempted.
///
/// If any file cannot be read or parsed, the process aborts immediately,
/// ensuring that no partial or incorrect formatting is applied.
///
/// ## Arguments
/// * `files` - A `Vec<PathBuf>` where each `PathBuf` represents a file to be parsed.
///   A special value `"-"` indicates reading from standard input.
///
/// # Returns
/// A `Result` containing a `Vec<ParsedDocument>` on success, or an `anyhow::Error`
/// if any file read or parsing operation fails.
/// # Parse Documents
///
/// Parses each file specified in the `files` vector and returns a vector of
/// `ParsedDocument` objects. This function ensures that all input documents
/// are syntactically valid JSON5 before any formatting is attempted.
///
/// If any file cannot be read or parsed, the process aborts immediately,
/// ensuring that no partial or incorrect formatting is applied.
///
/// ## Arguments
/// * `files` - A `Vec<PathBuf>` where each `PathBuf` represents a file to be parsed.
///   A special value `"-"` indicates reading from standard input.
///
/// # Returns
/// A `Result` containing a `Vec<ParsedDocument>` on success, or an `anyhow::Error`
/// if any file read or parsing operation fails.
fn parse_documents(files: Vec<PathBuf>) -> Result<Vec<ParsedDocument>, anyhow::Error> {
    let mut parsed_documents = Vec::with_capacity(files.len());
    for file in files {
        let filename = file.clone().into_os_string().to_string_lossy().to_string();
        let mut buffer = String::new();
        if filename == "-" {
            // Read from stdin if the filename is "-".
            Opt::from_stdin(&mut buffer)?;
        } else {
            // Read from the specified file.
            fs::File::open(&file)?.read_to_string(&mut buffer)?;
        }

        // Parse the content into a `ParsedDocument`, preserving comments and structure.
        parsed_documents.push(ParsedDocument::from_string(buffer, Some(filename))?);
    }
    Ok(parsed_documents)
}

/// # Format Documents
///
/// Formats the given `parsed_documents` according to the provided `options`.
///
/// This function handles the actual formatting and output. If `replace` is true,
/// each original file is overwritten. Otherwise, the formatted content is printed
/// to standard output.
///
/// ## Arguments
/// * `parsed_documents` - A `Vec<ParsedDocument>` containing the parsed JSON5 documents.
/// * `options` - A `FormatOptions` struct specifying how the documents should be formatted.
/// * `replace` - A boolean indicating whether to overwrite the input files (`true`)
///   or print to stdout (`false`).
///
/// # Returns
/// A `Result<(), anyhow::Error>` indicating success or failure of the formatting
/// and writing operations.
/// # Format Documents
///
/// Formats the given `parsed_documents` according to the provided `options`.
///
/// This function handles the actual formatting and output. If `replace` is true,
/// each original file is overwritten. Otherwise, the formatted content is printed
/// to standard output.
///
/// ## Arguments
/// * `parsed_documents` - A `Vec<ParsedDocument>` containing the parsed JSON5 documents.
/// * `options` - A `FormatOptions` struct specifying how the documents should be formatted.
/// * `replace` - A boolean indicating whether to overwrite the input files (`true`)
///   or print to stdout (`false`).
///
/// # Returns
/// A `Result<(), anyhow::Error>` indicating success or failure of the formatting
/// and writing operations.
fn format_documents(
    parsed_documents: Vec<ParsedDocument>,
    options: FormatOptions,
    replace: bool,
) -> Result<(), anyhow::Error> {
    let format = Json5Format::with_options(options)?;
    for (index, parsed_document) in parsed_documents.iter().enumerate() {
        let filename = parsed_document.filename().as_ref().expect("Document should have a filename").as_str();
        // Convert the parsed document back into a formatted UTF-8 byte array.
        let bytes = format.to_utf8(parsed_document)?;
        if replace {
            // Overwrite the original file.
            Opt::write_to_file(filename, &bytes)?;
        } else {
            // Print to stdout. Add separators if multiple files are being processed.
            if index > 0 {
                println!(); // Add a newline between formatted documents.
            }
            if parsed_documents.len() > 1 {
                // Prepend filename for clarity if multiple files are output.
                println!("{}:", filename);
                println!("{}", "=".repeat(filename.len()));
            }
            // Print the formatted JSON5.
            io::stdout().write_all(&bytes)?;
        }
    }
    Ok(())
}

/// # Main Entry Point for `j5-format` CLI
///
/// This is the `main` function for the `j5-format` command-line tool.
/// It orchestrates the entire process: argument parsing, document parsing,
/// and document formatting/output.
///
/// ## Workflow:
/// 1.  Parses command-line arguments using `structopt`.
/// 2.  Validates that at least one file (or stdin) is provided.
/// 3.  Calls `parse_documents` to read and parse all input files.
/// 4.  Constructs `FormatOptions` based on the parsed command-line arguments.
/// 5.  Calls `format_documents` to apply the formatting and handle output.
///
/// # Returns
/// A `Result<()>` indicating the overall success or failure of the CLI operation.
/// # Main Entry Point for `j5-format` CLI
///
/// This is the `main` function for the `j5-format` command-line tool.
/// It orchestrates the entire process: argument parsing, document parsing,
/// and document formatting/output.
///
/// ## Workflow:
/// 1.  Parses command-line arguments using `structopt`.
/// 2.  Validates that at least one file (or stdin) is provided.
/// 3.  Calls `parse_documents` to read and parse all input files.
/// 4.  Constructs `FormatOptions` based on the parsed command-line arguments.
/// 5.  Calls `format_documents` to apply the formatting and handle output.
///
/// # Returns
/// A `Result<()>` indicating the overall success or failure of the CLI operation.
fn main() -> Result<()> {
    /// Parses command-line arguments into the `Opt` struct.
    let args = Opt::args();

    /// Checks if any input files were provided. If not, it returns an error.
    if args.files.is_empty() {
        return Err(anyhow::anyhow!("No files to format. Please specify at least one file or use '-' for stdin."));
    }

    /// Parses all specified input documents into a vector of `ParsedDocument`s.
    let parsed_documents = parse_documents(args.files)?;

    /// Constructs `FormatOptions` based on the parsed command-line arguments.
    let options = FormatOptions {
        indent_by: args.indent,
        trailing_commas: !args.no_trailing_commas,
        collapse_containers_of_one: args.one_element_lines,
        sort_array_items: args.sort_arrays,
        ..Default::default()
    };

    /// Formats the documents and handles writing them to files or stdout.
    format_documents(parsed_documents, options, args.replace)
}

/// # Command Line Options
///
/// Defines the structure for parsing command-line arguments using `structopt`.
/// This struct also serves to generate the help documentation for the CLI.
/// # Command Line Options
///
/// Defines the structure for parsing command-line arguments using `structopt`.
/// This struct also serves to generate the help documentation for the CLI.
#[derive(Debug, StructOpt)]
#[structopt(
    name = "json5format",
    about = "Format JSON5 documents to a consistent style, preserving comments."
)]
struct Opt {
    /// Files to format (use "-" for stdin).
    #[structopt(parse(from_os_str))]
    files: Vec<PathBuf>,

    /// If set, the input file(s) will be overwritten with the formatted result.
    /// Use with caution.
    #[structopt(short, long)]
    replace: bool,

    /// If set, trailing commas will be suppressed. Otherwise, trailing commas
    /// are added by default where appropriate in JSON5 (e.g., in objects and arrays).
    #[structopt(short, long)]
    no_trailing_commas: bool,

    /// If set, objects or arrays that contain only a single child element will
    /// be collapsed to a single line, and no trailing comma will be added.
    #[structopt(short, long)]
    one_element_lines: bool,

    /// If set, arrays of primitive values (strings, numbers, booleans, or null)
    /// will be sorted lexicographically. This is useful for consistent output
    /// but only applies to primitive arrays, not arrays of objects or arrays.
    #[structopt(short, long)]
    sort_arrays: bool,

    /// The number of spaces to use for indentation. Defaults to 4 spaces.
    #[structopt(short, long, default_value = "4")]
    indent: usize,
}

/// # `Opt` Implementations for Non-Test Environment
///
/// This `impl` block provides the concrete implementations for reading arguments,
/// reading from stdin, and writing to files when the tool is run in a non-test
/// environment. These methods interact directly with the file system and standard I/O.
/// # `Opt` Implementations for Non-Test Environment
///
/// This `impl` block provides the concrete implementations for reading arguments,
/// reading from stdin, and writing to files when the tool is run in a non-test
/// environment. These methods interact directly with the file system and standard I/O.
#[cfg(not(test))]
impl Opt {
    /// Parses command-line arguments using `structopt`'s default behavior.
    fn args() -> Self {
        Self::from_args()
    }

    /// Reads all available data from standard input into the provided buffer.
    ///
    /// # Arguments
    /// * `buf` - A mutable reference to a `String` to store the input.
    ///
    /// # Returns
    /// A `Result` indicating the number of bytes read or an `io::Error`.
    fn from_stdin(buf: &mut String) -> Result<usize, io::Error> {
        io::stdin().read_to_string(buf)
    }

    /// Writes the given byte slice to the specified file, overwriting its contents.
    ///
    /// # Arguments
    /// * `filename` - The path to the file to write.
    /// * `bytes` - The byte slice to write into the file.
    ///
    /// # Returns
    /// A `Result` indicating success or an `io::Error`.
    fn write_to_file(filename: &str, bytes: &[u8]) -> Result<(), io::Error> {
        fs::OpenOptions::new()
            .create(true) // Create the file if it doesn't exist.
            .truncate(true) // Clear existing contents.
            .write(true) // Open in write mode.
            .open(filename)?
            .write_all(bytes)
    }
}

/// # `Opt` Implementations for Test Environment
///
/// This `impl` block provides mocked implementations for reading arguments,
/// reading from stdin, and writing to files when the tool is run within its
/// test suite (`cfg(test)`). This allows tests to inject arguments and
/// capture output without affecting the actual file system or console.
#[cfg(test)]
impl Opt {
    /// Mocks command-line argument parsing for testing purposes.
    /// It checks for `TEST_ARGS` global static variable to supply arguments.
    fn args() -> Self {
        if let Some(test_args) = unsafe { &self::tests::TEST_ARGS } {
            // If test arguments are provided, use them to parse the Opt struct.
            Self::from_clap(
                &Self::clap()
                    .get_matches_from_safe(test_args)
                    .expect("failed to parse TEST_ARGS command line arguments"),
            )
        } else {
            // Otherwise, fall back to normal argument parsing (should not happen in unit tests).
            Self::from_args()
        }
    }

    /// Mocks reading from standard input for testing purposes.
    /// It uses a `TEST_BUFFER` global static variable to supply input.
    ///
    /// # Arguments
    /// * `buf` - A mutable reference to a `String` where input would normally be stored.
    ///
    /// # Returns
    /// A `Result` indicating the length of the mocked input or an `io::Error`.
    fn from_stdin(mut buf: &mut String) -> Result<usize, io::Error> {
        if let Some(test_buffer) = unsafe { &mut self::tests::TEST_BUFFER } {
            *buf = test_buffer.clone();
            Ok(buf.as_bytes().len())
        } else {
            // Fallback to actual stdin if no test buffer is set.
            io::stdin().read_to_string(&mut buf)
        }
    }

    /// Mocks writing to a file for testing purposes.
    /// If the filename is `"-"`, it captures output into `TEST_BUFFER`.
    /// Otherwise, it performs a real file write (though this is less common in tests).
    ///
    /// # Arguments
    /// * `filename` - The target filename, or `"-"` for mock stdout.
    /// * `bytes` - The content to write.
    ///
    /// # Returns
    /// A `Result` indicating success or an `io::Error`.
    fn write_to_file(filename: &str, bytes: &[u8]) -> Result<(), io::Error> {
        if filename == "-" {
            // If writing to mock stdin/stdout, capture the output.
            let buf = std::str::from_utf8(&bytes)
                .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
            if let Some(test_buffer) = unsafe { &mut self::tests::TEST_BUFFER } {
                *test_buffer = buf.to_string();
            } else {
                // If no test buffer, print to actual stdout (fallback).
                print!("{}", buf);
            }
            Ok(())
        } else {
            // For actual files, perform a real write operation.
            fs::OpenOptions::new()
                .create(true)
                .truncate(true)
                .write(true)
                .open(filename)?
                .write_all(&bytes)
        }
    }
}

/// # Test Module
///
/// Contains unit and integration tests for the `j5-format` CLI tool.
///
/// This module uses global mutable static variables (`TEST_ARGS`, `TEST_BUFFER`)
/// to inject test data and capture output, allowing for isolated testing
/// of the `main` function and its dependencies.
#[cfg(test)]
mod tests {

    use super::*;

    /// Global static variable to inject command-line arguments for tests.
    pub(crate) static mut TEST_ARGS: Option<Vec<&str>> = None;
    /// Global static variable to capture or inject stdin/stdout content for tests.
    pub(crate) static mut TEST_BUFFER: Option<String> = None;

    /// # Test `main` Function with Mocked Input/Output
    ///
    /// This test case simulates running the `main` CLI function with specific
    /// arguments and an example JSON5 input string. It then asserts that the
    /// output matches the expected formatted JSON5.
    #[test]
    fn test_main() {
        let example_json5 = r##"{
    offer: [
        {
            runner: "elf",
        },
        {
            from: "framework",
            to: "#elements",
            protocol: "/svc/fuchsia.sys2.Realm",
        },
        {
            to: "#elements",
            protocol: [
                "/svc/fuchsia.logger.LogSink",
                "/svc/fuchsia.cobalt.LoggerFactory",
            ],
            from: "realm",
        },
    ],
    collections: [
        {
            name: "elements",
            durability: "transient",
        }
    ],
    use: [
        {
            runner: "elf",
        },
        {
            protocol: "/svc/fuchsia.sys2.Realm",
            from: "framework",
        },
        {
            from: "realm",
            to: "#elements",
            protocol: [
                "/svc/fuchsia.logger.LogSink",
                "/svc/fuchsia.cobalt.LoggerFactory",
            ],
        },
    ],
    children: [
    ],
    program: {
        args: [ "--zarg_first", "zoo_opt", "--arg3", "and_arg3_value" ],
        binary: "bin/session_manager",
    },
}"##;
        let expected = r##"{
  offer: [
    { runner: "elf" },
    {
      from: "framework",
      to: "#elements",
      protocol: "/svc/fuchsia.sys2.Realm"
    },
    {
      to: "#elements",
      protocol: [
        "/svc/fuchsia.cobalt.LoggerFactory",
        "/svc/fuchsia.logger.LogSink"
      ],
      from: "realm"
    }
  ],
  collections: [
    {
      name: "elements",
      durability: "transient"
    }
  ],
  use: [
    { runner: "elf" },
    {
      protocol: "/svc/fuchsia.sys2.Realm",
      from: "framework"
    },
    {
      from: "realm",
      to: "#elements",
      protocol: [
        "/svc/fuchsia.cobalt.LoggerFactory",
        "/svc/fuchsia.logger.LogSink"
      ]
    }
  ],
  children: [],
  program: {
    args: [
      "--arg3",
      "--zarg_first",
      "and_arg3_value",
      "zoo_opt"
    ],
    binary: "bin/session_manager"
  }
}
"##;
        unsafe {
            TEST_ARGS = Some(vec![
                "formatjson5",
                "--replace", // Simulates writing to a file (or stdout if filename is "-")
                "--no_trailing_commas",
                "--one_element_lines",
                "--sort_arrays",
                "--indent",
                "2",
                "-", // Read from stdin (mocked by TEST_BUFFER)
            ]);
            TEST_BUFFER = Some(example_json5.to_string());
        }
        main().expect("test failed");
        assert!(unsafe { &TEST_BUFFER }.is_some());
        assert_eq!(unsafe { TEST_BUFFER.as_ref().unwrap() }, expected);
    }

    /// # Test Command-Line Argument Parsing
    ///
    /// This test verifies that the `Opt` struct correctly parses various
    /// combinations of command-line arguments.
    #[test]
    fn test_args() {
        // Test default values.
        let args = Opt::from_iter(vec![""].iter());
        assert_eq!(args.files.len(), 0);
        assert_eq!(args.replace, false);
        assert_eq!(args.no_trailing_commas, false);
        assert_eq!(args.one_element_lines, false);
        assert_eq!(args.sort_arrays, false);
        assert_eq!(args.indent, 4);

        // Test with custom values and flags.
        let some_filename = "some_file.json5";
        let args = Opt::from_iter(
            vec!["formatjson5", "-r", "-n", "-o", "-s", "-i", "2", some_filename].iter(),
        );
        assert_eq!(args.files.len(), 1);
        assert_eq!(args.replace, true);
        assert_eq!(args.no_trailing_commas, true);
        assert_eq!(args.one_element_lines, true);
        assert_eq!(args.sort_arrays, true);
        assert_eq!(args.indent, 2);

        let filename = args.files[0].clone().into_os_string().to_string_lossy().to_string();
        assert_eq!(filename, some_filename);
    }
}
