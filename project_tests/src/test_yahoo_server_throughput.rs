use chrono::{Duration, Utc};
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};
use tokio::time::sleep;
use tokio_tungstenite::{connect_async, tungstenite::protocol::Message};
use clap::Parser; // Added for clap

const URL: &str = "wss://ckir.ddns.net:9002/ws";

#[derive(Parser, Debug)]
#[clap(author, version, about = "Monitors Yahoo server WebSocket throughput", long_about = None)]
struct Args {
    /// Reporting interval in minutes. A summary of messages per minute will be printed at this interval.
    #[clap(short, long, default_value_t = 1)]
    report_interval_minutes: u64,
}

#[derive(Debug, Deserialize, Serialize)]
/// # Pricing Message
///
/// Represents the structure of a pricing message received from the WebSocket.
struct PricingMessage {
    /// The type of the message, typically "pricing".
    #[serde(rename = "type")]
    msg_type: String,
    /// Optional symbol identification information associated with the message.
    message: Option<SymbolId>,
}

#[derive(Debug, Deserialize, Serialize)]
/// # Symbol ID
///
/// Represents the identifier for a financial symbol.
struct SymbolId {
    /// The ticker symbol (e.g., "AAPL").
    id: String,
}

/// # Statistics Container
///
/// Holds the collected message timestamps for calculating throughput rates.
struct Stats {
    /// A deque storing timestamps of all received messages globally.
    global_timestamps: VecDeque<chrono::DateTime<Utc>>,
    /// A hash map storing deques of timestamps for each individual symbol.
    symbol_timestamps: HashMap<String, VecDeque<chrono::DateTime<Utc>>>,
}

/// # Main Entry Point
///
/// This function is the main entry point for the Yahoo server throughput test.
/// It connects to a WebSocket server, subscribes to a list of financial symbols,
/// and continuously monitors the message throughput, reporting it periodically.
///
/// ## Workflow:
/// 1.  Parses command-line arguments for the reporting interval.
/// 2.  Initializes shared statistics structures (`Stats`).
/// 3.  Spawns a background task to periodically report throughput summaries.
///     -   Calculates global and per-symbol messages per minute.
///     -   Cleans up old timestamps from the deques.
///     -   Prints a formatted summary to the console.
/// 4.  Connects to the WebSocket server (`URL`).
/// 5.  Subscribes to a predefined list of financial symbols.
/// 6.  Enters a loop to receive and process incoming WebSocket messages:
///     -   Parses messages as `PricingMessage`.
///     -   Records timestamps for global and per-symbol throughput calculation.
///
/// Press Ctrl+C to stop the monitoring.
#[tokio::main]
async fn main() {
    /// Parses command-line arguments for the reporting interval.
    let args = Args::parse();

    /// Defines a list of financial symbols to subscribe to for testing.
    let symbols = vec![
        "AAPL", "ABNB", "ADBE", "ADI", "ADP", "ADSK", "AEP", "ALNY", "AMAT", "AMD",
        "AMGN", "AMZN", "APP", "ARM", "ASML", "AVGO", "AXON", "BKNG", "BKR", "CCEP",
        "CDNS", "CEG", "CHTR", "CMCSA", "COST", "CPRT", "CRWD", "CSCO", "CSGP", "CSX",
        "CTAS", "CTSH", "DASH", "DDOG", "DXCM", "EA", "EXC", "FANG", "FAST", "FER",
        "FTNT", "GEHC", "GILD", "GOOG", "GOOGL", "HON", "IDXX", "INSM", "INTC", "INTU",
        "ISRG", "KDP", "KHC", "KLAC", "LIN", "LRCX", "MAR", "MCHP", "MDLZ", "MELI",
        "META", "MNST", "MPWR", "MRVL", "MSFT", "MSTR", "MU", "NFLX", "NVDA", "NXPI",
        "ODFL", "ORLY", "PANW", "PAYX", "PCAR", "PDD", "PEP", "PLTR", "PYPL", "QCOM",
        "REGN", "ROP", "ROST", "SBUX", "SHOP", "SNPS", "STX", "TEAM", "TMUS", "TRI",
        "TSLA", "TTWO", "TXN", "VRSK", "VRTX", "WBD", "WDAY", "WDC", "WMT", "XEL", "ZS",
    ];

    /// Initializes shared statistics tracking structures. `Arc<Mutex<...>>` enables safe concurrent access.
    let stats = Arc::new(Mutex::new(Stats {
        global_timestamps: VecDeque::new(),
        symbol_timestamps: HashMap::new(),
    }));

    /// Clones the `stats` Arc for use in the reporter task.
    let stats_reporter = Arc::clone(&stats);
    /// Calculates the report interval in seconds.
    let report_interval_seconds = args.report_interval_minutes * 60;
    /// Spawns an asynchronous task to periodically generate and print throughput reports.
    tokio::spawn(async move {
        loop {
            sleep(std::time::Duration::from_secs(report_interval_seconds)).await;
            let now = Utc::now();
            let one_minute_ago = now - Duration::minutes(1);

            let mut data = stats_reporter.lock().unwrap();

            /// Cleans up old global timestamps (older than 1 minute) from the deque.
            while data.global_timestamps.front().map_or(false, |&t| t < one_minute_ago) {
                data.global_timestamps.pop_front();
            }
            let global_rate = data.global_timestamps.len();

            /// Cleans up old per-symbol timestamps and calculates per-symbol rates.
            let mut rates: Vec<(String, usize)> = Vec::new();
            for (symbol, dq) in data.symbol_timestamps.iter_mut() {
                while dq.front().map_or(false, |&t| t < one_minute_ago) {
                    dq.pop_front();
                }
                if !dq.is_empty() {
                    rates.push((symbol.clone(), dq.len()));
                }
            }

            /// Sorts symbols by messages per minute in descending order.
            rates.sort_by(|a, b| b.1.cmp(&a.1));

            /// Formats the per-symbol report string.
            let report = rates
                .iter()
                .map(|(s, r)| format!("{}: {} msg/min", s, r))
                .collect::<Vec<_>>()
                .join(", ");

            println!("\n----- 1-Minute Summary -----");
            println!("Global rate: {} msg/min", global_rate);
            println!("Symbols: {}", if report.is_empty() { "No data" } else { &report });
            println!("----------------------------\n");
        }
    });

    // Main WebSocket Loop
    println!("Connecting to {}...", URL);
    /// Establishes a WebSocket connection to the specified URL.
    let (ws_stream, _) = connect_async(URL).await.expect("Failed to connect");
    let (mut write, mut read) = ws_stream.split();

    // Subscribe
    /// Constructs and sends a subscription message for the predefined symbols.
    let sub_msg = json!({ "subscribe": symbols }).to_string();
    write.send(Message::Text(sub_msg.into())).await.expect("Failed to send sub");
    println!("Subscribed. Press Ctrl+C to stop.");

    // Handle incoming messages
    /// Continuously reads incoming WebSocket messages and updates statistics.
    while let Some(Ok(msg)) = read.next().await {
        if let Message::Text(text) = msg {
            if let Ok(parsed) = serde_json::from_str::<PricingMessage>(&text) {
                if parsed.msg_type == "pricing" {
                    if let Some(sym_info) = parsed.message {
                        let now = Utc::now();
                        let mut data = stats.lock().unwrap();
                        /// Records the timestamp for global throughput.
                        data.global_timestamps.push_back(now);
                        /// Records the timestamp for per-symbol throughput.
                        data.symbol_timestamps
                            .entry(sym_info.id)
                            .or_insert_with(VecDeque::new)
                            .push_back(now);
                    }
                }
            }
        }
    }
}
