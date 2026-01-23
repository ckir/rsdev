use aes::Aes256;
use base64::{Engine as _, engine::general_purpose};
use block_padding::Pkcs7; // NEW: Import Pkcs7 and Padding trait
use cbc::Encryptor;
use cbc::cipher::BlockEncryptMut; // FIX: Import BlockEncryptMut for encrypt_padded_vec_mut
use cbc::cipher::KeyIvInit; // FIX: Import KeyIvInit trait for Encryptor::new
use rand::{RngCore, rng};
use std::env;
use std::{fs::File, io::Read};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();

    if args.len() != 4 {
        eprintln!(
            "Usage: {} <input_filepath> <encrypted_output_filepath> <key_hex_256_bit>",
            args[0]
        );
        eprintln!(
            "Example: {} plaintext.txt encrypted.txt 0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
            args[0]
        );
        std::process::exit(1);
    }

    let input_filepath = &args[1];
    let encrypted_output_filepath = &args[2];
    let key_hex = &args[3];

    println!("Input file: {}", input_filepath);
    println!(
        "Output file for encrypted data: {}",
        encrypted_output_filepath
    );
    println!("Using key (first 8 chars): {}...", &key_hex[0..8]);

    println!("Reading file: {}", input_filepath);
    let mut file = File::open(input_filepath)?;
    let mut plaintext = Vec::new();
    file.read_to_end(&mut plaintext)?;

    // Derive key from hex string
    let key = hex::decode(key_hex).map_err(|_| "Invalid key hex string")?;
    if key.len() != 32 {
        return Err("Key must be 32 bytes (256 bits) for AES-256".into());
    }
    let key_array: &[u8; 32] = key
        .as_slice()
        .try_into()
        .map_err(|_| "Failed to convert key to array")?;

    // Generate a random 16-byte IV
    let mut iv_bytes = [0u8; 16];
    rng().fill_bytes(&mut iv_bytes);

    // Padding will be handled by encrypt_padded_vec_mut, so manual padding is not needed.

    // Perform CBC encryption using Encryptor
    let encryptor = Encryptor::<Aes256>::new(key_array.into(), (&iv_bytes).into());
    let ciphertext = encryptor.encrypt_padded_vec_mut::<Pkcs7>(&plaintext);

    // Encode IV and ciphertext to Base64
    let iv_base64 = general_purpose::STANDARD.encode(&iv_bytes);
    let ciphertext_base64 = general_purpose::STANDARD.encode(&ciphertext); // ciphertext now contains the ciphertext

    // Write the IV and ciphertext to the output file, separated by a newline
    use std::io::Write;
    let mut output_file = File::create(encrypted_output_filepath)?;
    writeln!(output_file, "{}", iv_base64)?;
    writeln!(output_file, "{}", ciphertext_base64)?;

    Ok(())
}
