use clap::Parser;
use std::fs;
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(author, version, about = "Converts JSON5 to Clean YAML with Block Scalars")]
struct Args {
    /// Path to the input .json5 file
    #[arg(short, long)]
    input: PathBuf,

    /// Path to the output .yaml file
    #[arg(short, long)]
    output: PathBuf,
}

fn main() {
    let args = Args::parse();

    // 1. Read the JSON5
    let input_str = fs::read_to_string(&args.input).unwrap_or_else(|err| {
        eprintln!("❌ Failed to read input: {}", err);
        std::process::exit(1);
    });

    // 2. Parse JSON5 into a generic Value
    // This handles unquoted keys and trailing commas automatically.
    let data: serde_json::Value = json5::from_str(&input_str).unwrap_or_else(|err| {
        eprintln!("❌ JSON5 Parse Error: {}", err);
        std::process::exit(1);
    });

    // 3. Convert to YAML
    // serde_yml detects strings with \n and uses '|' (Literal Block Scalar) automatically.
    let yaml_output = serde_yml::to_string(&data).unwrap_or_else(|err| {
        eprintln!("❌ YAML Serialization Error: {}", err);
        std::process::exit(1);
    });

    // 4. Verify the generated YAML
    // We try to parse the output we just created to ensure it's valid YAML.
    match serde_yml::from_str::<serde_json::Value>(&yaml_output) {
        Ok(_) => println!("✅ Verification Successful: Output is valid YAML."),
        Err(e) => {
            eprintln!("⚠️ Verification Failed: The generated YAML is invalid! Error: {}", e);
            std::process::exit(1);
        }
    }

    // 5. Write to file
    fs::write(&args.output, &yaml_output).expect("Failed to write file");
    println!("🚀 Conversion complete! Saved to {:?}", args.output);
}