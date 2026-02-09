use std::io::Result;
/// # Main Entry Point for Build Script
///
/// This build script compiles the `PricingData.proto` Protobuf schema into
/// Rust code using `prost_build`. The generated Rust code will be placed
/// in the `servers/src/yahoo_logic` directory.
///
/// This is typically run automatically by Cargo during the build process
/// if specified in `build.rs`.
///
/// # Returns
/// A `Result<()>` indicating success or an `std::io::Error` if Protobuf compilation fails.
fn main() -> Result<()> {
    /// Configures `prost_build` to generate Rust code from the Protobuf schema.
    prost_build::Config::new()
        /// Specifies the output directory for the generated Rust code.
        .out_dir("servers/src/yahoo_logic")
        /// Compiles the `PricingData.proto` file.
        .compile_protos(&["servers/proto/PricingData.proto"], &["servers/proto/"])?;
    Ok(())
}
