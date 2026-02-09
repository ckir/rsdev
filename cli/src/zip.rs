use anyhow::{Context, Result};
use clap::Parser;
use glob::Pattern;
use std::fs::File;
use std::io::{Read, Write};
use std::path::Path;
use walkdir::WalkDir;
use zip::write::FileOptions;

#[derive(Parser, Debug)]
#[command(
    name = "zip",
    about = "A Rust zip utility similar to linux zip",
    version
)]
struct Args {
    /// Recurse into directories
    #[arg(short = 'r', long)]
    recurse: bool,

    /// Quiet mode
    #[arg(short = 'q', long)]
    quiet: bool,

    /// Junk the path (store just file name)
    #[arg(short = 'j', long)]
    junk_paths: bool,

    /// Compression level 0 (Store)
    #[arg(short = '0', conflicts_with_all = ["level_1", "level_2", "level_3", "level_4", "level_5", "level_6", "level_7", "level_8", "level_9"])]
    level_0: bool,
    #[arg(short = '1', conflicts_with_all = ["level_0", "level_2", "level_3", "level_4", "level_5", "level_6", "level_7", "level_8", "level_9"])]
    level_1: bool,
    #[arg(short = '2', conflicts_with_all = ["level_0", "level_1", "level_3", "level_4", "level_5", "level_6", "level_7", "level_8", "level_9"])]
    level_2: bool,
    #[arg(short = '3', conflicts_with_all = ["level_0", "level_1", "level_2", "level_4", "level_5", "level_6", "level_7", "level_8", "level_9"])]
    level_3: bool,
    #[arg(short = '4', conflicts_with_all = ["level_0", "level_1", "level_2", "level_3", "level_5", "level_6", "level_7", "level_8", "level_9"])]
    level_4: bool,
    #[arg(short = '5', conflicts_with_all = ["level_0", "level_1", "level_2", "level_3", "level_4", "level_6", "level_7", "level_8", "level_9"])]
    level_5: bool,
    #[arg(short = '6', conflicts_with_all = ["level_0", "level_1", "level_2", "level_3", "level_4", "level_5", "level_7", "level_8", "level_9"])]
    level_6: bool,
    #[arg(short = '7', conflicts_with_all = ["level_0", "level_1", "level_2", "level_3", "level_4", "level_5", "level_6", "level_8", "level_9"])]
    level_7: bool,
    #[arg(short = '8', conflicts_with_all = ["level_0", "level_1", "level_2", "level_3", "level_4", "level_5", "level_6", "level_7", "level_9"])]
    level_8: bool,
    #[arg(short = '9', conflicts_with_all = ["level_0", "level_1", "level_2", "level_3", "level_4", "level_5", "level_6", "level_7", "level_8"])]
    level_9: bool,

    /// Exclude files matching pattern
    #[arg(short = 'x', long)]
    exclude: Vec<String>,

    /// Verbose output
    #[arg(short = 'v', long)]
    verbose: bool,

    /// Output zip file path
    output: String,

    /// Input files or directories
    #[arg(required = true)]
    inputs: Vec<String>,
}

fn main() -> Result<()> {
    let args = Args::parse();

    let output_path = Path::new(&args.output);
    if let Some(parent) = output_path.parent() {
        if !parent.exists() && !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent).context("Failed to create output directory")?;
        }
    }

    let file = File::create(&output_path)
        .context(format!("Failed to create output file: {}", args.output))?;
    let mut zip = zip::ZipWriter::new(file);

    let (method, level) = if args.level_0 {
        (zip::CompressionMethod::Stored, None)
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
        (zip::CompressionMethod::Deflated, Some(6))
    } else if args.level_7 {
        (zip::CompressionMethod::Deflated, Some(7))
    } else if args.level_8 {
        (zip::CompressionMethod::Deflated, Some(8))
    } else if args.level_9 {
        (zip::CompressionMethod::Deflated, Some(9))
    } else {
        (zip::CompressionMethod::Deflated, None)
    };

    let mut options = FileOptions::<'_, ()>::default()
        .compression_method(method)
        .unix_permissions(0o755);

    if let Some(l) = level {
        options = options.compression_level(Some(l));
    }

    let exclude_patterns: Vec<Pattern> = args
        .exclude
        .iter()
        .map(|p| Pattern::new(p).context(format!("Invalid glob pattern: {}", p)))
        .collect::<Result<Vec<_>>>()?;

    let mut buffer = Vec::new();

    for input in &args.inputs {
        let input_path = Path::new(input);

        if !input_path.exists() {
            eprintln!("Warning: '{}' not found, skipping.", input);
            continue;
        }

        if input_path.is_dir() {
            if args.recurse {
                for entry in WalkDir::new(input_path) {
                    let entry = entry.context("Failed to read directory entry")?;
                    let path = entry.path();

                    let path_str = path.to_string_lossy();
                    if exclude_patterns.iter().any(|p| p.matches(&path_str)) {
                        continue;
                    }

                    // Normalize path separators for ZIP (forward slashes)
                    let name = if args.junk_paths {
                        path.file_name()
                            .unwrap_or(path.as_os_str())
                            .to_string_lossy()
                            .to_string()
                    } else {
                        path.to_string_lossy().replace('\\', "/")
                    };

                    if path.is_dir() {
                        if !args.junk_paths {
                            if args.verbose && !args.quiet {
                                println!("adding: {}/ (stored 0%)", name);
                            }
                            zip.add_directory(&name, options)?;
                        }
                    } else {
                        if args.verbose && !args.quiet {
                            println!(
                                "adding: {} ({})",
                                name,
                                if method == zip::CompressionMethod::Stored {
                                    "stored"
                                } else {
                                    "deflated"
                                }
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
                // Add directory entry only
                let name = input_path.to_string_lossy().replace('\\', "/");
                if args.verbose {
                    println!("adding: {}/ (stored 0%)", name);
                }
                zip.add_directory(&name, options)?;
            }
        } else {
            // File
            let name = input_path.to_string_lossy().replace('\\', "/");
            if args.verbose {
                println!("adding: {} (deflated)", name);
            }
            zip.start_file(&name, options)?;
            let mut f = File::open(input_path)?;
            f.read_to_end(&mut buffer)?;
            zip.write_all(&buffer)?;
            buffer.clear();
        }
    }

    zip.finish()?;

    if args.verbose && !args.quiet {
        println!("Archive created successfully: {}", args.output);
    }

    Ok(())
}
