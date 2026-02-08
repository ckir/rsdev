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
#[clap(author, version, about, long_about = None)]
struct Args {
    /// Report interval in minutes
    #[clap(short, long, default_value_t = 1)]
    report_interval_minutes: u64,
}

#[derive(Debug, Deserialize, Serialize)]
struct PricingMessage {
    #[serde(rename = "type")]
    msg_type: String,
    message: Option<SymbolId>,
}

#[derive(Debug, Deserialize, Serialize)]
struct SymbolId {
    id: String,
}

struct Stats {
    global_timestamps: VecDeque<chrono::DateTime<Utc>>,
    symbol_timestamps: HashMap<String, VecDeque<chrono::DateTime<Utc>>>,
}

#[tokio::main]
async fn main() {
    let args = Args::parse();

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

    let stats = Arc::new(Mutex::new(Stats {
        global_timestamps: VecDeque::new(),
        symbol_timestamps: HashMap::new(),
    }));

    // Clone for the reporter task
    let stats_reporter = Arc::clone(&stats);
    let report_interval_seconds = args.report_interval_minutes * 60;
    tokio::spawn(async move {
        loop {
            sleep(std::time::Duration::from_secs(report_interval_seconds)).await;
            let now = Utc::now();
            let one_minute_ago = now - Duration::minutes(1);

            let mut data = stats_reporter.lock().unwrap();

            // Clean global
            while data.global_timestamps.front().map_or(false, |&t| t < one_minute_ago) {
                data.global_timestamps.pop_front();
            }
            let global_rate = data.global_timestamps.len();

            // Clean per symbol and collect rates
            let mut rates: Vec<(String, usize)> = Vec::new();
            for (symbol, dq) in data.symbol_timestamps.iter_mut() {
                while dq.front().map_or(false, |&t| t < one_minute_ago) {
                    dq.pop_front();
                }
                if !dq.is_empty() {
                    rates.push((symbol.clone(), dq.len()));
                }
            }

            // Sort DESC by msg/min
            rates.sort_by(|a, b| b.1.cmp(&a.1));

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
    let (ws_stream, _) = connect_async(URL).await.expect("Failed to connect");
    let (mut write, mut read) = ws_stream.split();

    // Subscribe
    let sub_msg = json!({ "subscribe": symbols }).to_string();
    write.send(Message::Text(sub_msg.into())).await.expect("Failed to send sub");
    println!("Subscribed. Press Ctrl+C to stop.");

    // Handle incoming messages
    while let Some(Ok(msg)) = read.next().await {
        if let Message::Text(text) = msg {
            if let Ok(parsed) = serde_json::from_str::<PricingMessage>(&text) {
                if parsed.msg_type == "pricing" {
                    if let Some(sym_info) = parsed.message {
                        let now = Utc::now();
                        let mut data = stats.lock().unwrap();
                        data.global_timestamps.push_back(now);
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
