//! # dir-to-json
//!
//! A high-performance command-line utility that converts a directory tree into a
//! structured JSON format. Supports exclusion patterns, .gitignore rules, and
//! optional minified output.

use clap::{ArgAction, Parser, ValueHint};
use ignore::overrides::OverrideBuilder;
use ignore::WalkBuilder;
use serde::Serialize;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// Represents a directory node in the JSON tree.
#[derive(Serialize, Default)]
pub struct DirectoryNode {
    #[serde(flatten)]
    pub children: BTreeMap<String, DirectoryNode>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub files: Option<Vec<String>>,
}

/// CLI arguments for dir-to-json.
#[derive(Parser)]
#[clap(
    name = "dir-to-json",
    version = "1.0.0",
    author = "ckir",
    about = "Exports a directory structure to JSON.",
    long_about = "Recursively scans a directory and generates a JSON representation of its hierarchy. \
                  Supports exclusion patterns, .gitignore rules, and optional minified output."
)]
pub struct Cli {
    /// Root directory to scan.
    #[clap(value_parser, value_hint = ValueHint::DirPath, default_value = ".")]
    pub path: PathBuf,

    /// Exclude all files from the output.
    #[clap(long, action = ArgAction::SetTrue)]
    pub no_files: bool,

    /// Respect .gitignore files.
    #[clap(long, short = 'g', action = ArgAction::SetTrue)]
    pub use_gitignore: bool,

    /// Custom exclusion patterns.
    #[clap(long, short = 'e', value_name = "PATTERN", action = ArgAction::Append)]
    pub exclude: Vec<String>,

    /// Output minified JSON instead of pretty-printed.
    #[clap(long, action = ArgAction::SetTrue)]
    pub minify: bool,
}

fn main() {
    let args = Cli::parse();

    match dir_to_json(&args.path, args.no_files, args.use_gitignore, &args.exclude, args.minify) {
        Ok(json) => println!("{}", json),
        Err(e) => {
            eprintln!("Error: Failed to process directory.");
            eprintln!("Details: {}", e);
            std::process::exit(1);
        }
    }
}

/// Core logic for directory → JSON conversion.
pub fn dir_to_json(
    path: &Path,
    no_files: bool,
    use_gitignore: bool,
    excludes: &[String],
    minify: bool,
) -> Result<String, Box<dyn std::error::Error>> {
    let mut root_tree = DirectoryNode::default();

    // Build exclusion overrides
    let mut override_builder = OverrideBuilder::new(path);
    for pattern in excludes {
        let neg_pattern = if pattern.starts_with('!') {
            pattern.to_string()
        } else {
            format!("!{}", pattern)
        };
        override_builder.add(&neg_pattern)?;
    }
    let overrides = override_builder.build()?;

    // Configure directory walker
    let walker = WalkBuilder::new(path)
        .git_ignore(use_gitignore)
        .overrides(overrides)
        .hidden(false)
        .build();

    for result in walker.skip(1) {
        let entry = match result {
            Ok(e) => e,
            Err(err) => {
                eprintln!("Warning: Skipping entry due to error: {}", err);
                continue;
            }
        };

        let entry_path = entry.path();
        let relative_path = entry_path.strip_prefix(path).unwrap();

        let mut current_node = &mut root_tree;

        if let Some(parent) = relative_path.parent() {
            for component in parent.components() {
                let component_str = component.as_os_str().to_str().unwrap().to_string();
                current_node = current_node.children.entry(component_str).or_default();
            }
        }

        let file_name = relative_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .to_string();

        if entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
            current_node.children.entry(file_name).or_default();
        } else if !no_files {
            current_node.files.get_or_insert_with(Vec::new).push(file_name);
        }
    }

    // Wrap in root object
    let mut root = BTreeMap::new();
    let root_name = path
        .canonicalize()
        .ok()
        .and_then(|p| p.file_name().map(|n| n.to_string_lossy().into_owned()))
        .unwrap_or_else(|| "root".to_string());

    root.insert(root_name, root_tree);

    // Serialize JSON
    let json = if minify {
        serde_json::to_string(&root)?
    } else {
        serde_json::to_string_pretty(&root)?
    };

    Ok(json)
}
