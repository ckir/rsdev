//! # dir-to-yaml
//!
//! A high-performance command-line utility that converts a directory tree into a
//! structured YAML format. It provides features for filtering, excluding patterns,
//! and respecting `.gitignore` rules.

use clap::{ArgAction, Parser, ValueHint};
use ignore::overrides::OverrideBuilder;
use ignore::WalkBuilder;
use serde::Serialize;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// Represents a single node in the directory tree.
/// 
/// This structure is designed to be serialized into YAML, where keys are 
/// subdirectory names and values are nested `DirectoryNode` objects.
#[derive(Serialize, Default)]
pub struct DirectoryNode {
    /// A map of subdirectory names to their respective child nodes.
    /// The `flatten` attribute ensures the map keys appear directly in the YAML object.
    #[serde(flatten)]
    pub children: BTreeMap<String, DirectoryNode>,

    /// An optional list of filenames located directly within this directory.
    /// This field is omitted from the output if it is `None` or empty.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub files: Option<Vec<String>>,
}

/// The command-line argument structure for the application.
#[derive(Parser)]
#[clap(
    name = "dir-to-yaml",
    version = "1.0.0",
    author = "ckir",
    about = "Exports a directory structure to a clean YAML format.",
    long_about = "A utility that recursively scans a directory and generates a YAML representation of its hierarchy. \
                  It allows for powerful filtering using glob patterns and can optionally respect .gitignore rules."
)]
pub struct Cli {
    /// The root directory to scan.
    ///
    /// The scanner will traverse this path and use its base name as the root of the YAML tree.
    #[clap(value_parser, value_hint = ValueHint::DirPath, default_value = ".")]
    pub path: PathBuf,

    /// Exclude all files from the output.
    ///
    /// When set, the output will only contain the directory hierarchy (folders), providing a high-level overview.
    #[clap(long, action = ArgAction::SetTrue)]
    pub no_files: bool,

    /// Respect .gitignore files.
    ///
    /// If enabled, the tool will skip any files or directories that are ignored by your local .gitignore files.
    #[clap(long, short = 'g', action = ArgAction::SetTrue)]
    pub use_gitignore: bool,

    /// Custom exclusion patterns (e.g., --exclude \"target/*\" --exclude \"*.log\").
    ///
    /// You can provide multiple patterns. Patterns use standard glob syntax. 
    /// To exclude a folder entirely, use \"folder_name/*\".
    #[clap(long, short = 'e', value_name = "PATTERN", action = ArgAction::Append)]
    pub exclude: Vec<String>,
}

fn main() {
    // Parse the command line arguments provided by the user
    let args = Cli::parse();

    // Execute the core logic and handle the result
    match dir_to_yaml(&args.path, args.no_files, args.use_gitignore, &args.exclude) {
        Ok(yaml) => {
            // Print the successfully generated YAML to standard output
            println!("{}", yaml);
        }
        Err(e) => {
            // Print descriptive error messages to standard error
            eprintln!("Error: Failed to process directory.");
            eprintln!("Details: {}", e);
            // Exit with a non-zero status code to indicate failure
            std::process::exit(1);
        }
    }
}

/// Traverses the filesystem and generates a YAML string representing the directory structure.
///
/// # Arguments
/// * `path` - The starting directory for the scan.
/// * `no_files` - If true, file entries will be omitted from the output.
/// * `use_gitignore` - If true, the scan will respect local `.gitignore` rules.
/// * `excludes` - A slice of glob patterns to exclude from the results.
///
/// # Errors
/// Returns an error if the path is inaccessible or if serialization fails.
pub fn dir_to_yaml(
    path: &Path,
    no_files: bool,
    use_gitignore: bool,
    excludes: &[String],
) -> Result<String, Box<dyn std::error::Error>> {
    // Initialize the root of our internal tree structure
    let mut root_tree = DirectoryNode::default();

    // Initialize the override builder to handle manual exclusion patterns
    let mut override_builder = OverrideBuilder::new(path);
    for pattern in excludes {
        // Ensure the pattern is treated as an exclusion by prefixing with '!'
        let neg_pattern = if pattern.starts_with('!') {
            pattern.to_string()
        } else {
            format!("!{}", pattern)
        };
        // Add the exclusion pattern to the builder
        override_builder.add(&neg_pattern)?;
    }
    // Finalize the overrides configuration
    let overrides = override_builder.build()?;

    // Configure the recursive directory walker with user preferences
    let walker = WalkBuilder::new(path)
        .git_ignore(use_gitignore)
        .overrides(overrides)
        .hidden(false) // Include hidden files unless explicitly excluded by patterns
        .build();

    // Iterate through the directory entries, skipping the root path itself
    for result in walker.skip(1) {
        let entry = match result {
            Ok(e) => e,
            Err(err) => {
                // Log warnings for inaccessible files or permission issues
                eprintln!("Warning: Skipping entry due to error: {}", err);
                continue;
            }
        };

        // Determine the path relative to the provided root for tree construction
        let entry_path = entry.path();
        let relative_path = entry_path.strip_prefix(path).unwrap();

        // Start from the root and descend into the tree based on the parent components
        let mut current_node = &mut root_tree;
        if let Some(parent) = relative_path.parent() {
            for component in parent.components() {
                // Convert each path component into a string for the map key
                let component_str = component.as_os_str().to_str().unwrap().to_string();
                // Find or create the child node for this component
                current_node = current_node.children.entry(component_str).or_default();
            }
        }

        // Get the final segment of the path (the file or directory name)
        let file_name = relative_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .to_string();

        // Check if the current entry is a directory
        if entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
            // Ensure a node exists in the children map for this directory
            current_node.children.entry(file_name).or_default();
        } else if !no_files {
            // If it's a file and file tracking is enabled, add it to the list
            current_node
                .files
                .get_or_insert_with(Vec::new)
                .push(file_name);
        }
    }

    // Wrap the entire tree in a top-level map using the canonical name of the root path
    let mut root = BTreeMap::new();
    let root_name = path
        .canonicalize()
        .ok()
        .and_then(|p| p.file_name().map(|n| n.to_string_lossy().into_owned()))
        .unwrap_or_else(|| "root".to_string());

    // Insert the tree into the map
    root.insert(root_name, root_tree);

    // Serialize the BTreeMap into a YAML-formatted string and return it
    Ok(serde_yml::to_string(&root)?)
}
