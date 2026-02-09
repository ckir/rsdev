//! # `dir-to-yaml`: Directory Structure Exporter
//!
//! This command-line utility provides a simple yet powerful way to export a
//! given directory's structure into a YAML representation. It's particularly
//! useful for generating documentation, configuration templates, or for
//! visualizing project layouts.
//!
//! ## Key Features:
//! - **Recursive Traversal**: Scans directories recursively to capture the
//!   full hierarchy.
//! - **YAML Output**: Generates a clean, human-readable YAML string representing
//!   the directory tree.
//! - **File Exclusion**: Option to exclude all files from the output, focusing
//!   only on the directory structure.
//! - **`.gitignore` Integration**: Can respect `.gitignore` rules to filter out
//!   unwanted files and directories, mimicking a clean project view.
//!
//! The output YAML represents directories as nested maps and files as a list
//! within their parent directory's node.

#![doc(html_logo_url = "https://example.com/logo.png")] // Placeholder for consistency
#![forbid(unsafe_code)]
#![warn(missing_docs, rust_2018_idioms, unused_qualifications)]

use clap::Parser;
use serde::Serialize;
use serde_yml;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use ignore::WalkBuilder;

/// # Command Line Interface Arguments
///
/// Defines the arguments accepted by the `dir-to-yaml` CLI tool using `clap`.
/// # Command Line Interface Arguments
///
/// Defines the arguments accepted by the `dir-to-yaml` CLI tool using `clap`.
#[derive(Parser)]
#[clap(name = "dir-to-yaml", about = "Exports a directory structure to YAML.")]
struct Cli {
    /// The root directory to scan and convert to YAML.
    path: PathBuf,

    /// If set, no individual files will be included in the YAML output;
    /// only the directory structure will be shown.
    #[clap(long)]
    no_files: bool,

    /// If set, the tool will respect `.gitignore` files found in the
    /// scanned directory and its parents, excluding matching files/directories
    /// from the YAML output. This is useful for generating a clean project view.
    #[clap(long)]
    use_gitignore: bool,
}

/// # Directory Node Representation
///
/// Represents a single node (either a directory or a collection of files)
/// in the directory tree for YAML serialization.
/// # Directory Node Representation
///
/// Represents a single node (either a directory or a collection of files)
/// in the directory tree for YAML serialization.
#[derive(Serialize, Default)]
struct DirectoryNode {
    /// Child directories, represented as a `BTreeMap` to maintain sorted order
    /// in the YAML output for readability.
    #[serde(flatten)]
    children: BTreeMap<String, DirectoryNode>,
    /// A list of file names directly within this directory node.
    /// It is omitted from the YAML output if empty or `None`.
    #[serde(skip_serializing_if = "Option::is_none")]
    files: Option<Vec<String>>,
}

/// # Main Entry Point
///
/// Parses command-line arguments, calls the `dir_to_yaml` function, and prints
/// the resulting YAML or an error message.
/// # Main Entry Point
///
/// Parses command-line arguments, calls the `dir_to_yaml` function, and prints
/// the resulting YAML or an error message.
fn main() {
    let args = Cli::parse();
    let result = dir_to_yaml(&args.path, args.no_files, args.use_gitignore);
    match result {
        Ok(yaml) => println!("{}", yaml),
        Err(e) => eprintln!("Error: {}", e),
    }
}

/// # Directory to YAML Conversion Logic
///
/// Traverses a given directory and constructs a YAML string representing its structure.
///
/// ## Logic:
/// 1.  Initializes an empty `DirectoryNode` to serve as the root of the tree.
/// 2.  Uses `ignore::WalkBuilder` to efficiently traverse the directory, optionally
///     respecting `.gitignore` rules. `ignore` is chosen for its performance and
///     robust handling of `.gitignore` files.
/// 3.  Iterates through each entry found by the walker (skipping the root directory itself).
/// 4.  For each entry, it determines its relative path and navigates the `root_tree`
///     to the correct parent node.
/// 5.  If the entry is a directory, it ensures a corresponding `DirectoryNode` exists
///     in the parent's `children`.
/// 6.  If the entry is a file and `no_files` is `false`, its name is added to the
///     `files` list of its parent `DirectoryNode`.
/// 7.  Finally, it wraps the constructed `root_tree` within a `BTreeMap` using the
///     actual name of the scanned directory and serializes it to a YAML string.
///
/// ## Arguments
/// * `path` - A reference to the `Path` of the directory to scan.
/// * `no_files` - A boolean indicating whether to exclude files from the output.
/// * `use_gitignore` - A boolean indicating whether to respect `.gitignore` rules.
///
/// # Returns
/// A `Result` containing the YAML string on success or a `serde_yml::Error` on failure.
/// # Directory to YAML Conversion Logic
///
/// Traverses a given directory and constructs a YAML string representing its structure.
///
/// ## Logic:
/// 1.  Initializes an empty `DirectoryNode` to serve as the root of the tree.
/// 2.  Uses `ignore::WalkBuilder` to efficiently traverse the directory, optionally
///     respecting `.gitignore` rules. `ignore` is chosen for its performance and
///     robust handling of `.gitignore` files.
/// 3.  Iterates through each entry found by the walker (skipping the root directory itself).
/// 4.  For each entry, it determines its relative path and navigates the `root_tree`
///     to the correct parent node.
/// 5.  If the entry is a directory, it ensures a corresponding `DirectoryNode` exists
///     in the parent's `children`.
/// 6.  If the entry is a file and `no_files` is `false`, its name is added to the
///     `files` list of its parent `DirectoryNode`.
/// 7.  Finally, it wraps the constructed `root_tree` within a `BTreeMap` using the
///     actual name of the scanned directory and serializes it to a YAML string.
///
/// ## Arguments
/// * `path` - A reference to the `Path` of the directory to scan.
/// * `no_files` - A boolean indicating whether to exclude files from the output.
/// * `use_gitignore` - A boolean indicating whether to respect `.gitignore` rules.
///
/// # Returns
/// A `Result` containing the YAML string on success or a `serde_yml::Error` on failure.
fn dir_to_yaml(path: &Path, no_files: bool, use_gitignore: bool) -> Result<String, serde_yml::Error> {
    /// Initializes an empty `DirectoryNode` which will be populated with the directory structure.
    let mut root_tree = DirectoryNode::default();
    
    /// `WalkBuilder` is used for efficient and configurable directory traversal,
    /// especially for respecting .gitignore rules.
    let walker = WalkBuilder::new(path)
        .git_ignore(use_gitignore)
        .build();

    /// Skips the root directory itself, as its name will be used explicitly later for the top-level YAML key.
    for result in walker.skip(1) { 
        let entry = result.expect("Error walking directory entry");
        let entry_path = entry.path();
        
        /// Gets the path relative to the initial `path` argument to correctly build the YAML hierarchy.
        let relative_path = entry_path.strip_prefix(path).expect("Path should be within root");

        /// Navigates `current_node` down the `root_tree` to the correct parent for the current entry.
        let mut current_node = &mut root_tree;

        if let Some(parent) = relative_path.parent() {
            for component in parent.components() {
                let component_str = component.as_os_str().to_str().expect("Invalid UTF-8 in path").to_string();
                current_node = current_node.children.entry(component_str).or_default();
            }
        }

        /// Adds the current entry to the tree structure.
        if entry.file_type().expect("Could not determine file type").is_dir() {
            if let Some(dir_name) = relative_path.file_name() {
                // Ensure the directory node exists, even if empty.
                current_node.children.entry(dir_name.to_str().expect("Invalid UTF-8 in dir name").to_string()).or_default();
            }
        } else if !no_files {
            if let Some(file_name) = relative_path.file_name() {
                // Add the file name to the current node's files list.
                current_node.files.get_or_insert_with(Vec::new).push(file_name.to_str().expect("Invalid UTF-8 in file name").to_string());
            }
        }
    }

    /// Wraps the constructed directory tree in a single root `BTreeMap` with the scanned directory's name as the key.
    let mut root = BTreeMap::new();
    let root_name = path.file_name().and_then(|s| s.to_str()).unwrap_or(".").to_string();
    root.insert(root_name, root_tree);
    
    /// Serializes the final `BTreeMap` structure to a YAML string.
    serde_yml::to_string(&root)
}