
use super::logrecord::Logrecord;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fs::OpenOptions;
use std::io::Write;
use std::process::Command;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::task;
use colored::*;
use chrono::Local;
use glob::glob;
use std::path::{Path, PathBuf};

#[derive(Serialize, Deserialize, Debug, Clone)]
/// # Voice Options
///
/// Configuration settings for text-to-speech (TTS) output.
pub struct VoiceOptions {
    /// The volume level for the voice output (e.g., 100).
    pub volume: i64,
    /// The specific voice to use for TTS (e.g., "en+f2" for espeak, "5" for wsay).
    pub voice: String,
    /// A list of log levels for which voice alerts should be triggered.
    pub levels: Vec<i64>,
}

impl Default for VoiceOptions {
    /// Provides default `VoiceOptions`: volume 100, OS-dependent voice, and all log levels enabled.
    fn default() -> Self {
        Self {
            volume: 100,
            voice: if cfg!(target_os = "linux") { "en+f2".to_string() } else { "5".to_string() },
            levels: vec![6, 5, 4],
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
/// # Logger Local Options
///
/// Configuration options for the `LoggerLocal` instance, controlling where and how
/// log messages are output.
pub struct LoggerLocalOptions {
    /// A list of log levels that should be printed to the TTY (console).
    pub use_tty: Option<Vec<i64>>,
    /// Optional `VoiceOptions` for text-to-speech output.
    pub use_voice: Option<VoiceOptions>,
    /// A list of log levels that should be written to a log file.
    pub use_file: Option<Vec<i64>>,
    /// The directory where log files should be stored. If `None`, defaults to the executable's directory.
    pub log_dir: Option<PathBuf>,
}

pub struct LoggerLocal {
    /// The name of the application associated with this logger instance.
    app_name: String,
    /// Configuration options determining logging behavior.
    options: LoggerLocalOptions,
    /// A mutex to ensure only one voice message is played at a time, preventing overlapping audio.
    say_mutex: Arc<Mutex<()>>,
    /// The path to the currently active log file, if file logging is enabled.
    current_log_file: Option<PathBuf>,
}

    impl LoggerLocal {
        // ... existing methods ...

        /// Rotates log files for a given application and log directory.
        ///
        /// This function keeps only the most recent log file (based on timestamp in filename)
        /// and deletes older log files for the specified application within the given directory.
        ///
        /// # Arguments
        /// * `app_name` - The name of the application whose logs are being rotated.
        /// * `log_dir` - The directory containing the log files.
        fn rotate_logs(app_name: &str, log_dir: &Path) {
            /// Constructs a glob pattern to find all log files belonging to the application.
            let pattern = format!("{}/{}-*.log", log_dir.display(), app_name);
            let mut log_files: Vec<PathBuf> = Vec::new();

            /// Collects all matching log files.
            for entry in glob(&pattern).expect("Failed to read glob pattern for log rotation") {
                if let Ok(path) = entry {
                    log_files.push(path);
                }
            }

            /// Sorts log files by filename in descending order (newest first).
            log_files.sort_by(|a, b| {
                b.file_name().expect("No filename").cmp(a.file_name().expect("No filename"))
            });

            /// Keeps only the newest log file and deletes all older ones.
            if log_files.len() > 1 {
                for old_file in log_files.iter().skip(1) {
                    if let Err(e) = std::fs::remove_file(old_file) {
                        eprintln!("Error deleting old log file {}: {}", old_file.display(), e);
                    } else {
                        // eprintln!("Deleted old log file: {}", old_file.display()); // Too verbose
                    }
                }
            }
        }
        /// Creates a new `LoggerLocal` instance.
        ///
        /// Initializes the logger with an application name and optional configuration.
        /// If file logging is enabled, it ensures the log directory exists,
        /// rotates old logs, and sets up the current log file path.
        ///
        /// # Arguments
        /// * `app_name` - The name of the application using this logger.
        /// * `options` - Optional `LoggerLocalOptions` to customize logging behavior.
        ///   If `None`, default options are used (TTY, voice, and file logging for all levels).
        pub fn new(app_name: String, options: Option<LoggerLocalOptions>) -> Self {
            let default_options = LoggerLocalOptions {
                use_tty: Some(vec![6, 5, 4, 3, 2, 1, 0]),
                use_voice: Some(VoiceOptions::default()),
                use_file: Some(vec![6, 5, 4, 3, 2, 1, 0]),
                log_dir: None,
            };
            let opts = options.unwrap_or(default_options);

            let mut logger = Self {
                app_name: app_name.clone(),
                options: opts,
                say_mutex: Arc::new(Mutex::new(())),
                current_log_file: None, // Will be set below if use_file is enabled
            };

            if logger.options.use_file.is_some() {
                /// Determines the base directory for log files. Defaults to executable location if not specified.
                let log_base_dir = logger.options.log_dir.clone().unwrap_or_else(|| {
                    super::logrecord::PROCESSINFO.as_ref().ok()
                        .map(|info| PathBuf::from(&info.process_location))
                        .unwrap_or_else(|| PathBuf::from("."))
                });
                
                /// Creates the log directory if it doesn't already exist.
                if let Err(e) = std::fs::create_dir_all(&log_base_dir) {
                    eprintln!("Error creating log directory {}: {}", log_base_dir.display(), e);
                }

                /// Rotates old log files to keep only the newest one.
                LoggerLocal::rotate_logs(&app_name, &log_base_dir);
                
                /// Generates a timestamped filename for the current log file.
                let timestamp = Local::now().format("%Y%m%d_%H%M%S").to_string();
                let current_log_filename = format!("{}-{}.log", app_name, timestamp);
                logger.current_log_file = Some(log_base_dir.join(current_log_filename));
            }

            logger
        }
    /// Asynchronously logs a message with a specified level, handling TTY output,
    /// voice alerts, and file writing based on the logger's configuration.
    ///
    /// # Arguments
    /// * `log_level` - The numeric log level (e.g., 0 for Silly, 6 for Fatal).
    /// * `log_message` - The main message string to be logged.
    /// * `log_extras` - An `Option<Value>` for additional structured data to include in the log.
    pub async fn log(&self, log_level: i64, log_message: &str, log_extras: Option<Value>) {
        /// Initializes a default `Logrecord` and populates it with application name, log level, and message.
        let mut record = Logrecord::default();
        record.app.name = self.app_name.clone();
        record.loglevel = log_level;
        record.message.text = log_message.to_string();
        if let Some(extras) = log_extras {
            record.tags = extras;
        }

        let ts = &record.rfc9557.as_str().truecolor(128, 128, 128);
        let app_name_colored = format!("[{}]", self.app_name).truecolor(128, 128, 128);

        /// Handles TTY (console) output if enabled for the current log level.
        if let Some(tty_levels) = &self.options.use_tty {
            if tty_levels.contains(&log_level) {
                /// Colors the message based on its log level for better readability in the console.
                let colored_message = match log_level {
                    6 => log_message.bright_white().on_bright_red(),    // Fatal
                    5 => log_message.bright_red(),                     // Error
                    4 => log_message.bright_yellow(),                  // Warn
                    3 => log_message.bright_green(),                   // Info
                    2 => log_message.bright_white(),                   // Debug
                    1 => log_message.bright_cyan(),                    // Trace
                    _ => log_message.blue(),                           // Silly
                };

                println!("{}{}\n{}", ts, app_name_colored, colored_message);
                /// Prints additional structured data (tags) to the console if present.
                if record.tags != serde_json::json!([]) {
                    if let Ok(tags_str) = serde_json::to_string(&record.tags) {
                        println!("{}{}{}", ts, app_name_colored, tags_str.truecolor(128,128,128));
                    }
                }
            }
        }

        /// Triggers a voice alert if enabled for the current log level.
        if let Some(voice_opts) = &self.options.use_voice {
            if voice_opts.levels.contains(&log_level) {
                self.say(log_message, Some(voice_opts.volume), Some(voice_opts.voice.clone())).await;
            }
        }

        /// Writes the log message to a file if enabled for the current log level.
        if let Some(file_levels) = &self.options.use_file {
            if file_levels.contains(&log_level) {
                if let Some(log_file_path) = &self.current_log_file {
                    let formatted_message = format!("{} [{}] {}\n", record.rfc9557, self.app_name, log_message);
                    if let Ok(tags_str) = serde_json::to_string(&record.tags) {
                        if record.tags != serde_json::json!([]) {
                            // Append extras if present
                            let _ = OpenOptions::new()
                                .create(true)
                                .append(true)
                                .open(log_file_path)
                                .and_then(|mut file| write!(file, "{}{}\n", formatted_message, tags_str));
                        } else {
                            let _ = OpenOptions::new()
                                .create(true)
                                .append(true)
                                .open(log_file_path)
                                .and_then(|mut file| write!(file, "{}", formatted_message));
                        }
                    } else {
                        let _ = OpenOptions::new()
                            .create(true)
                            .append(true)
                            .open(log_file_path)
                            .and_then(|mut file| write!(file, "{}", formatted_message));
                    }
                }
            }
        }
    }
    
    /// Asynchronously converts a message to speech using either `wsay` (Windows) or
    /// `espeak` (Linux) in a blocking task to prevent blocking the async runtime.
    ///
    /// It acquires a mutex lock to ensure that only one voice message is played at a time.
    ///
    /// # Arguments
    /// * `log_message` - The text message to be spoken.
    /// * `volume` - An optional volume level for the voice output. Defaults to `VoiceOptions::volume`.
    /// * `voice` - An optional string specifying the voice to use. Defaults to `VoiceOptions::voice`.
    pub async fn say(&self, log_message: &str, volume: Option<i64>, voice: Option<String>) {
        /// Acquires a lock on `say_mutex` to ensure exclusive access to the voice output.
        let _guard = self.say_mutex.lock().await;

        let voice_opts = self.options.use_voice.as_ref().unwrap();
        let vol = volume.unwrap_or(voice_opts.volume);
        let vce = voice.unwrap_or_else(|| voice_opts.voice.clone());
        let msg = log_message.to_string();

        /// Spawns a blocking task to execute the external text-to-speech command.
        task::spawn_blocking(move || {
            let command_result = if cfg!(target_os = "windows") {
                Command::new("wsay")
                    .arg("-V")
                    .arg(vol.to_string())
                    .arg("-v")
                    .arg(vce)
                    .arg(&msg)
                    .output()
            } else {
                Command::new("espeak")
                    .arg(format!("-a{}", vol))
                    .arg(format!("-v{}", vce))
                    .arg(&msg)
                    .output()
            };

            if let Err(e) = command_result {
                eprintln!("{}", format!("Error saying [{}]: {}", msg, e).red());
            }
        }).await.unwrap();
    }

    /// Logs a message at the "Silly" (level 0) log level.
    ///
    /// This level is typically used for very fine-grained, verbose debugging information
    /// that is rarely needed.
    ///
    /// # Arguments
    /// * `log_message` - The message to log.
    /// * `log_extras` - Optional structured data to include.
    pub async fn silly(&self, log_message: &str, log_extras: Option<Value>) {
        self.log(0, log_message, log_extras).await;
    }

    /// Logs a message at the "Trace" (level 1) log level.
    ///
    /// This level is used for tracing the execution flow, typically showing
    /// function entry/exit, variable values, etc.
    ///
    /// # Arguments
    /// * `log_message` - The message to log.
    /// * `log_extras` - Optional structured data to include.
    pub async fn trace(&self, log_message: &str, log_extras: Option<Value>) {
        self.log(1, log_message, log_extras).await;
    }

    /// Logs a message at the "Debug" (level 2) log level.
    ///
    /// This level is used for detailed internal information that is helpful
    /// for debugging specific issues.
    ///
    /// # Arguments
    /// * `log_message` - The message to log.
    /// * `log_extras` - Optional structured data to include.
    pub async fn debug(&self, log_message: &str, log_extras: Option<Value>) {
        self.log(2, log_message, log_extras).await;
    }

    /// Logs a message at the "Info" (level 3) log level.
    ///
    /// This level is used for general application progress and important events
    /// that are not errors but provide useful context.
    ///
    /// # Arguments
    /// * `log_message` - The message to log.
    /// * `log_extras` - Optional structured data to include.
    pub async fn info(&self, log_message: &str, log_extras: Option<Value>) {
        self.log(3, log_message, log_extras).await;
    }

    /// Logs a message at the "Warn" (level 4) log level.
    ///
    /// This level indicates a potential problem or an unusual event that might
    /// require attention but doesn't prevent the application from continuing.
    ///
    /// # Arguments
    /// * `log_message` - The message to log.
    /// * `log_extras` - Optional structured data to include.
    pub async fn warn(&self, log_message: &str, log_extras: Option<Value>) {
        self.log(4, log_message, log_extras).await;
    }

    /// Logs a message at the "Error" (level 5) log level.
    ///
    /// This level signifies a serious problem that has occurred,
    /// typically indicating a failure in some operation or component.
    ///
    /// # Arguments
    /// * `log_message` - The message to log.
    /// * `log_extras` - Optional structured data to include.
    pub async fn error(&self, log_message: &str, log_extras: Option<Value>) {
        self.log(5, log_message, log_extras).await;
    }

    /// Logs a message at the "Fatal" (level 6) log level.
    ///
    /// This level indicates a critical error that is likely to cause the application
    /// to terminate or become unusable.
    ///
    /// # Arguments
    /// * `log_message` - The message to log.
    /// * `log_extras` - Optional structured data to include.
    pub async fn fatal(&self, log_message: &str, log_extras: Option<Value>) {
        self.log(6, log_message, log_extras).await;
    }
}
