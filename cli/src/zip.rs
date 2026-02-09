//! # `zip`: A Rust Implementation of the Linux `zip` Utility
//!
//! This command-line utility provides a powerful and flexible way to create
//! zip archives, mimicking many features found in the standard Linux `zip` command.
//! It is built in Rust for performance and reliability.
//!
//! ## Key Features:
//! - **File and Directory Archiving**: Can add individual files or entire
//!   directory trees (recursively) to a zip archive.
//! - **Compression Levels**: Supports various DEFLATE compression levels (0-9)
//!   or no compression (store only).
//! - **Path Manipulation**:
//!   - `junk_paths` (`-j`): Stores only the filename, discarding directory structure.
//! - **Exclusion Patterns**: Allows excluding files or directories based on glob patterns.
//! - **Verbose Output**: Provides detailed feedback on files being added.
//! - **Quiet Mode**: Suppresses most output for scripting.
//!
//! ## Usage
//!
//! ```bash
//! zip [OPTIONS] <OUTPUT_ZIP_FILE> <INPUTS...>
//!
//! # Example: Archive a directory recursively with maximum compression
//! zip -r -9 my_archive.zip my_directory/
//!
//! # Example: Archive specific files, junking their paths
//! zip -j files.zip path/to/file1.txt another_file.doc
//!
//! # Example: Archive a directory but exclude all .log files
//! zip -r -x "*.log" project.zip project_folder/
//! ```
//!
//! This tool is ideal for build processes, deployment, or general file archiving
//! tasks where a lightweight, efficient, and configurable zip utility is needed.

#![doc(html_logo_url = "https://example.com/logo.png")] // Placeholder for consistency
#![forbid(unsafe_code)]
#![warn(missing_docs, rust_2018_idioms, unused_qualifications)]

use anyhow::{Context, Result};
use clap::Parser;
use glob::Pattern;
use std::fs::File;
use std::io::{Read, Write};
use std::path::Path;
use walkdir::WalkDir; // Used for efficient recursive directory traversal.
use zip::write::FileOptions;

/// # Command Line Arguments
///
/// Defines the command-line arguments and options for the `zip` tool,
/// using `clap` for parsing and help generation.
/// # Command Line Arguments
///
/// Defines the command-line arguments and options for the `zip` tool,
/// using `clap` for parsing and help generation.
#[derive(Parser, Debug)]
#[command(
    name = "zip",
    about = "A Rust zip utility similar to linux zip",
    version // Automatically pulls version from Cargo.toml.
)]
struct Args {
    /// Recursively include contents of directories.
    #[arg(short = 'r', long)]
    recurse: bool,

    /// Operate in quiet mode; suppress most output messages.
    #[arg(short = 'q', long)]
    quiet: bool,

    /// Junk paths; store just the name of the file, not the whole path.
    #[arg(short = 'j', long)]
    junk_paths: bool,

    /// Use compression level 0 (store only, no compression).
    /// Mutually exclusive with other compression levels.
    #[arg(short = '0', conflicts_with_all = ["level_1", "level_2", "level_3", "level_4", "level_5", "level_6", "level_7", "level_8", "level_9"])]
    level_0: bool,
    /// Use compression level 1.
    #[arg(short = '1', conflicts_with_all = ["level_0", "level_2", "level_3", "level_4", "level_5", "level_6", "level_7", "level_8", "level_9"])]
    level_1: bool,
    /// Use compression level 2.
    #[arg(short = '2', conflicts_with_all = ["level_0", "level_1", "level_3", "level_4", "level_5", "level_6", "level_7", "level_8", "level_9"])]
    level_2: bool,
    /// Use compression level 3.
    #[arg(short = '3', conflicts_with_all = ["level_0", "level_1", "level_2", "level_4", "level_5", "level_6", "level_7", "level_8", "level_9"])]
    level_3: bool,
    /// Use compression level 4.
    #[arg(short = '4', conflicts_with_all = ["level_0", "level_1", "level_2", "level_3", "level_5", "level_6", "level_7", "level_8", "level_9"])]
    level_4: bool,
    /// Use compression level 5.
    #[arg(short = '5', conflicts_with_all = ["level_0", "level_1", "level_2", "level_3", "level_4", "level_6", "level_7", "level_8", "level_9"])]
    level_5: bool,
    /// Use compression level 6 (default if no level is specified).
    #[arg(short = '6', conflicts_with_all = ["level_0", "level_1", "level_2", "level_3", "level_4", "level_5", "level_7", "level_8", "level_9"])]
    level_6: bool,
    /// Use compression level 7.
    #[arg(short = '7', conflicts_with_all = ["level_0", "level_1", "level_2", "level_3", "level_4", "level_5", "level_6", "level_8", "level_9"])]
    level_7: bool,
    /// Use compression level 8.
    #[arg(short = '8', conflicts_with_all = ["level_0", "level_1", "level_2", "level_3", "level_4", "level_5", "level_6", "level_7", "level_9"])]
    level_8: bool,
    /// Use compression level 9 (maximum compression).
    #[arg(short = '9', conflicts_with_all = ["level_0", "level_1", "level_2", "level_3", "level_4", "level_5", "level_6", "level_7", "level_8"])]
    level_9: bool,

    /// Exclude files or directories matching the given glob pattern(s).
    /// Can be specified multiple times for multiple patterns.
    #[arg(short = 'x', long)]
    exclude: Vec<String>,

    /// Enable verbose output, showing each file as it's added.
    #[arg(short = 'v', long)]
    verbose: bool,

    /// The path to the output zip file that will be created.
    output: String,

    /// One or more input files or directories to be added to the zip archive.
    #[arg(required = true)]
    inputs: Vec<String>,
}

/// # Main Entry Point
///
/// This is the `main` function for the `zip` command-line tool.
/// It orchestrates the entire archiving process: parsing arguments,
/// creating the zip file, configuring compression, traversing inputs,
/// and adding files/directories to the archive.
///
/// ## Workflow:
/// 1.  **Parse Arguments**: Uses `clap` to parse command-line arguments.
/// 2.  **Prepare Output Directory**: Ensures the parent directory for the output
///     zip file exists.
/// 3.  **Create Zip Writer**: Initializes a `zip::ZipWriter` for the output file.
/// 4.  **Determine Compression**: Sets the compression method and level based on
///     command-line flags. Defaults to DEFLATE with default level if none specified.
/// 5.  **Compile Exclude Patterns**: Converts `exclude` glob patterns into
///     `glob::Pattern` objects for efficient matching.
/// 6.  **Process Inputs**: Iterates through each `input` path provided:
///     -   Handles warnings for non-existent inputs.
///     -   If an input is a directory:
///         -   If `recurse` is true, it uses `walkdir::WalkDir` to traverse the
///             directory tree, adding files and directories.
///         -   If `recurse` is false, it only adds the top-level directory entry.
///     -   If an input is a file, it adds the file directly to the archive.
///     -   During processing, it respects `junk_paths` and `exclude` options.
///     -   Verbose output is printed if enabled.
/// 7.  **Finalize Archive**: Calls `zip.finish()` to write the central directory
///     and close the zip file.
/// 8.  **Confirmation**: Prints a success message if verbose output is enabled.
///
/// Error handling uses `anyhow` for context propagation.
///
/// # Returns
/// A `Result<()>` indicating the overall success or failure of the archiving process.
/// # Main Entry Point
///
/// This is the `main` function for the `zip` command-line tool.
/// It orchestrates the entire archiving process: parsing arguments,
/// creating the zip file, configuring compression, traversing inputs,
/// and adding files/directories to the archive.
///
/// ## Workflow:
/// 1.  **Parse Arguments**: Uses `clap` to parse command-line arguments.
/// 2.  **Prepare Output Directory**: Ensures the parent directory for the output
///     zip file exists.
/// 3.  **Create Zip Writer**: Initializes a `zip::ZipWriter` for the output file.
/// 4.  **Determine Compression**: Sets the compression method and level based on
///     command-line flags. Defaults to DEFLATE with default level if none specified.
/// 5.  **Compile Exclude Patterns**: Converts `exclude` glob patterns into
///     `glob::Pattern` objects for efficient matching.
/// 6.  **Process Inputs**: Iterates through each `input` path provided:
///     -   Handles warnings for non-existent inputs.
///     -   If an input is a directory:
///         -   If `recurse` is true, it uses `walkdir::WalkDir` to traverse the
///             directory tree, adding files and directories.
///         -   If `recurse` is false, it only adds the top-level directory entry.
///     -   If an input is a file, it adds the file directly to the archive.
///     -   During processing, it respects `junk_paths` and `exclude` options.
///     -   Verbose output is printed if enabled.
/// 7.  **Finalize Archive**: Calls `zip.finish()` to write the central directory
///     and close the zip file.
/// 8.  **Confirmation**: Prints a success message if verbose output is enabled.
///
/// Error handling uses `anyhow` for context propagation.
///
/// # Returns
/// A `Result<()>` indicating the overall success or failure of the archiving process.
fn main() -> Result<()> {
    /// Parses command-line arguments using `clap`.
    let args = Args::parse();

    /// 1. Prepares the output directory for the zip file.
    let output_path = Path::new(&args.output);
    if let Some(parent) = output_path.parent() {
        if !parent.exists() && !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent).context("Failed to create output directory for zip file")?;
        }
    }

    /// 2. Creates the output zip file and initializes `zip::ZipWriter`.
    let file = File::create(&output_path)
        .context(format!("Failed to create output zip file: {}", args.output))?;
    let mut zip = zip::ZipWriter::new(file);

    /// 3. Determines the compression method and level based on command-line arguments.
    let (method, level) = if args.level_0 {
        (zip::CompressionMethod::Stored, None) // No compression
    } else if args.level_1 {
        (zip::CompressionMethod::Deflated, Some(1))
    } else if args.level_2 {
        (zip::CompressionMethod::Deflated, Some(2))
    } else if args.level_3 {
        (zip::CompressionMethod::Deflated, Some(3))
    } else if args.level_4 {
        (zip::CompressionMethod::Deflated, Some(4))
    } else if args.level_5 {
        (zip::CompressionMethod::Deflated, Some(5))
    } else if args.level_6 {
        (zip::CompressionMethod::Deflated, Some(6)) // Default DEFLATE level
    } else if args.level_7 {
        (zip::CompressionMethod::Deflated, Some(7))
    } else if args.level_8 {
        (zip::CompressionMethod::Deflated, Some(8))
    } else if args.level_9 {
        (zip::CompressionMethod::Deflated, Some(9)) // Maximum compression
    } else {
        (zip::CompressionMethod::Deflated, None) // Default to DEFLATE with default level
    };

    /// Configures `FileOptions` for zip entries, including compression method, Unix permissions, and large file support.
    let mut options = FileOptions::<'_, ()>::default()
        .compression_method(method)
        .unix_permissions(0o755) // Standard executable permissions for Unix-like systems.
        .large_file(true); // Allow files larger than 4GB.

    if let Some(l) = level {
        options = options.compression_level(Some(l));
    }

    /// 4. Compiles exclude glob patterns into `glob::Pattern` objects for efficient matching.
    let exclude_patterns: Vec<Pattern> = args
        .exclude
        .iter()
        .map(|p| Pattern::new(p).context(format!("Invalid glob pattern: {}", p)))
        .collect::<Result<Vec<_>>>()?;

    /// Reusable buffer for reading file contents to reduce allocations.
    let mut buffer = Vec::new(); // Reusable buffer for reading file contents.

    /// 5. Processes each input path (file or directory) provided by the user.
    for input in &args.inputs {
        let input_path = Path::new(input);

        if !input_path.exists() {
            eprintln!("Warning: Input path '{}' not found, skipping.", input_path.display());
            continue;
        }

        if input_path.is_dir() {
            // Handle directories.
            if args.recurse {
                /// Recursively traverses directories using `walkdir::WalkDir`.
                for entry in WalkDir::new(input_path).into_iter().filter_map(|e| e.ok()) {
                    let path = entry.path();
                    let path_str = path.to_string_lossy();

                    /// Checks if the current path matches any exclusion pattern.
                    if exclude_patterns.iter().any(|p| p.matches(&path_str)) {
                        continue;
                    }

                    /// Determines the name under which the file/directory will be stored in the zip archive.
                    let name = if args.junk_paths {
                        // Only store the base filename.
                        path.file_name()
                            .unwrap_or(path.as_os_str())
                            .to_string_lossy()
                            .to_string()
                    } else {
                        // Store the relative path from the input root.
                        path.strip_prefix(input_path.parent().unwrap_or_else(|| Path::new(".")))
                            .unwrap_or(path)
                            .to_string_lossy()
                            .replace('\\', "/")
                    };

                    if path.is_dir() {
                        /// Adds a directory entry to the zip archive if not junking paths.
                        if !args.junk_paths {
                            if args.verbose && !args.quiet {
                                println!("adding: {}/ (stored 0%)", name);
                            }
                            zip.add_directory(&name, options)?;
                        }
                    } else if path.is_file() {
                        /// Adds a file entry to the zip archive.
                        if args.verbose && !args.quiet {
                            println!(
                                "adding: {} ({})",
                                name,
                                if method == zip::CompressionMethod::Stored { "stored" } else { "deflated" }
                            );
                        }
                        zip.start_file(&name, options)?;
                        let mut f = File::open(path)?;
                        f.read_to_end(&mut buffer)?;
                        zip.write_all(&buffer)?;
                        buffer.clear();
                    }
                }
            } else {
                /// Adds only the top-level directory entry if not recursing.
                let name = input_path.to_string_lossy().replace('\\', "/");
                if args.verbose && !args.quiet {
                    println!("adding: {}/ (stored 0%)", name);
                }
                zip.add_directory(&name, options)?;
            }
        } else if input_path.is_file() {
            // Handle individual files.
            let name = if args.junk_paths {
                input_path.file_name().unwrap_or(input_path.as_os_str()).to_string_lossy().to_string()
            } else {
                input_path.to_string_lossy().replace('\\', "/")
            };

            if args.verbose && !args.quiet {
                println!(
                    "adding: {} ({})",
                    name,
                    if method == zip::CompressionMethod::Stored { "stored" } else { "deflated" }
                );
            }
            zip.start_file(&name, options)?;
            let mut f = File::open(input_path)?;
            f.read_to_end(&mut buffer)?;
            zip.write_all(&buffer)?;
            buffer.clear();
        }
    }

    /// 6. Finalizes the zip archive, writing the central directory and closing the file.
    zip.finish()?;

    /// Prints a success message if not in quiet mode.
    if !args.quiet {
        println!("Archive created successfully: {}", output_path.display());
    }

    Ok(())
}
