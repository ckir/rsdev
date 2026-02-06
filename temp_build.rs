use std::io::Result;
fn main() -> Result<()> {
    prost_build::Config::new()
        .out_dir("servers/src/yahoo_logic")
        .compile_protos(&["servers/proto/PricingData.proto"], &["servers/proto/"])?;
    Ok(())
}
