pub mod yahoo_wss;
pub mod cnn_polling;

// Re-export so they are available via lib_common::ingestors::...
pub use yahoo_wss::{YahooWssIngestor, YahooConfig};
pub use cnn_polling::CnnPollingPlugin;
