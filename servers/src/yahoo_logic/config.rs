use clap::Parser;
use serde::Deserialize;
use std::fs;
use std::path::PathBuf;

#[derive(Parser, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Config {
    #[clap(long, env = "YAHOO_PORT", default_value = "9002")]
    pub port: u16,

    #[clap(long, env = "YAHOO_CONFIG_PATH")]
    pub config_path: Option<PathBuf>,

    #[clap(long, env = "YAHOO_LOG_DIR", default_value = "./logs")]
    pub log_dir: PathBuf,

    #[clap(long, env = "YAHOO_LOG_LEVEL", default_value = "info")]
    pub log_level: String,

    #[clap(long, env = "YAHOO_URL", default_value = "wss://streamer.finance.yahoo.com/?version=2")]
    pub yahoo_ws_url: String,

    #[clap(long, env = "YAHOO_PROTO_PATH")]
    pub proto_path: Option<PathBuf>,

    #[clap(long, env = "TLS_CERT_PATH")]
    pub tls_cert_path: Option<PathBuf>,

    #[clap(long, env = "TLS_KEY_PATH")]
    pub tls_key_path: Option<PathBuf>,
}

pub fn load_config() -> Config {
    let cli_config = Config::parse();

    let config_from_file = cli_config.config_path.as_ref().and_then(|path| {
        fs::read_to_string(path)
            .ok()
            .and_then(|c| serde_json::from_str::<Config>(&c).ok())
    });

    if let Some(file_config) = config_from_file {
        // Command-line arguments override file configuration
        Config {
            port: cli_config.port,
            config_path: cli_config.config_path.or(file_config.config_path),
            log_dir: cli_config.log_dir,
            log_level: cli_config.log_level,
            yahoo_ws_url: cli_config.yahoo_ws_url,
            proto_path: cli_config.proto_path.or(file_config.proto_path),
            tls_cert_path: cli_config.tls_cert_path.or(file_config.tls_cert_path),
            tls_key_path: cli_config.tls_key_path.or(file_config.tls_key_path),
        }
    } else {
        cli_config
    }
}
