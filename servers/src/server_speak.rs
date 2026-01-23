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
use lib_common::logrecord::{Message, Voice};
use lib_common::sys_info::{ProcessInfo, ProcessInfoError, get_process_info};

// load .env files before anything else
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

#[dynamic]
pub static PROCESSINFO: Result<ProcessInfo, ProcessInfoError> = get_process_info();

#[dynamic]
pub static RUNTIMECONFIG: Result<RuntimeConfig, RuntimeConfigError> = get_runtime_config();

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Espeak {
    pub message: Message,
    pub options: Voice,
}

fn setup_logging() -> io::Result<()> {
    // Get log level from environment variable or use default
    let log_level: String = env::var("RUST_LOG").unwrap_or_else(|_| "info".to_string());

    // Get log directory from environment variable or use default
    let log_dir: String = env::var("LOG_DIR").unwrap_or_else(|_| "logs".to_string());

    // Create log directory if it doesn't exist
    std::fs::create_dir_all(&log_dir)?;

    // Configure file appender for rotating log files daily
    let process_basename: &String = match &*PROCESSINFO {
        Ok(process_info) => &process_info.process_basename,
        Err(e) => {
            eprintln!("Failed to retrieve process info: {}", e);
            std::process::exit(1);
        }
    };
    let file_appender = rolling::daily(&log_dir, process_basename.as_str());
    let (non_blocking_appender, _guard) = non_blocking(file_appender);

    // Store the guard in a static to keep it alive for the duration of the program
    // This prevents the non-blocking writer from being dropped prematurely
    static mut GUARD: Option<tracing_appender::non_blocking::WorkerGuard> = None;
    unsafe {
        GUARD = Some(_guard);
    }

    // Create console layer for stdout
    let console_layer = fmt::layer().with_target(true).with_ansi(true);

    // Create JSON-formatted file layer
    let file_layer = fmt::layer()
        .with_ansi(false)
        .with_writer(non_blocking_appender)
        .json();

    // Create environment filter from log level
    let env_filter: EnvFilter = EnvFilter::try_from_default_env()
        .or_else(|_| EnvFilter::try_new(&log_level))
        .unwrap();

    // Combine all layers
    tracing_subscriber::registry()
        .with(env_filter)
        .with(console_layer)
        .with(file_layer)
        .init();

    info!("Logging initialized with level: {}", log_level);
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    // Set up logging
    if let Err(e) = setup_logging() {
        eprintln!("Failed to initialize logging: {}", e);
        std::process::exit(1);
    }

    match &*PROCESSINFO {
        Ok(process_info) => {
            info!("{}", process_info);
        }
        Err(e) => {
            error!("Failed to retrieve process info: {}", e);
            std::process::exit(1);
        }
    }

    match &*RUNTIMECONFIG {
        Ok(runtime_config) => {
            info!("{}", runtime_config);
        }
        Err(e) => {
            error!("Failed to retrieve runtime config: {}", e);
            std::process::exit(1);
        }
    }

    let shutdown: Shutdown = tokio_graceful::Shutdown::default();

    // Short for `shutdown.guard().into_spawn_task_fn(serve_tcp)`
    // In case you only wish to pass in a future (in contrast to a function)
    // as you do not care about being able to use the linked guard,
    // you can also use [`Shutdown::spawn_task`](https://docs.rs/tokio-graceful/latest/tokio_graceful/struct.Shutdown.html#method.spawn_task).
    shutdown.spawn_task_fn(tokio_main);

    // use [`Shutdown::shutdown`](https://docs.rs/tokio-graceful/latest/tokio_graceful/struct.Shutdown.html#method.shutdown)
    // to wait for all guards to drop without any limit on how long to wait.
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

async fn tokio_main(shutdown_guard: tokio_graceful::ShutdownGuard) {
    let process_basename: String = PROCESSINFO.as_ref().unwrap().process_basename.clone();
    let process_options: BTreeMap<String, String> =
        RUNTIMECONFIG.as_ref().unwrap().config_options.clone();
    let tcp_port: String = process_options.get("Tcp:Port").unwrap().clone();

    // if let Err(e) = CRON.start().await {
    //     eprintln!("Error on scheduler {:?}", e);
    // }
    // tokio::spawn(CRON.start());

    let tcplistener: String = format!("0.0.0.0:{}", tcp_port);
    let listener: TcpListener = TcpListener::bind(tcplistener.clone()).await.unwrap();
    info!(
        "{:?} listening on {}",
        process_basename,
        listener.local_addr().unwrap()
    );

    let (tx, rx) = mpsc::channel(32);
    task::spawn(espeak_exec(rx));

    loop {
        let shutdown_guard: tokio_graceful::ShutdownGuard = shutdown_guard.clone();
        tokio::select! {
            _ = shutdown_guard.cancelled() => {
                info!("Signal received: initiate graceful shutdown");
                break;
            }
            result = listener.accept() => {
                match result {
                    Ok((socket, _)) => {
                        let tx = tx.clone();
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

async fn espeak_exec(mut rx: Receiver<Espeak>) {
    while let Some(say_this) = rx.recv().await {
        let say_this_json: Value = serde_json::to_value(say_this.clone()).unwrap();
        let mut command: Command = tokio::process::Command::new("espeak-ng");

        // Add all values of say_this.options.voptions as arguments to the command
        if let Some(options) = say_this.options.voptions.as_array() {
            for option in options {
                if let Some(option_str) = option.as_str() {
                    command.arg(option_str);
                }
            }
        }
        command.arg(say_this.message.text);
        trace!("Executing command: {:?}", command);

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

async fn handle_client(mut client: tokio::net::TcpStream, tx: Sender<Espeak>) -> Result<()> {
    let mut buffer = [0; 4096];

    loop {
        let n = match client.read(&mut buffer).await {
            Ok(n) if n == 0 => break Ok(()), // connection closed
            Ok(n) => n,
            Err(e) => {
                warn!("Failed to read from socket; err = {:?}", e);
                break Ok(());
            }
        };

        let data: &str = match std::str::from_utf8(&buffer[..n]) {
            Ok(data) => data,
            Err(_) => continue,
        };

        trace!("Raw data: {}", data);

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
