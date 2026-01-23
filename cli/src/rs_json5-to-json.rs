use clap::Parser;
use std::fs;
use std::io::{self, Write};
use std::path::PathBuf;

/// A simple CLI tool to convert a JSON5 file to a JSON file.
#[derive(Parser, Debug)]
#[command(
    version,
    about,
    long_about = "This tool converts a JSON5 file to a standard JSON file. It can either save the output to a specified file or print it to standard output. You can also choose between pretty-printed and minified output."
)]
struct Args {
    /// Path to the input JSON5 file.
    #[arg(short, long)]
    input: PathBuf,

    /// Optional path to the output JSON file. If not provided, the output will be printed to stdout.
    #[arg(short, long)]
    output: Option<PathBuf>,

    /// Output minified JSON (without pretty-printing).
    #[arg(short, long)]
    minify: bool,
}

fn main() -> io::Result<()> {
    let args = Args::parse();

    // Read the JSON5 content from the input file.
    let json5_content = fs::read_to_string(&args.input)?;

    // Parse the JSON5 content into a serde_json::Value.
    let json_value: serde_json::Value = match serde_json5::from_str(&json5_content) {
        Ok(value) => value,
        Err(e) => {
            eprintln!("Error parsing JSON5: {}", e);
            std::process::exit(1);
        }
    };

    // Serialize the serde_json::Value to a pretty-printed or minified JSON string.
    let json_output = if args.minify {
        match serde_json::to_string(&json_value) {
            Ok(json) => json,
            Err(e) => {
                eprintln!("Error serializing to minified JSON: {}", e);
                std::process::exit(1);
            }
        }
    } else {
        match serde_json::to_string_pretty(&json_value) {
            Ok(json) => json,
            Err(e) => {
                eprintln!("Error serializing to pretty JSON: {}", e);
                std::process::exit(1);
            }
        }
    };

    // Write the JSON output to a file or stdout.
    if let Some(output_path) = args.output {
        fs::write(output_path, json_output)?;
        println!("Successfully converted and saved to output file!");
    } else {
        io::stdout().write_all(json_output.as_bytes())?;
    }

    Ok(())
}
