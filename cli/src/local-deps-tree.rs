//! # Universal Workspace Deep Tracer
//!
//! A professional-grade utility to map recursive internal dependencies 
//! from binary entry points down to specific library modules.

use cargo_metadata::MetadataCommand;
use clap::Parser;
use std::collections::{BTreeSet, HashMap};
use std::fs;
use std::path::{Path, PathBuf};

/// Analyzes your Rust workspace to map how binaries depend on internal modules.
/// 
/// It parses 'use' statements recursively, handling blocks like 'use crate::{a, b}'
/// and cross-crate references like 'use lib_common::module'.
#[derive(Parser)]
#[clap(
    name = "deep-deps", 
    version = "1.0", 
    author = "Gemini",
    about = "Maps recursive module-level dependencies for Rust binaries."
)]
pub struct Cli {
    /// The root directory of the Rust project to analyze.
    #[clap(
        value_parser, 
        default_value = ".",
        help = "Path to the project root (must contain a Cargo.toml)"
    )]
    pub root: PathBuf,

    /// Filter by a specific binary filename.
    #[clap(
        long, 
        short = 'b', 
        help = "Only analyze a specific binary (e.g., restream.rs). If omitted, all binaries are scanned."
    )]
    pub bin: Option<String>,

    /// Increase output detail.
    #[clap(long, short = 'v', help = "Display verbose path resolution logs.")]
    pub verbose: bool,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Cli::parse();
    
    // Check if the target directory is a Rust project
    let manifest_path = args.root.join("Cargo.toml");
    if !manifest_path.exists() {
        eprintln!("Error: The directory '{:?}' is not a Rust project.", args.root);
        eprintln!("Please run this tool from a directory containing a Cargo.toml.");
        std::process::exit(1);
    }

    // Attempt to fetch metadata
    let metadata_res = MetadataCommand::new()
        .manifest_path(&manifest_path)
        .no_deps()
        .exec();

    let metadata = match metadata_res {
        Ok(m) => m,
        Err(e) => {
            eprintln!("Error: Failed to parse Cargo metadata: {}", e);
            std::process::exit(1);
        }
    };

    let mut crate_map = HashMap::new();
    let mut all_packages = HashMap::new();

    for pkg in &metadata.packages {
        let src_path = pkg.manifest_path.parent().unwrap().join("src");
        crate_map.insert(pkg.name.to_string(), src_path.into_std_path_buf());
        all_packages.insert(pkg.id.clone(), pkg.clone());
    }

    println!("\nRust Dependency Explorer: Deep Trace");
    println!("{:=<50}", "");

    for pkg_id in &metadata.workspace_members {
        let pkg = &all_packages[pkg_id];
        let pkg_src_root = pkg.manifest_path.parent().unwrap().join("src").into_std_path_buf();

        for target in &pkg.targets {
            if target.kind.iter().any(|k| k.to_string() == "bin") {
                let bin_path = target.src_path.clone().into_std_path_buf();
                let bin_name = bin_path.file_name().unwrap().to_str().unwrap();

                if let Some(ref filter) = args.bin {
                    if bin_name != filter { continue; }
                }

                println!("\n{} (Crate: {})", bin_name, pkg.name);
                let mut visited = BTreeSet::new();
                trace_deps(&bin_path, &pkg_src_root, &crate_map, 1, &mut visited);
            }
        }
    }
    
    println!("\nAnalysis Complete.");
    Ok(())
}

fn trace_deps(path: &Path, root: &Path, crate_map: &HashMap<String, PathBuf>, depth: usize, visited: &mut BTreeSet<PathBuf>) {
    let can_path = match path.canonicalize() { Ok(p) => p, Err(_) => return };
    if !visited.insert(can_path) { return; }

    let content = match fs::read_to_string(path) { Ok(c) => c, Err(_) => return };
    let indent = "  ".repeat(depth);

    for line in content.lines() {
        let line = line.trim();
        if !line.starts_with("use ") { continue; }

        let raw_path = line.replacen("use ", "", 1).trim_matches(';').to_string();
        
        if let Some(brace_start) = raw_path.find('{') {
            let base_path = raw_path[..brace_start].trim_matches(':');
            let items = raw_path[brace_start + 1..].trim_matches('}').split(',');

            for item in items {
                let trimmed = item.trim();
                if trimmed.is_empty() { continue; }
                let full_path = format!("{}::{}", base_path, trimmed);
                process_import(&full_path, root, crate_map, &indent, visited);
            }
        } else {
            process_import(&raw_path, root, crate_map, &indent, visited);
        }
    }
}

fn process_import(import: &str, root: &Path, crate_map: &HashMap<String, PathBuf>, indent: &str, visited: &mut BTreeSet<PathBuf>) {
    let parts: Vec<&str> = import.split("::").collect();
    let mut target_rs: Option<PathBuf> = None;
    let mut next_root = root;

    if parts[0] == "crate" || parts[0] == "self" {
        target_rs = resolve_module_path(root, &parts[1..]);
    } else if let Some(base_path) = crate_map.get(parts[0]) {
        target_rs = resolve_module_path(base_path, &parts[1..]);
        next_root = base_path;
    }

    if let Some(resolved) = target_rs {
        println!("{}{} {}", indent, "└──", import);
        trace_deps(&resolved, next_root, crate_map, (indent.len() / 2) + 1, visited);
    }
}

fn resolve_module_path(base: &Path, components: &[&str]) -> Option<PathBuf> {
    if components.is_empty() { return None; }
    let mut current = base.to_path_buf();
    let mut last_valid = None;

    for comp in components {
        current.push(comp);
        let file_rs = current.with_extension("rs");
        if file_rs.exists() { last_valid = Some(file_rs); }
        let mod_rs = current.join("mod.rs");
        if mod_rs.exists() { last_valid = Some(mod_rs); }
    }
    last_valid
}