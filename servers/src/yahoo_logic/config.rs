use clap::Parser;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use dirs; // Added for home_dir()

#[derive(Parser, Deserialize, Serialize, Debug, Clone, Default)]
#[clap(about = "Yahoo Finance WebSocket Proxy Server", version)]
#[serde(rename_all = "camelCase")]
pub struct Config {
    #[clap(long, env = "YAHOO_PORT", help = "Port to listen on for client connections.")]
    pub port: Option<u16>,

    #[clap(long, env = "YAHOO_CONFIG_PATH", help = "Path to the JSON configuration file.")]
    pub config_path: Option<PathBuf>,

    #[clap(long, env = "YAHOO_LOG_DIR", help = "Directory for log files.")]
    pub log_dir: Option<PathBuf>,

    #[clap(long, env = "YAHOO_LOG_LEVEL", help = "Logging level (trace, debug, info, warn, error, fatal).")]
    pub log_level: Option<String>,

    #[clap(long, env = "YAHOO_URL", help = "Upstream Yahoo Finance WebSocket URL.")]
    pub yahoo_ws_url: Option<String>,

    #[clap(long, env = "YAHOO_RECONNECT_BASE_DELAY_MS", help = "Base delay in milliseconds for upstream reconnect attempts.")]
    pub reconnect_base_delay_ms: Option<u64>,

    #[clap(long, env = "YAHOO_RECONNECT_MAX_DELAY_MS", help = "Maximum delay in milliseconds for upstream reconnect attempts.")]
    pub reconnect_max_delay_ms: Option<u64>,

    #[clap(long, env = "YAHOO_HEARTBEAT_THRESHOLD_SECONDS", help = "Seconds of inactivity before upstream heartbeat is considered lost.")]
    pub heartbeat_threshold_seconds: Option<u64>,

    #[clap(long, env = "YAHOO_DATAFLOW_CHECK_INTERVAL_SECONDS", help = "Interval in seconds to check dataflow from upstream.")]
    pub dataflow_check_interval_seconds: Option<u64>,

    #[clap(long, env = "YAHOO_DATAFLOW_INACTIVITY_THRESHOLD_SECONDS", help = "Seconds of no dataflow before triggering upstream reconnection.")]
    pub dataflow_inactivity_threshold_seconds: Option<u64>,

    #[clap(long, env = "YAHOO_PROTO_PATH", help = "Path to the Protobuf schema file (for internal use).")]
    pub proto_path: Option<PathBuf>,

    #[clap(long, env = "TLS_CERT_PATH", help = "Path to the TLS certificate file.")]
    pub tls_cert_path: Option<PathBuf>,

    #[clap(long, env = "TLS_KEY_PATH", help = "Path to the TLS private key file.")]
    pub tls_key_path: Option<PathBuf>,
}

impl Config {
    // Merge two Config structs, where 'other' overrides 'self' for Some values
    fn merge(self, other: Config) -> Config {
        Config {
            port: other.port.or(self.port),
            config_path: other.config_path.or(self.config_path),
            log_dir: other.log_dir.or(self.log_dir),
            log_level: other.log_level.or(self.log_level),
            yahoo_ws_url: other.yahoo_ws_url.or(self.yahoo_ws_url),
            reconnect_base_delay_ms: other.reconnect_base_delay_ms.or(self.reconnect_base_delay_ms),
            reconnect_max_delay_ms: other.reconnect_max_delay_ms.or(self.reconnect_max_delay_ms),
            heartbeat_threshold_seconds: other.heartbeat_threshold_seconds.or(self.heartbeat_threshold_seconds),
            dataflow_check_interval_seconds: other.dataflow_check_interval_seconds.or(self.dataflow_check_interval_seconds),
            dataflow_inactivity_threshold_seconds: other.dataflow_inactivity_threshold_seconds.or(self.dataflow_inactivity_threshold_seconds),
            proto_path: other.proto_path.or(self.proto_path),
            tls_cert_path: other.tls_cert_path.or(self.tls_cert_path),
            tls_key_path: other.tls_key_path.or(self.tls_key_path),
        }
    }
}

pub fn load_config() -> Config {
    // 1. Load defaults
    let default_config = Config {
        port: Some(9002),
        log_dir: Some(PathBuf::from("./logs")),
        log_level: Some("info".to_string()),
        yahoo_ws_url: Some("wss://streamer.finance.yahoo.com/?version=2".to_string()),
        reconnect_base_delay_ms: Some(1000),
        reconnect_max_delay_ms: Some(60000),
        heartbeat_threshold_seconds: Some(30),
        dataflow_check_interval_seconds: Some(10),
        dataflow_inactivity_threshold_seconds: Some(60),
        ..Default::default()
    };

    // 2. Load from config file (server_yahoo.conf) if present.
    //    Allow overriding default config file path with CLI arg.
    let cli_args_for_path = Config::parse(); // Parse CLI to get potential config_path override early

    let config_file_path = cli_args_for_path.config_path
        .clone()
        .unwrap_or_else(|| PathBuf::from("server_yahoo.conf"));

    let mut current_config = default_config;

    if config_file_path.exists() {
        if let Ok(config_str) = fs::read_to_string(&config_file_path) {
            if let Ok(file_config) = serde_json::from_str::<Config>(&config_str) {
                current_config = current_config.merge(file_config);
            } else {
                log::warn!("Failed to parse config file: {}. Falling back to other sources.", config_file_path.display());
            }
        } else {
            log::warn!("Failed to read config file: {}. Falling back to other sources.", config_file_path.display());
        }
    } else {
        log::info!("Config file not found at {}. Using defaults and environment/CLI variables.", config_file_path.display());
    }

    // 3. Override with environment variables and CLI arguments
    //    clap::Parser automatically handles env vars and CLI args.
    //    We merge CLI args (which include env vars) over the file config.
    //    Parse CLI args again without default_value, as it will take values already set in `current_config`
    let cli_args_final = Config::parse();
    current_config = current_config.merge(cli_args_final);

    // 4. Apply default TLS paths if not already set
    if current_config.tls_cert_path.is_none() || current_config.tls_key_path.is_none() {
        if let Some(home_dir) = dirs::home_dir() {
            let letsencrypt_dir = home_dir.join(".letsencrypt");
            if current_config.tls_cert_path.is_none() {
                current_config.tls_cert_path = Some(letsencrypt_dir.join("fullchain.pem"));
            }
            if current_config.tls_key_path.is_none() {
                current_config.tls_key_path = Some(letsencrypt_dir.join("privkey.pem"));
            }
        } else {
            log::warn!("Could not determine home directory for default TLS paths.");
        }
    }

    current_config
}