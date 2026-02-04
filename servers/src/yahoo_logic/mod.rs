pub mod config;
pub mod logger;
pub mod model;
pub mod state;
pub mod upstream;
pub mod downstream;

// Include the prost-generated rust file
pub mod yahoo_finance {
    include!(concat!(env!("OUT_DIR"), "/yahoo_finance.rs"));
}
