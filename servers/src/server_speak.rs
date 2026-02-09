//! # Text-to-Speech (TTS) Server
//!
//! This module implements a TCP server that acts as an interface to the
//! `espeak-ng` text-to-speech engine. It listens for incoming JSON messages
//! over TCP, extracts textual content and speech parameters, and then
//! vocalizes the message using `espeak-ng`.
//!
//! ## Functionality:
//! - **TCP Listener**: Binds to a configurable TCP port and accepts client connections.
//! - **JSON Message Processing**: Parses incoming data as JSON, expecting an `Espeak`
//!   structure which includes the message text and voice options.
//! - **`espeak-ng` Integration**: Executes the `espeak-ng` command-line tool to
//!   convert the received text into speech, applying specified voice parameters.
//! - **Message Repetition**: Supports repeating a message multiple times with
//!   a configurable interval.
//! - **Logging & Error Handling**: Utilizes `tracing` for structured logging
//!   of server operations, messages, and `espeak-ng` execution results.
//! - **Graceful Shutdown**: Integrates `tokio-graceful` for clean shutdown
//!   on signal reception.
//! - **Dynamic Configuration**: Loads process and runtime configuration from
//!   environment variables and `lib_common` utilities.
//!
//! This server is intended for applications requiring real-time audio alerts
//! or text-to-speech notifications, suchs as market event alerts or system
//! status announcements.

#![allow(dead_code)]
#![allow(unused_variables)]
#![allow(unused_imports)]
#![allow(unreachable_code)]

use std::collections::BTreeMap;
use std::env;
use std::io;
use std::time::Duration;

use anyhow::Result;

use static_init::dynamic;

use tokio::net::tcp;
use tracing::{debug, error, info, trace, warn};
use tracing_appender::{non_blocking, rolling};
use tracing_subscriber::{EnvFilter, fmt, layer::SubscriberExt, prelude::*};

use serde::{Deserialize, Serialize};
use serde_json::Value;
// use serde_json::Result;

use tokio::io::AsyncReadExt;
use tokio::net::TcpListener;
use tokio::process::Command;
use tokio::sync::OnceCell;

use tokio::sync::mpsc;
use tokio::sync::mpsc::{Receiver, Sender};
use tokio::{task, time};

use tokio_cron_scheduler::{Job, JobScheduler, JobSchedulerError};
use tokio_graceful::Shutdown;

use lib_common::config_sys::{RuntimeConfig, RuntimeConfigError, get_runtime_config};
use lib_common::loggers::logrecord::{Message, Voice};
use lib_common::utils::misc::sys_info::{ProcessInfo, ProcessInfoError, get_process_info};

// load .env files before anything else
/// Initializes environment variables by loading `.env` files.
///
/// It first attempts to load a generic `.env` file, and then
/// an OS-specific `.env.windows` or `.env.linux` file.
#[dynamic]
static DOTENV_INIT: () = {
    // Determine the operating system
    let dotenv_os: &str = if cfg!(target_os = "windows") {
        ".env.windows"
    } else {
        ".env.linux"
    };

    // Set up environment variables
    dotenvy::dotenv().ok();
    // Load the platform .env file
    dotenvy::from_filename(dotenv_os).ok();
    // for (key, value) in env::vars() {
    //     println!("{key}: {value}");
    // }
};

/// Statically initialized `ProcessInfo` instance, providing details about the current process.
#[dynamic]
pub static PROCESSINFO: Result<ProcessInfo, ProcessInfoError> = get_process_info();

/// Statically initialized `RuntimeConfig` instance, providing application runtime configuration.
#[dynamic]
pub static RUNTIMECONFIG: Result<RuntimeConfig, RuntimeConfigError> = get_runtime_config();

/// # Espeak Message Structure
///
/// Represents a message intended for text-to-speech conversion via `espeak-ng`,
/// including the message content and voice options.
#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Espeak {
    /// The message to be spoken.
    pub message: Message,
    /// Voice and playback options for `espeak-ng`.
    pub options: Voice,
}

/// # Setup Logging
///
/// Configures the `tracing` subscriber to handle application logging.
///
/// Logging is set up to:
/// - Read the log level from the `RUST_LOG` environment variable (defaults to "info").
/// - Write logs to both standard output (console) and a daily rotating file.
/// - Console logs are human-readable with ANSI color support.
/// - File logs are JSON-formatted for structured analysis.
///
/// Log files are stored in a directory specified by `LOG_DIR` environment variable
/// (defaults to "logs") and named based on the process basename.
///
/// # Returns
/// An `io::Result<()>` indicating success or failure of logging setup.
fn setup_logging() -> io::Result<()> {
    /// Get log level from environment variable or use default
    let log_level: String = env::var("RUST_LOG").unwrap_or_else(|_| "info".to_string());

    /// Get log directory from environment variable or use default
    let log_dir: String = env::var("LOG_DIR").unwrap_or_else(|_| "logs".to_string());

    /// Create log directory if it doesn't exist
    std::fs::create_dir_all(&log_dir)?;

    /// Configure file appender for rotating log files daily.
    /// The log file name is based on the process's executable name.
    let process_basename: &String = match &*PROCESSINFO {
        Ok(process_info) => &process_info.process_basename,
        Err(e) => {
            eprintln!("Failed to retrieve process info: {}", e);
            std::process::exit(1);
        }
    };
    let file_appender = rolling::daily(&log_dir, process_basename.as_str());
    let (non_blocking_appender, _guard) = non_blocking(file_appender);

    /// Stores the `WorkerGuard` in a `static mut` to keep it alive for the duration of the program.
    /// This is crucial for `non_blocking` appenders to ensure all buffered logs are flushed.
    static mut GUARD: Option<tracing_appender::non_blocking::WorkerGuard> = None;
    unsafe {
        GUARD = Some(_guard);
    }

    /// Create console layer for stdout, enabling target information and ANSI colors.
    let console_layer = fmt::layer().with_target(true).with_ansi(true);

    /// Create JSON-formatted file layer for structured logging to the daily rotating file.
    let file_layer = fmt::layer()
        .with_ansi(false)
        .with_writer(non_blocking_appender)
        .json();

    /// Create environment filter from log level, allowing dynamic control of verbosity.
    let env_filter: EnvFilter = EnvFilter::try_from_default_env()
        .or_else(|_| EnvFilter::try_new(&log_level))
        .unwrap();

    /// Combine all layers (console, file) and initialize the global tracing subscriber.
    tracing_subscriber::registry()
        .with(env_filter)
        .with(console_layer)
        .with(file_layer)
        .init();

    info!("Logging initialized with level: {}", log_level);
    Ok(())
}

#[tokio::main]
/// # Main Entry Point
///
/// Initializes and runs the text-to-speech server.
///
/// This function performs the following steps:
/// 1.  Sets up structured logging using `tracing`.
/// 2.  Logs process and runtime configuration information.
/// 3.  Initializes `tokio-graceful` for graceful shutdown handling.
/// 4.  Spawns the `tokio_main` function as the primary server task.
/// 5.  Waits for a shutdown signal and handles graceful or forceful termination.
///
/// # Returns
/// An `anyhow::Result<()>` indicating success or failure of the server operation.
async fn main() -> Result<()> {
    /// Set up logging for the application. If logging initialization fails, the process exits.
    if let Err(e) = setup_logging() {
        eprintln!("Failed to initialize logging: {}", e);
        std::process::exit(1);
    }

    /// Logs the process information retrieved during static initialization.
    match &*PROCESSINFO {
        Ok(process_info) => {
            info!("{}", process_info);
        }
        Err(e) => {
            error!("Failed to retrieve process info: {}", e);
            std::process::exit(1);
        }
    }

    /// Logs the runtime configuration information retrieved during static initialization.
    match &*RUNTIMECONFIG {
        Ok(runtime_config) => {
            info!("{}", runtime_config);
        }
        Err(e) => {
            error!("Failed to retrieve runtime config: {}", e);
            std::process::exit(1);
        }
    }

    /// Initializes the `tokio-graceful` shutdown manager.
    let shutdown: Shutdown = tokio_graceful::Shutdown::default();

    /// Spawns the main asynchronous server logic (`tokio_main`) and attaches it to the shutdown guard.
    shutdown.spawn_task_fn(tokio_main);

    /// Waits for a shutdown signal (e.g., Ctrl+C, SIGTERM) and attempts a graceful shutdown
    /// within a 10-second limit.
    match shutdown.shutdown_with_limit(Duration::from_secs(10)).await {
        Ok(elapsed) => {
            info!(
                "shutdown: gracefully {}s after shutdown signal received",
                elapsed.as_secs_f64()
            );
        }
        Err(e) => {
            info!("shutdown: forcefully due to timeout: {}", e);
        }
    }

    info!("Bye!");

    Ok(())
}

/// # Asynchronous Main Logic
///
/// This function contains the core asynchronous logic of the text-to-speech server.
/// It sets up the TCP listener, a message passing channel for `espeak-ng` execution,
/// and handles incoming client connections while respecting graceful shutdown signals.
///
/// # Arguments
/// * `shutdown_guard` - A `tokio_graceful::ShutdownGuard` to monitor for shutdown signals.
async fn tokio_main(shutdown_guard: tokio_graceful::ShutdownGuard) {
    /// Retrieves the process basename from `PROCESSINFO` for logging and identification.
    let process_basename: String = PROCESSINFO.as_ref().unwrap().process_basename.clone();
    /// Retrieves runtime configuration options, including the TCP port.
    let process_options: BTreeMap<String, String> =
        RUNTIMECONFIG.as_ref().unwrap().config_options.clone();
    /// Extracts the TCP port from runtime configuration.
    let tcp_port: String = process_options.get("Tcp:Port").unwrap().clone();

    // if let Err(e) = CRON.start().await {
    //     eprintln!("Error on scheduler {:?}", e);
    // }
    // tokio::spawn(CRON.start());

    /// Binds the TCP listener to the configured port.
    let tcplistener: String = format!("0.0.0.0:{}", tcp_port);
    let listener: TcpListener = TcpListener::bind(tcplistener.clone()).await.unwrap();
    info!(
        "{:?} listening on {}",
        process_basename,
        listener.local_addr().unwrap()
    );

    /// Creates an MPSC channel to send `Espeak` messages to the `espeak_exec` task.
    let (tx, rx) = mpsc::channel(32);
    /// Spawns the `espeak_exec` task to handle text-to-speech command execution.
    task::spawn(espeak_exec(rx));

    /// Main loop for accepting incoming TCP connections and handling shutdown signals.
    loop {
        let shutdown_guard: tokio_graceful::ShutdownGuard = shutdown_guard.clone();
        tokio::select! {
            /// Monitors for a graceful shutdown signal. If received, the loop breaks.
            _ = shutdown_guard.cancelled() => {
                info!("Signal received: initiate graceful shutdown");
                break;
            }
            /// Attempts to accept a new incoming TCP connection.
            result = listener.accept() => {
                match result {
                    Ok((socket, _)) => {
                        let tx = tx.clone();
                        /// Spawns a new task to handle each client connection.
                        task::spawn(async move {
                            // NOTE, make sure to pass a clone of the shutdown guard to this function
                            // or any of its children in case you wish to be able to cancel a long running process should the
                            // shutdown signal be received and you know that your task might not finish on time.
                            // This allows you to at least leave it behind in a consistent state such that another
                            // process can pick up where you left that task.
                            if let Err(e) = handle_client(socket, tx).await {
                                error!("Failed to handle client: {}", e);
                            }
                            drop(shutdown_guard);
                        });
                    }
                    Err(e) => {
                        warn!("accept error: {:?}", e);
                    }
                }
            }
        }
    }
}

/// # Espeak Execution Task
///
/// This asynchronous task continuously listens for `Espeak` messages from an MPSC channel
/// and executes the `espeak-ng` command-line tool to vocalize them.
///
/// # Arguments
/// * `rx` - The receiving end of an MPSC channel for `Espeak` messages.
async fn espeak_exec(mut rx: Receiver<Espeak>) {
    /// Loops indefinitely, processing `Espeak` messages as they arrive in the channel.
    while let Some(say_this) = rx.recv().await {
        /// Serializes the `Espeak` message to JSON for logging purposes.
        let say_this_json: Value = serde_json::to_value(say_this.clone()).unwrap();
        /// Initializes a new `Command` for `espeak-ng`.
        let mut command: Command = tokio::process::Command::new("espeak-ng");

        /// Adds voice options from the `Espeak` message as arguments to the `espeak-ng` command.
        if let Some(options) = say_this.options.voptions.as_array() {
            for option in options {
                if let Some(option_str) = option.as_str() {
                    command.arg(option_str);
                }
            }
        }
        /// Adds the message text as an argument to the `espeak-ng` command.
        command.arg(say_this.message.text);
        trace!("Executing command: {:?}", command);

        /// Executes the `espeak-ng` command and logs its output or errors.
        match command.output().await {
            Ok(output) => {
                if !output.status.success() {
                    error!(
                        "Error executing espeak-ng: {}",
                        String::from_utf8_lossy(&output.stderr)
                    );
                } else {
                    debug!("Successfully executed: {:?}", say_this_json);
                }
            }
            Err(e) => warn!("Failed to execute command: {}", e),
        }
    }
}

/// # Handle Client Connection
///
/// This asynchronous function handles a single incoming TCP client connection.
/// It reads data from the socket, attempts to parse it as a JSON `Espeak` message,
/// and then sends the message to the `espeak_exec` task for vocalization.
///
/// If the `Espeak` message includes a `repeat` option greater than 0, the message
/// will be sent multiple times with a specified `interval`.
///
/// # Arguments
/// * `client` - The `tokio::net::TcpStream` representing the client connection.
/// * `tx` - The sending end of an MPSC channel to send `Espeak` messages to the executor.
///
/// # Returns
/// A `Result<()>` indicating whether the client handling was successful.
async fn handle_client(mut client: tokio::net::TcpStream, tx: Sender<Espeak>) -> Result<()> {
    /// Buffer to read incoming data from the TCP socket.
    let mut buffer = [0; 4096];

    /// Loop to continuously read from the client socket.
    loop {
        /// Reads data from the socket. Handles connection closure and read errors.
        let n = match client.read(&mut buffer).await {
            Ok(n) if n == 0 => break Ok(()), // connection closed
            Ok(n) => n,
            Err(e) => {
                warn!("Failed to read from socket; err = {:?}", e);
                break Ok(());
            }
        };

        /// Converts the received bytes into a UTF-8 string. If not valid UTF-8, it skips this message.
        let data: &str = match std::str::from_utf8(&buffer[..n]) {
            Ok(data) => data,
            Err(_) => continue,
        };

        trace!("Raw data: {}", data);

        /// Attempts to deserialize the JSON string into an `Espeak` struct.
        let json_data: std::result::Result<Espeak, serde_json::Error> =
            serde_json::from_str::<Espeak>(data);
        let json_data: Espeak = match json_data {
            Ok(json_data) => json_data,
            Err(e) => {
                warn!("Failed to parse JSON data: {}", e);
                continue;
            }
        };

        debug!("Got valid espeak message: {:?}", json_data.clone());

        // if json_data.message.text.is_empty() {
        //     eprintln!("Empty message, skipping");
        //     continue;
        // }
        // if !json_data.voice.cron.is_empty() {
        //     eprintln!("Cron job detected: {:?}", json_data.voice.cron);
        //     let cronjob: Job = Job::new_async(json_data.voice.cron, |uuid, mut l| {
        //         println!("New cron job: {:?}", uuid);
        //         Box::pin(async move {
        //             println!("I run async every 7 seconds");

        //             // Query the next execution time for this job
        //             let next_tick = l.next_tick_for_job(uuid).await;
        //             match next_tick {
        //                 Ok(Some(ts)) => println!("Next time for 7s job is {:?}", ts),
        //                 _ => println!("Could not get next tick for 7s job"),
        //             }
        //         })
        //     }).unwrap();
        //     let sched = CRON.add(cronjob).await;
        //     match sched {
        //         Ok(_) => eprintln!("Cron job added successfully"),
        //         Err(e) => eprintln!("Error adding cron job: {:?}", e),
        //     }
        //     tokio::spawn(CRON.start());
        //     eprintln!("");
        //     continue;
        // };
        /// Handles message repetition based on `json_data.options.repeat` and `interval`.
        if json_data.options.repeat > 0 {
            for _ in 0..json_data.options.repeat {
                tx.send(json_data.clone()).await.unwrap();
                time::sleep(Duration::from_secs(
                    json_data.options.interval.try_into().unwrap(),
                ))
                .await;
            }
            continue;
        }
    }
}
