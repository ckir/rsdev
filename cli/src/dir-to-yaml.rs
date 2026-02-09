use clap::Parser;
use serde::Serialize;
use serde_yml;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use ignore::WalkBuilder;

#[derive(Parser)]
#[clap(name = "dir-to-yaml", about = "Exports a directory structure to YAML.")]
struct Cli {
    /// The directory to scan.
    path: PathBuf,

    /// Exclude files from the output.
    #[clap(long)]
    no_files: bool,

    /// Exclude items from output based on .gitignore files.
    #[clap(long)]
    use_gitignore: bool,
}

#[derive(Serialize, Default)]
struct DirectoryNode {
    #[serde(flatten)]
    children: BTreeMap<String, DirectoryNode>,
    #[serde(skip_serializing_if = "Option::is_none")]
    files: Option<Vec<String>>,
}

fn main() {
    let args = Cli::parse();
    let result = dir_to_yaml(&args.path, args.no_files, args.use_gitignore);
    match result {
        Ok(yaml) => println!("{}", yaml),
        Err(e) => eprintln!("Error: {}", e),
    }
}

fn dir_to_yaml(path: &Path, no_files: bool, use_gitignore: bool) -> Result<String, serde_yml::Error> {
    let mut root_tree = DirectoryNode::default();
    let walker = WalkBuilder::new(path)
        .git_ignore(use_gitignore)
        .build();

    for result in walker.skip(1) { // Skip the root directory itself
        let entry = result.unwrap();
        let entry_path = entry.path();
        let relative_path = entry_path.strip_prefix(path).unwrap();

        let mut current_node = &mut root_tree;

        if let Some(parent) = relative_path.parent() {
            for component in parent.components() {
                let component_str = component.as_os_str().to_str().unwrap().to_string();
                current_node = current_node.children.entry(component_str).or_default();
            }
        }

        if entry.file_type().unwrap().is_dir() {
            if let Some(dir_name) = relative_path.file_name() {
                current_node.children.entry(dir_name.to_str().unwrap().to_string()).or_default();
            }
        } else if !no_files {
            if let Some(file_name) = relative_path.file_name() {
                current_node.files.get_or_insert_with(Vec::new).push(file_name.to_str().unwrap().to_string());
            }
        }
    }

    let mut root = BTreeMap::new();
    let root_name = path.file_name().and_then(|s| s.to_str()).unwrap_or(".").to_string();
    root.insert(root_name, root_tree);
    
    serde_yml::to_string(&root)
}