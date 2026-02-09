use anyhow::{Context, Result};
use log::{error, info};
use rodio::{Decoder, OutputStreamBuilder, Sink};
use std::env;
use std::fs::File;
use std::io::BufReader;
use std::time::{Duration, Instant};
use tokio::net::TcpStream;

/// # Setup Logging
///
/// Configures the `fern` logger to output to both standard error and a file.
///
/// Log messages are formatted with a timestamp, target, level, and the message itself.
/// The log level is set to `Info` by default.
///
/// # Returns
/// A `Result<()>` indicating success or failure of the logging setup.
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

/// # Check Connection
///
/// Asynchronously attempts to establish a TCP connection to a well-known
/// DNS server (Cloudflare: 1.1.1.1 on port 53) to determine internet connectivity.
///
/// # Returns
/// `true` if a connection can be established within a 3-second timeout, `false` otherwise.
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

/// # Play Disconnect Sound
///
/// Loads and plays a "Disconnected.wav" sound file. This function is blocking
/// and should be spawned in a separate thread (e.g., using `tokio::task::spawn_blocking`)
/// to avoid blocking the main asynchronous runtime.
///
/// If the sound file is not found or cannot be decoded, an error is logged.
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
/// # Main Entry Point
///
/// This is the main function for the network monitoring utility.
/// It continuously checks internet connectivity and plays an audio alert
/// when a disconnection is detected.
///
/// ## Workflow:
/// 1.  Sets up logging for the application.
/// 2.  Parses the monitoring interval from command-line arguments (defaults to 10 seconds).
/// 3.  Enters an infinite loop to periodically check network status:
///     -   Calls `check_connection()` to determine current connectivity.
///     -   If connection is lost, it plays a disconnect sound and records the start time of the outage.
///     -   If connection is restored after an outage, it logs the duration of the disconnection.
///     -   Plays the disconnect sound repeatedly if still disconnected.
///     -   Pauses for the specified interval before the next check.
///
/// # Returns
/// A `Result<()>` indicating the overall success or failure of the monitoring process.
async fn main() -> Result<()> {
    /// Initializes logging for the application.
    setup_logging().context("Failed to initialize logging")?;

    /// Collects command-line arguments.
    let args: Vec<String> = env::args().collect();

    /// Parses the monitoring interval from arguments or defaults to 10 seconds.
    let interval_secs: u64 = args.get(1).and_then(|s| s.parse().ok()).unwrap_or(10);

    info!("Monitoring internet every {} seconds...", interval_secs);

    /// Flag to track the current connection status.
    let mut is_currently_connected = true;
    /// Optional timestamp to record when a disconnection event began.
    let mut disconnect_start_time: Option<Instant> = None;

    /// Main monitoring loop.
    loop {
        /// Checks the current internet connectivity.
        let connected = check_connection().await;

        if !connected {
            /// Handles initial disconnection event.
            if is_currently_connected {
                // Just lost connection
                info!("Disconnected! Playing alert...");
                is_currently_connected = false;
                disconnect_start_time = Some(Instant::now());
            }
            // Play sound every interval if still disconnected
            /// Plays a disconnect sound. This blocking operation is spawned to avoid freezing the async runtime.
            tokio::task::spawn_blocking(play_disconnect_sound)
                .await
                .ok();
        } else {
            /// Handles connection restoration event.
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

        /// Pauses for the specified interval before the next connectivity check.
        tokio::time::sleep(Duration::from_secs(interval_secs)).await;
    }
}
