use anyhow::{Context, Result};
use clap::Parser;
use serde_json::Value;
use std::fs;
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(
    name = "js-paths",
    version,
    about = "Output all paths in a JSON/JSON5 file"
)]
struct Args {
    /// Input JSON/JSON5 file
    #[arg(required = true)]
    input: PathBuf,
}

fn main() -> Result<()> {
    let args = Args::parse();

    let content = fs::read_to_string(&args.input)
        .context(format!("Failed to read file: {:?}", args.input))?;

    let v: Value = serde_json5::from_str(&content).context("Failed to parse JSON/JSON5")?;

    print_paths(&v, String::new());

    Ok(())
}

fn print_paths(v: &Value, current_path: String) {
    if !current_path.is_empty() {
        println!("{}", current_path);
    }

    match v {
        Value::Object(map) => {
            // Collect and sort keys to ensure deterministic output
            let mut keys: Vec<_> = map.keys().collect();
            keys.sort();

            for k in keys {
                let val = &map[k];
                let new_path = if current_path.is_empty() {
                    k.to_string()
                } else {
                    format!("{}.{}", current_path, k)
                };
                print_paths(val, new_path);
            }
        }
        Value::Array(arr) => {
            for (i, val) in arr.iter().enumerate() {
                let new_path = if current_path.is_empty() {
                    i.to_string()
                } else {
                    format!("{}.{}", current_path, i)
                };
                print_paths(val, new_path);
            }
        }
        _ => {}
    }
}
