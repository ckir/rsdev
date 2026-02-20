use clap::{ArgAction, Parser, ValueHint};
use ignore::overrides::OverrideBuilder;
use ignore::WalkBuilder;
use serde::Serialize;
use std::collections::BTreeMap;
use std::fs;
use std::io::Read;
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
    name = "repo-bundler", 
    version = "1.9.1", 
    about = "Bundles a directory. YAML structure strictly respects ALL .gitignore files."
)]
pub struct Cli {
    #[clap(value_parser, value_hint = ValueHint::DirPath, default_value = ".")]
    pub path: PathBuf,

    #[clap(long, short = 'g', action = ArgAction::SetTrue)]
    pub use_gitignore: bool,

    #[clap(long, short = 'e', value_name = "PATTERN", action = ArgAction::Append)]
    pub exclude: Vec<String>,

    #[clap(long, short = 'o', value_hint = ValueHint::DirPath, default_value = ".")]
    pub output: PathBuf,

    #[clap(long, short = 'm', value_name = "CHARS")]
    pub max: Option<usize>,
}

fn main() {
    let args = Cli::parse();
    if let Err(e) = run(&args) {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }
}

fn run(args: &Cli) -> Result<(), Box<dyn std::error::Error>> {
    let mut root_tree = DirectoryNode::default();
    let mut file_contents = Vec::new();

    // 1. Setup manual overrides
    let mut override_builder = OverrideBuilder::new(&args.path);
    for pattern in &args.exclude {
        override_builder.add(&format!("!{}", pattern))?;
    }
    let overrides = override_builder.build()?;

    // 2. Configure Walker 
    // We use WalkBuilder::new(&args.path) to anchor the search to the target folder.
    let mut builder = WalkBuilder::new(&args.path);
    builder
        .overrides(overrides)
        .hidden(false);

    if args.use_gitignore {
        builder
            .git_ignore(true)
            .require_git(false) // Respect .gitignore even if not a formal git repo
            .add_custom_ignore_filename(".gitignore"); // Look for .gitignore at every level
    }

    let walker = builder.build();

    // 3. Process entries: Build YAML and collect content in one pass
    for result in walker.skip(1) {
        let entry = match result {
            Ok(e) => e,
            Err(err) => {
                eprintln!("Warning: Skipping entry: {}", err);
                continue;
            }
        };

        // The walker yields paths NOT ignored. We use this to build a mirrored YAML tree.
        let entry_path = entry.path();
        let rel_path = entry_path.strip_prefix(&args.path).unwrap();

        let mut current_node = &mut root_tree;
        if let Some(parent) = rel_path.parent() {
            for component in parent.components() {
                let comp_str = component.as_os_str().to_str().unwrap().to_string();
                current_node = current_node.children.entry(comp_str).or_default();
            }
        }

        let name = rel_path.file_name().unwrap().to_string_lossy().into_owned();

        if entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
            current_node.children.entry(name).or_default();
        } else {
            if let Ok(content) = read_file_if_text(entry_path) {
                current_node.files.get_or_insert_with(Vec::new).push(name.clone());
                file_contents.push((rel_path.to_string_lossy().into_owned(), content));
            }
        }
    }

    let root_name = args.path.canonicalize()?
        .file_name().map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "root".to_string());

    let mut wrap = BTreeMap::new();
    wrap.insert(root_name.clone(), root_tree);
    
    // FIX: Corrected variable name from 'root' to 'wrap'
    let yaml_structure = serde_yml::to_string(&wrap)?;
    
    let mut part_number = 1;
    let mut current_buffer = format!("--- PART {} ---\n--- REPO STRUCTURE ---\n{}\n", part_number, yaml_structure);

    // 4. Bundling Logic
    for (path, content) in file_contents {
        let file_block = format!("\n--- FILE: {} ---\n{}\n--- END FILE ---\n", path, content);
        
        if let Some(max_chars) = args.max {
            if current_buffer.len() + file_block.len() > max_chars {
                save_part(&args.output, &root_name, part_number, &current_buffer, true)?;
                part_number += 1;
                current_buffer = format!("--- PART {} ---\n", part_number);
            }
        }
        current_buffer.push_str(&file_block);
    }

    save_part(&args.output, &root_name, part_number, &current_buffer, args.max.is_some())?;
    Ok(())
}

fn save_part(dir: &Path, root_name: &str, part: usize, content: &str, is_split: bool) -> Result<(), std::io::Error> {
    let filename = if is_split { format!("{}_part{}.txt", root_name, part) } else { format!("{}.txt", root_name) };
    let path = dir.join(filename);
    fs::write(&path, content)?;
    println!("Saved: {:?}", path);
    Ok(())
}

fn read_file_if_text(path: &Path) -> Result<String, std::io::Error> {
    let mut f = fs::File::open(path)?;
    let mut buffer = Vec::new();
    // Resolve trait ambiguity between Read and Write
    Read::by_ref(&mut f).take(1024).read_to_end(&mut buffer)?;
    
    if buffer.contains(&0u8) { 
        return Err(std::io::Error::new(std::io::ErrorKind::InvalidData, "Binary")); 
    }

    let mut rest = String::new();
    let mut full_content = String::from_utf8_lossy(&buffer).into_owned();
    f.read_to_string(&mut rest)?;
    full_content.push_str(&rest);
    Ok(full_content)
}