
//! # Local Logger Implementation
//!
//! Provides a `tracing`-compatible logger that supports:
//! 1. TTY (console) output with colors.
//! 2. File output with daily rotation.
//! 3. Voice synthesis (TTS) for alerts via `wsay` (Windows) or `espeak` (Linux).

use super::logrecord::PROCESSINFO;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fs::OpenOptions;
use std::io::Write;
use std::process::Command;
use std::sync::Arc;
use tokio::sync::{Mutex, mpsc};
use tokio::task;
use colored::*;
use chrono::Local;
use glob::glob;
use std::path::{Path, PathBuf};
use tracing::{Level, field::Visit, Event, Subscriber};
use tracing_subscriber::{Layer, registry::LookupSpan, prelude::*};

/// Configuration options for voice synthesis alerts.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct VoiceOptions {
    /// Volume level (0-100).
    pub volume: i64,
    /// Name of the voice to use.
    pub voice: String,
    /// Log levels that should trigger a voice alert.
    pub levels: Vec<i64>,
}

impl Default for VoiceOptions {
    fn default() -> Self {
        Self {
            volume: 100,
            voice: if cfg!(target_os = "linux") { "en+f2".to_string() } else { "5".to_string() },
            levels: vec![6, 5, 4],
        }
    }
}

/// Global configuration options for the local logger.
#[derive(Serialize, Deserialize, Debug, Clone, Default, PartialEq)]
pub struct LoggerLocalOptions {
    /// Log levels to output to TTY.
    pub use_tty: Option<Vec<i64>>,
    /// Options for voice alerts.
    pub use_voice: Option<VoiceOptions>,
    /// Log levels to output to file.
    pub use_file: Option<Vec<i64>>,
    /// Custom directory for log files.
    pub log_dir: Option<PathBuf>,
}

/// Internal task sent to the voice background worker.
struct VoiceTask {
    message: String,
    volume: i64,
    voice: String,
}

/// A `tracing` layer that handles log events for TTY, file, and voice.
pub struct LoggerLocalLayer {
    /// Name of the application.
    app_name: String,
    /// Active logger options.
    options: LoggerLocalOptions,
    /// Path to the current active log file.
    current_log_file: Option<PathBuf>,
    /// Channel for sending tasks to the voice worker.
    voice_tx: Option<mpsc::UnboundedSender<VoiceTask>>,
}

impl LoggerLocalLayer {
    /// Writes a log message and its tags to the active log file.
    fn log_to_file(&self, log_level: i64, rfc9557: &str, message: &str, tags: &Value) {
        if let Some(file_levels) = &self.options.use_file {
            if file_levels.contains(&log_level) {
                if let Some(log_file_path) = &self.current_log_file {
                    let formatted_message = format!("{} [{}] {}\n", rfc9557, self.app_name, message);
                    let tags_str = serde_json::to_string(tags).unwrap_or_else(|_| "[]".to_string());
                    
                    let result = if tags != &serde_json::json!([]) {
                        OpenOptions::new()
                            .create(true)
                            .append(true)
                            .open(log_file_path)
                            .and_then(|mut file| writeln!(file, "{}{}", formatted_message, tags_str))
                    } else {
                        OpenOptions::new()
                            .create(true)
                            .append(true)
                            .open(log_file_path)
                            .and_then(|mut file| write!(file, "{}", formatted_message))
                    };

                    if let Err(e) = result {
                        eprintln!("Error writing to log file: {}", e);
                    }
                }
            }
        }
    }

    /// Prints a colorized log message to the console.
    fn print_to_tty(&self, log_level: i64, rfc9557: &str, message: &str, tags: &Value) {
        if let Some(tty_levels) = &self.options.use_tty {
            if tty_levels.contains(&log_level) {
                let ts = rfc9557.truecolor(128, 128, 128);
                let app_name_colored = format!("[{}]", self.app_name).truecolor(128, 128, 128);

                let colored_message = match log_level {
                    6 => message.bright_white().on_bright_red(),    // Fatal
                    5 => message.bright_red(),                     // Error
                    4 => message.bright_yellow(),                  // Warn
                    3 => message.bright_green(),                   // Info
                    2 => message.bright_white(),                   // Debug
                    1 => message.bright_cyan(),                    // Trace
                    _ => message.blue(),                           // Silly
                };

                println!("{}{}\n{}", ts, app_name_colored, colored_message);
                if tags != &serde_json::json!([]) {
                    if let Ok(tags_str) = serde_json::to_string(tags) {
                        println!("{}{}{}", ts, app_name_colored, tags_str.truecolor(128, 128, 128));
                    }
                }
            }
        }
    }

    /// Triggers a voice alert if the log level qualifies.
    fn handle_voice(&self, log_level: i64, message: &str) {
        if let Some(voice_opts) = &self.options.use_voice {
            if voice_opts.levels.contains(&log_level) {
                if let Some(tx) = &self.voice_tx {
                    let _ = tx.send(VoiceTask {
                        message: message.to_string(),
                        volume: voice_opts.volume,
                        voice: voice_opts.voice.clone(),
                    });
                }
            }
        }
    }
}

/// Visitor to extract fields from `tracing` events.
struct EventVisitor {
    message: String,
    loglevel: Option<i64>,
    extras: Value,
}

impl Visit for EventVisitor {
    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
        if field.name() == "message" {
            self.message = format!("{:?}", value);
        } else if field.name() == "loglevel" {
            // Handled separately
        } else {
            self.extras[field.name()] = serde_json::json!(format!("{:?}", value));
        }
    }

    fn record_i64(&mut self, field: &tracing::field::Field, value: i64) {
        if field.name() == "loglevel" {
            self.loglevel = Some(value);
        } else {
            self.extras[field.name()] = serde_json::json!(value);
        }
    }

    fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
        if field.name() == "message" {
            self.message = value.to_string();
        } else if field.name() == "extras" {
            if let Ok(val) = serde_json::from_str(value) {
                self.extras = val;
            } else {
                self.extras["extras_raw"] = serde_json::json!(value);
            }
        } else {
            self.extras[field.name()] = serde_json::json!(value);
        }
    }
}

impl<S> Layer<S> for LoggerLocalLayer
where
    S: Subscriber + for<'a> LookupSpan<'a>,
{
    fn on_event(&self, event: &Event<'_>, _ctx: tracing_subscriber::layer::Context<'_, S>) {
        let mut visitor = EventVisitor {
            message: String::new(),
            loglevel: None,
            extras: serde_json::json!([]),
        };
        event.record(&mut visitor);

        let log_level = visitor.loglevel.unwrap_or_else(|| {
            match *event.metadata().level() {
                Level::ERROR => 5,
                Level::WARN => 4,
                Level::INFO => 3,
                Level::DEBUG => 2,
                Level::TRACE => 1,
            }
        });

        let rfc9557 = crate::utils::misc::utils::current_datetime_rfc9557();
        
        self.print_to_tty(log_level, &rfc9557, &visitor.message, &visitor.extras);
        self.log_to_file(log_level, &rfc9557, &visitor.message, &visitor.extras);
        self.handle_voice(log_level, &visitor.message);
    }
}

/// The main logger controller.
pub struct LoggerLocal {
    /// Name of the application.
    pub app_name: String,
    /// Active logger options.
    pub options: LoggerLocalOptions,
    /// Mutex for synchronizing direct `say` calls.
    say_mutex: Arc<Mutex<()>>,
}

impl LoggerLocal {
    /// Removes old log files, keeping only the most recent one.
    fn rotate_logs(app_name: &str, log_dir: &Path) {
        let pattern = format!("{}/{}-*.log", log_dir.display(), app_name);
        let mut log_files: Vec<PathBuf> = Vec::new();

        if let Ok(entries) = glob(&pattern) {
            for path in entries.flatten() {
                log_files.push(path);
            }
        }

        // Sort descending by filename (which includes timestamp)
        log_files.sort_by(|a, b| {
            b.file_name().unwrap_or_default().cmp(a.file_name().unwrap_or_default())
        });

        if log_files.len() > 1 {
            // Keep the first (most recent) and delete the rest
            for old_file in log_files.iter().skip(1) {
                let _ = std::fs::remove_file(old_file);
            }
        }
    }

    /// Creates a new `LoggerLocal` instance.
    pub fn new(app_name: String, options: Option<LoggerLocalOptions>) -> Self {
        let default_options = LoggerLocalOptions {
            use_tty: Some(vec![6, 5, 4, 3, 2, 1, 0]),
            use_voice: Some(VoiceOptions::default()),
            use_file: Some(vec![6, 5, 4, 3, 2, 1, 0]),
            log_dir: None,
        };
        let opts = options.unwrap_or(default_options);

        Self {
            app_name,
            options: opts,
            say_mutex: Arc::new(Mutex::new(())),
        }
    }

    /// Initializes and returns a `tracing` layer.
    pub fn init_layer(&self) -> LoggerLocalLayer {
        let mut current_log_file = None;

        if self.options.use_file.is_some() {
            let log_base_dir = self.options.log_dir.clone().unwrap_or_else(|| {
                PROCESSINFO.as_ref().ok()
                    .map(|info| PathBuf::from(&info.process_location))
                    .unwrap_or_else(|| PathBuf::from("."))
            });
            
            let _ = std::fs::create_dir_all(&log_base_dir);
            Self::rotate_logs(&self.app_name, &log_base_dir);
            
            let timestamp = Local::now().format("%Y%m%d_%H%M%S").to_string();
            let current_log_filename = format!("{}-{}.log", self.app_name, timestamp);
            current_log_file = Some(log_base_dir.join(current_log_filename));
        }

        let (voice_tx, mut voice_rx) = mpsc::unbounded_channel::<VoiceTask>();
        let say_mutex_clone = self.say_mutex.clone();

        // Background task for voice alerts
        task::spawn(async move {
            while let Some(task) = voice_rx.recv().await {
                let _guard = say_mutex_clone.lock().await;
                let msg = task.message;
                let vol = task.volume;
                let vce = task.voice;

                let _ = task::spawn_blocking(move || {
                    if cfg!(target_os = "windows") {
                        let _ = Command::new("wsay")
                            .arg("-V")
                            .arg(vol.to_string())
                            .arg("-v")
                            .arg(vce)
                            .arg(&msg)
                            .output();
                    } else {
                        let _ = Command::new("espeak")
                            .arg(format!("-a{}", vol))
                            .arg(format!("-v{}", vce))
                            .arg(&msg)
                            .output();
                    };
                }).await;
            }
        });

        LoggerLocalLayer {
            app_name: self.app_name.clone(),
            options: self.options.clone(),
            current_log_file,
            voice_tx: Some(voice_tx),
        }
    }

    /// Initializes tracing globally with this logger.
    pub fn init_global(&self) {
        let layer = self.init_layer();
        let subscriber = tracing_subscriber::registry().with(layer);
        let _ = tracing::subscriber::set_global_default(subscriber);
    }

    /// Logs a message at the specified level.
    pub async fn log(&self, log_level: i64, log_message: &str, log_extras: Option<Value>) {
        match log_level {
            6 => tracing::error!(loglevel = 6, extras = ?log_extras, "{}", log_message),
            5 => tracing::error!(loglevel = 5, extras = ?log_extras, "{}", log_message),
            4 => tracing::warn!(loglevel = 4, extras = ?log_extras, "{}", log_message),
            3 => tracing::info!(loglevel = 3, extras = ?log_extras, "{}", log_message),
            2 => tracing::debug!(loglevel = 2, extras = ?log_extras, "{}", log_message),
            1 => tracing::trace!(loglevel = 1, extras = ?log_extras, "{}", log_message),
            _ => tracing::trace!(loglevel = 0, extras = ?log_extras, "{}", log_message),
        }
    }

    /// Triggers an immediate voice synthesis alert.
    pub async fn say(&self, log_message: &str, volume: Option<i64>, voice: Option<String>) {
        let voice_opts = self.options.use_voice.as_ref().unwrap();
        let vol = volume.unwrap_or(voice_opts.volume);
        let vce = voice.unwrap_or_else(|| voice_opts.voice.clone());
        let msg = log_message.to_string();

        let _guard = self.say_mutex.lock().await;
        let _ = task::spawn_blocking(move || {
            if cfg!(target_os = "windows") {
                let _ = Command::new("wsay")
                    .arg("-V")
                    .arg(vol.to_string())
                    .arg("-v")
                    .arg(vce)
                    .arg(&msg)
                    .output();
            } else {
                let _ = Command::new("espeak")
                    .arg(format!("-a{}", vol))
                    .arg(format!("-v{}", vce))
                    .arg(&msg)
                    .output();
            };
        }).await;
    }

    /// Logs a message at level 0 (Silly).
    pub async fn silly(&self, log_message: &str, log_extras: Option<Value>) {
        self.log(0, log_message, log_extras).await;
    }

    /// Logs a message at level 1 (Trace).
    pub async fn trace(&self, log_message: &str, log_extras: Option<Value>) {
        self.log(1, log_message, log_extras).await;
    }

    /// Logs a message at level 2 (Debug).
    pub async fn debug(&self, log_message: &str, log_extras: Option<Value>) {
        self.log(2, log_message, log_extras).await;
    }

    /// Logs a message at level 3 (Info).
    pub async fn info(&self, log_message: &str, log_extras: Option<Value>) {
        self.log(3, log_message, log_extras).await;
    }

    /// Logs a message at level 4 (Warn).
    pub async fn warn(&self, log_message: &str, log_extras: Option<Value>) {
        self.log(4, log_message, log_extras).await;
    }

    /// Logs a message at level 5 (Error).
    pub async fn error(&self, log_message: &str, log_extras: Option<Value>) {
        self.log(5, log_message, log_extras).await;
    }

    /// Logs a message at level 6 (Fatal).
    pub async fn fatal(&self, log_message: &str, log_extras: Option<Value>) {
        self.log(6, log_message, log_extras).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_logger_local_options_default() {
        let opts = LoggerLocalOptions::default();
        assert!(opts.use_tty.is_none());
    }

    #[tokio::test]
    async fn test_logger_local_new() {
        let logger = LoggerLocal::new("test_app".into(), None);
        assert_eq!(logger.app_name, "test_app");
        assert!(logger.options.use_tty.is_some());
    }

    #[tokio::test]
    async fn test_logger_init_layer() {
        let logger = LoggerLocal::new("test_app".into(), None);
        let layer = logger.init_layer();
        assert_eq!(layer.app_name, "test_app");
    }
}

