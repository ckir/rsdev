//! # `rs_encrypt`: AES-256 CBC File Encryptor
//!
//! This command-line utility provides a simple yet robust tool for encrypting
//! files using AES-256 in Cipher Block Chaining (CBC) mode. It's designed for
//! encrypting sensitive configuration files, secrets, or any other data that
//! needs to be protected at rest.
//!
//! ## Key Features:
//! - **AES-256 Encryption**: Employs the strong Advanced Encryption Standard
//!   with a 256-bit key size.
//! - **CBC Mode**: Uses Cipher Block Chaining for enhanced security, ensuring
//!   that identical plaintext blocks encrypt to different ciphertext blocks.
//! - **PKCS7 Padding**: Automatically handles plaintext padding to meet block
//!   size requirements.
//! - **Base64 Encoding**: Encrypted output (Initialization Vector and ciphertext)
//!   is Base64 encoded for safe storage and transmission in text-based formats.
//! - **Command-Line Interface**: Easy to use from the terminal with `clap` for
//!   argument parsing.
//!
//! ## Security Considerations:
//! - **Key Management**: The security of the encrypted data is entirely dependent
//!   on the secrecy and strength of the provided 256-bit hexadecimal key. **Never**
//!   hardcode keys in source code or expose them in insecure environments.
//!   Proper key management practices (e.g., environment variables, secure key stores)
//!   are crucial.
//! - **Random IV**: A cryptographically strong random Initialization Vector (IV)
//!   is generated for each encryption operation. This is essential for the security
//!   of CBC mode, preventing identical plaintexts from yielding identical ciphertexts.
//! - **No Authentication**: This tool performs encryption but does **not** include
//!   any message authentication code (MAC) or authenticated encryption. This means
//!   it cannot detect if the ciphertext has been tampered with. For integrity
//!   and authenticity, a separate mechanism (e.g., HMAC) should be applied to the
//!   ciphertext.
//!
//! ## Usage
//!
//! ```bash
//! rs_encrypt --input <INPUT_FILE> --output <OUTPUT_FILE> --key <HEX_256_BIT_KEY>
//! ```

#![doc(html_logo_url = "https://example.com/logo.png")] // Placeholder for consistency
#![forbid(unsafe_code)]
#![warn(missing_docs, rust_2018_idioms, unused_qualifications)]

use aes::Aes256;
use base64::{Engine as _, engine::general_purpose};
use block_padding::Pkcs7;
use cbc::Encryptor;
use cbc::cipher::{BlockEncryptMut, KeyIvInit};
use rand::Rng; // Use Rng trait for fill_bytes
use clap::Parser; // Import clap Parser trait
use std::fs;
use std::io::{Read, Write};
use std::path::PathBuf;

/// # Command Line Arguments
///
/// Defines the command-line arguments for the `rs_encrypt` tool,
/// using `clap` for parsing and help generation.
/// # Command Line Arguments
///
/// Defines the command-line arguments for the `rs_encrypt` tool,
/// using `clap` for parsing and help generation.
#[derive(Parser, Debug)]
#[command(
    name = "rs_encrypt",
    version,
    about = "Encrypts a file using AES-256 CBC mode with a provided 256-bit hex key.",
    long_about = "This tool takes an input file, encrypts its contents using AES-256 in CBC mode, and writes the Base64-encoded Initialization Vector (IV) and ciphertext to an output file. A 256-bit (64-character hexadecimal) key must be provided."
)]
struct Args {
    /// Path to the input file to be encrypted.
    #[arg(short, long)]
    input: PathBuf,

    /// Path to the output file where the Base64-encoded IV and ciphertext will be written.
    #[arg(short, long)]
    output: PathBuf,

    /// The 256-bit encryption key, provided as a 64-character hexadecimal string.
    /// Example: 0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef
    #[arg(short, long)]
    key: String,
}

/// # Main Entry Point
///
/// This is the `main` function for the `rs_encrypt` command-line tool.
/// It orchestrates the file reading, key derivation, IV generation, AES-256 CBC
/// encryption, Base64 encoding, and writing the result to an output file.
///
/// ## Workflow:
/// 1.  **Parse Arguments**: Uses `clap` to parse command-line arguments, including
///     input file, output file, and the hexadecimal encryption key.
/// 2.  **Read Plaintext**: Reads the entire content of the input file into memory.
/// 3.  **Derive Key**: Converts the hexadecimal key string into a 32-byte (256-bit)
///     key array, validating its format and length.
/// 4.  **Generate IV**: Generates a cryptographically secure random 16-byte (128-bit)
///     Initialization Vector (IV).
/// 5.  **Encrypt**: Performs AES-256 CBC encryption using the derived key, generated
///     IV, and PKCS7 padding.
/// 6.  **Base64 Encode**: Encodes both the IV and the resulting ciphertext into
///     Base64 strings for text-safe storage.
/// 7.  **Write Output**: Writes the Base64-encoded IV and ciphertext to the specified
///     output file, each on a new line.
///
/// Error handling is robust, providing clear messages and exiting on critical failures
/// like invalid keys or file I/O errors.
///
/// # Returns
/// A `Result<()>` indicating the overall success or failure of the encryption process.
/// # Main Entry Point
///
/// This is the `main` function for the `rs_encrypt` command-line tool.
/// It orchestrates the file reading, key derivation, IV generation, AES-256 CBC
/// encryption, Base64 encoding, and writing the result to an output file.
///
/// ## Workflow:
/// 1.  **Parse Arguments**: Uses `clap` to parse command-line arguments, including
///     input file, output file, and the hexadecimal encryption key.
/// 2.  **Read Plaintext**: Reads the entire content of the input file into memory.
/// 3.  **Derive Key**: Converts the hexadecimal key string into a 32-byte (256-bit)
///     key array, validating its format and length.
/// 4.  **Generate IV**: Generates a cryptographically secure random 16-byte (128-bit)
///     Initialization Vector (IV).
/// 5.  **Encrypt**: Performs AES-256 CBC encryption using the derived key, generated
///     IV, and PKCS7 padding.
/// 6.  **Base64 Encode**: Encodes both the IV and the resulting ciphertext into
///     Base64 strings for text-safe storage.
/// 7.  **Write Output**: Writes the Base64-encoded IV and ciphertext to the specified
///     output file, each on a new line.
///
/// Error handling is robust, providing clear messages and exiting on critical failures
/// like invalid keys or file I/O errors.
///
/// # Returns
/// A `Result<()>` indicating the overall success or failure of the encryption process.
fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// Parses command-line arguments into an `Args` struct.
    let args = Args::parse();

    println!("Input file: {}", args.input.display());
    println!("Output file for encrypted data: {}", args.output.display());
    println!("Using key (first 8 chars): {}...", &args.key[0..8]);

    // --- 1. Read Plaintext from Input File ---
    println!("Reading file: {}", args.input.display());
    let mut file = fs::File::open(&args.input)?;
    let mut plaintext = Vec::new();
    file.read_to_end(&mut plaintext)?;

    // --- 2. Derive Key from Hex String ---
    /// Decodes the hexadecimal key string into a byte array and validates its length.
    let key = hex::decode(&args.key).map_err(|_| "Invalid key hex string")?;
    if key.len() != 32 {
        return Err("Key must be 32 bytes (256 bits) for AES-256".into());
    }
    /// Converts the `Vec<u8>` key to a fixed-size `&[u8; 32]` array required by `aes` crate.
    let key_array: &[u8; 32] = key
        .as_slice()
        .try_into()
        .map_err(|_| "Failed to convert key to array (internal error)")?;

    // --- 3. Generate a Random 16-byte IV ---
    // A unique and random IV is crucial for the security of CBC mode.
    /// Generates a cryptographically secure random 16-byte Initialization Vector (IV).
    let mut iv_bytes = [0u8; 16];
    rand::thread_rng().fill_bytes(&mut iv_bytes); // Fill with cryptographically secure random bytes.

    // --- 4. Perform AES-256 CBC Encryption ---
    // `Encryptor::new` initializes the cipher with the key and IV.
    // `encrypt_padded_vec_mut` handles PKCS7 padding and encryption.
    /// Initializes the AES-256 CBC encryptor with the key and IV.
    let encryptor = Encryptor::<Aes256>::new(key_array.into(), (&iv_bytes).into());
    /// Encrypts the plaintext using PKCS7 padding.
    let ciphertext = encryptor.encrypt_padded_vec_mut::<Pkcs7>(&plaintext);

    // --- 5. Encode IV and Ciphertext to Base64 ---
    // Base64 encoding makes the binary IV and ciphertext safe for storage
    // and transmission in text-based formats (e.g., config files).
    /// Base64-encodes the generated Initialization Vector.
    let iv_base64 = general_purpose::STANDARD.encode(&iv_bytes);
    /// Base64-encodes the resulting ciphertext.
    let ciphertext_base64 = general_purpose::STANDARD.encode(&ciphertext);

    // --- 6. Write the Encrypted Data to Output File ---
    /// Creates or opens the output file for writing.
    let mut output_file = fs::File::create(&args.output)?;
    /// Writes the Base64-encoded IV and ciphertext to the output file, each on a new line.
    writeln!(output_file, "{}", iv_base64)?;
    writeln!(output_file, "{}", ciphertext_base64)?;

    println!("Encryption complete. Output written to '{}'.", args.output.display());

    Ok(())
}
