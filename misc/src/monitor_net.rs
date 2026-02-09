use anyhow::{Context, Result};
use log::{error, info};
use rodio::{Decoder, OutputStreamBuilder, Sink};
use std::env;
use std::fs::File;
use std::io::BufReader;
use std::time::{Duration, Instant};
use tokio::net::TcpStream;

fn setup_logging() -> Result<()> {
    fern::Dispatch::new()
        .format(|out, message, record| {
            out.finish(format_args!(
                "{}[{}][{}] {}",
                chrono::Local::now().format("[%Y-%m-%d %H:%M:%S]"),
                record.target(),
                record.level(),
                message
            ))
        })
        .level(log::LevelFilter::Info)
        .chain(std::io::stderr())
        .chain(fern::log_file("monitor_net.log")?)
        .apply()?;
    Ok(())
}

async fn check_connection() -> bool {
    // We check connection by trying to reach a reliable DNS server (Cloudflare)
    let address = "1.1.1.1:53";
    let timeout = Duration::from_secs(3);

    // Use tokio's async TcpStream and timeout
    match tokio::time::timeout(timeout, TcpStream::connect(address)).await {
        Ok(Ok(_)) => true,
        _ => false,
    }
}

fn play_disconnect_sound() {
    // Create an output stream and a sink for audio playback
    // Use the OutputStreamBuilder to open the default stream and keep it alive.
    let stream = match OutputStreamBuilder::open_default_stream() {
        Ok(s) => s,
        Err(_) => {
            error!("Could not find audio output device.");
            return;
        }
    };

    let sink = Sink::connect_new(stream.mixer());

    // Load the local wav file
    if let Ok(file) = File::open("Disconnected.wav") {
        match Decoder::new(BufReader::new(file)) {
            Ok(source) => {
                sink.append(source);
                sink.sleep_until_end();
            }
            Err(e) => error!("Error decoding audio file: {}", e),
        }
    } else {
        error!("Disconnected.wav not found! Please place it in the project root.");
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    setup_logging().context("Failed to initialize logging")?;

    let args: Vec<String> = env::args().collect();

    // Parse interval from arguments or default to 10 seconds
    let interval_secs: u64 = args.get(1).and_then(|s| s.parse().ok()).unwrap_or(10);

    info!("Monitoring internet every {} seconds...", interval_secs);

    let mut is_currently_connected = true;
    let mut disconnect_start_time: Option<Instant> = None;

    loop {
        let connected = check_connection().await;

        if !connected {
            if is_currently_connected {
                // Just lost connection
                info!("Disconnected! Playing alert...");
                is_currently_connected = false;
                disconnect_start_time = Some(Instant::now());
            }
            // Play sound every interval if still disconnected
            // Run blocking audio task in a separate thread to avoid blocking the async runtime
            tokio::task::spawn_blocking(play_disconnect_sound)
                .await
                .ok();
        } else {
            if !is_currently_connected {
                // Connection restored
                if let Some(start) = disconnect_start_time {
                    let duration = start.elapsed();
                    info!("Connection restored. Down for {:?}", duration);
                }
                is_currently_connected = true;
                disconnect_start_time = None;
            }
        }

        tokio::time::sleep(Duration::from_secs(interval_secs)).await;
    }
}
