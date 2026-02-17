//! # dir-to-json
//!
//! A high-performance command-line utility that converts a directory tree into a
//! structured JSON format. Respects .gitignore and creates a file by default.

use clap::{ArgAction, Parser, ValueHint};
use ignore::overrides::OverrideBuilder;
use ignore::WalkBuilder;
use serde::Serialize;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Serialize, Default)]
pub struct DirectoryNode {
    #[serde(flatten)]
    pub children: BTreeMap<String, DirectoryNode>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub files: Option<Vec<String>>,
}

#[derive(Parser)]
#[clap(
    name = "dir-to-json",
    version = "1.3.0",
    author = "ckir",
    about = "Exports a directory structure to JSON file."
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

    /// Custom output path. Defaults to [folder-name].json
    #[clap(long, short = 'o', value_hint = ValueHint::FilePath)]
    pub output: Option<PathBuf>,
}

fn main() {
    let args = Cli::parse();

    // Determine the root name for the file and the JSON object
    let canonical_path = args.path.canonicalize().unwrap_or_else(|_| args.path.clone());
    let root_name = canonical_path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "root".to_string());

    // Logic for default filename if --output is not provided
    let output_file = args.output.clone().unwrap_or_else(|| {
        PathBuf::from(format!("{}.json", root_name))
    });

    match dir_to_json(&args.path, &root_name, args.no_files, args.use_gitignore, &args.exclude, args.minify) {
        Ok(json) => {
            if let Err(e) = fs::write(&output_file, json) {
                eprintln!("Error: Failed to write to file {:?}.", output_file);
                eprintln!("Details: {}", e);
                std::process::exit(1);
            }
            println!("Successfully saved JSON to {:?}", output_file);
        }
        Err(e) => {
            eprintln!("Error: Failed to process directory.");
            eprintln!("Details: {}", e);
            std::process::exit(1);
        }
    }
}

pub fn dir_to_json(
    path: &Path,
    root_name: &str,
    no_files: bool,
    use_gitignore: bool,
    excludes: &[String],
    minify: bool,
) -> Result<String, Box<dyn std::error::Error>> {
    let mut root_tree = DirectoryNode::default();

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

    let mut builder = WalkBuilder::new(path);
    builder.overrides(overrides).hidden(false);

    if use_gitignore {
        builder
            .git_ignore(true)
            .git_global(true)
            .git_exclude(true)
            .require_git(false) 
            .add_custom_ignore_filename(".gitignore"); 
    }

    let walker = builder.build();

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

    let mut wrap = BTreeMap::new();
    wrap.insert(root_name.to_string(), root_tree);

    let json = if minify {
        serde_json::to_string(&wrap)?
    } else {
        serde_json::to_string_pretty(&wrap)?
    };

    Ok(json)
}