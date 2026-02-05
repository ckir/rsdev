
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
pub struct VoiceOptions {
    pub volume: i64,
    pub voice: String,
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

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct LoggerLocalOptions {
    pub use_tty: Option<Vec<i64>>,
    pub use_voice: Option<VoiceOptions>,
    pub use_file: Option<Vec<i64>>,
    pub log_dir: Option<PathBuf>,
}

pub struct LoggerLocal {
    app_name: String,
    options: LoggerLocalOptions,
    say_mutex: Arc<Mutex<()>>,
    current_log_file: Option<PathBuf>,
}

    impl LoggerLocal {
        // ... existing methods ...

        fn rotate_logs(app_name: &str, log_dir: &Path) {
            let pattern = format!("{}/{}-*.log", log_dir.display(), app_name);
            let mut log_files: Vec<PathBuf> = Vec::new();

            for entry in glob(&pattern).expect("Failed to read glob pattern for log rotation") {
                if let Ok(path) = entry {
                    log_files.push(path);
                }
            }

            // Sort by filename (which contains timestamp), newest first
            log_files.sort_by(|a, b| {
                b.file_name().expect("No filename").cmp(a.file_name().expect("No filename"))
            });

            // Keep only the first (newest) file, delete the rest
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
                let log_base_dir = logger.options.log_dir.clone().unwrap_or_else(|| {
                    super::logrecord::PROCESSINFO.as_ref().ok()
                        .map(|info| PathBuf::from(&info.process_location))
                        .unwrap_or_else(|| PathBuf::from("."))
                });
                
                if let Err(e) = std::fs::create_dir_all(&log_base_dir) {
                    eprintln!("Error creating log directory {}: {}", log_base_dir.display(), e);
                }

                LoggerLocal::rotate_logs(&app_name, &log_base_dir);
                
                let timestamp = Local::now().format("%Y%m%d_%H%M%S").to_string();
                let current_log_filename = format!("{}-{}.log", app_name, timestamp);
                logger.current_log_file = Some(log_base_dir.join(current_log_filename));
            }

            logger
        }
    pub async fn log(&self, log_level: i64, log_message: &str, log_extras: Option<Value>) {
        let mut record = Logrecord::default();
        record.app.name = self.app_name.clone();
        record.loglevel = log_level;
        record.message.text = log_message.to_string();
        if let Some(extras) = log_extras {
            record.tags = extras;
        }

        let ts = &record.rfc9557.as_str().truecolor(128, 128, 128);
        let app_name_colored = format!("[{}]", self.app_name).truecolor(128, 128, 128);

        if let Some(tty_levels) = &self.options.use_tty {
            if tty_levels.contains(&log_level) {
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
                if record.tags != serde_json::json!([]) {
                    if let Ok(tags_str) = serde_json::to_string(&record.tags) {
                        println!("{}{}{}", ts, app_name_colored, tags_str.truecolor(128,128,128));
                    }
                }
            }
        }

        if let Some(voice_opts) = &self.options.use_voice {
            if voice_opts.levels.contains(&log_level) {
                self.say(log_message, Some(voice_opts.volume), Some(voice_opts.voice.clone())).await;
            }
        }

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
    
    pub async fn say(&self, log_message: &str, volume: Option<i64>, voice: Option<String>) {
        let _guard = self.say_mutex.lock().await;

        let voice_opts = self.options.use_voice.as_ref().unwrap();
        let vol = volume.unwrap_or(voice_opts.volume);
        let vce = voice.unwrap_or_else(|| voice_opts.voice.clone());
        let msg = log_message.to_string();

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

    pub async fn silly(&self, log_message: &str, log_extras: Option<Value>) {
        self.log(0, log_message, log_extras).await;
    }

    pub async fn trace(&self, log_message: &str, log_extras: Option<Value>) {
        self.log(1, log_message, log_extras).await;
    }

    pub async fn debug(&self, log_message: &str, log_extras: Option<Value>) {
        self.log(2, log_message, log_extras).await;
    }

    pub async fn info(&self, log_message: &str, log_extras: Option<Value>) {
        self.log(3, log_message, log_extras).await;
    }

    pub async fn warn(&self, log_message: &str, log_extras: Option<Value>) {
        self.log(4, log_message, log_extras).await;
    }

    pub async fn error(&self, log_message: &str, log_extras: Option<Value>) {
        self.log(5, log_message, log_extras).await;
    }

    pub async fn fatal(&self, log_message: &str, log_extras: Option<Value>) {
        self.log(6, log_message, log_extras).await;
    }
}
