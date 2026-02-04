fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("cargo:rerun-if-changed=proto/PricingData.proto");
    let mut config = prost_build::Config::new();
    config.type_attribute(".", "#[derive(serde::Serialize)]");
    config.compile_protos(&["proto/PricingData.proto"], &["proto/"])?;
    Ok(())
}
