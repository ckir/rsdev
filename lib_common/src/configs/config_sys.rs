#![allow(dead_code)]
#![allow(unused_variables)]
#![allow(unused_imports)]
#![allow(unreachable_code)]

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::{env, fmt, fs};

use serde::{Deserialize, Serialize};

use thiserror::Error;

use config::{ext::*, *};

const CONFIG_GLOBAL_NAME: &str = "config.global.json";

#[derive(Debug, Error)]
pub enum RuntimeConfigError {
    #[error("I/O error occurred: {0}")]
    IoError(#[from] std::io::Error),

    #[error("UTF-8 error occurred: {0}")]
    Utf8Error(#[from] std::str::Utf8Error),

    #[error("Command failed with non-zero exit status ({status}): {stderr}")]
    ExitStatusError { status: i32, stderr: String },

    #[error("Failed to execute the command: {0}")]
    ExecutionError(String),

    #[error("Environment variable error: {0}")]
    VarError(#[from] env::VarError),

    #[error("Environment variable {0} is not present")]
    MissingEnvVar(String),
}

#[derive(Default, Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all(deserialize = "PascalCase"))]
#[serde(rename_all(serialize = "PascalCase"))]
pub struct RuntimeConfig {
    pub config_running_mode: String,
    pub config_dir: String,
    pub config_global_file: String,
    pub config_common_file: String,
    pub config_mode_file: String,
    pub config_platform_file: String,
    pub config_options: BTreeMap<String, String>,
}

impl RuntimeConfig {
    pub fn new(
        config_running_mode: String,
        config_dir: String,
        config_global_file: String,
        config_common_file: String,
        config_mode_file: String,
        config_platform_file: String,
        config_options: BTreeMap<String, String>,
    ) -> Self {
        Self {
            config_running_mode: config_running_mode,
            config_dir: config_dir,
            config_global_file: config_global_file,
            config_common_file: config_common_file,
            config_mode_file: config_mode_file,
            config_platform_file: config_platform_file,
            config_options: config_options,
        }
    }
}

impl fmt::Display for RuntimeConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "RuntimeConfig
    Running mode: {},
    Config dir: {},
    Global file: {},
    Common file: {},
    Mode file: {},
    Platform file: {},
    Options: {:?}
",
            self.config_running_mode,
            self.config_dir,
            self.config_global_file,
            self.config_common_file,
            self.config_mode_file,
            self.config_platform_file,
            self.config_options
        )
    }
}

pub fn get_runtime_config() -> Result<RuntimeConfig, RuntimeConfigError> {
    let _current_exec: PathBuf = get_current_exe()?;
    let _basename: String = get_process_basename(_current_exec.clone())?.to_owned();
    let _location: String = get_process_location(_current_exec.clone())?.to_owned();
    let _running_mode: String = get_running_mode(_basename.clone())?.to_owned();
    let _config_dir: String = env::var("CONFIGS_LOCATION").unwrap_or(_location.clone());

    // This is the global configuration file
    let _global_file: PathBuf = PathBuf::from(_config_dir.clone()).join(CONFIG_GLOBAL_NAME);
    let mut _config_global_file: String = _global_file.clone().to_string_lossy().to_string();
    if !_global_file.is_file() {
        _config_global_file = "".to_string();
    }

    let _common_file: PathBuf =
        PathBuf::from(_config_dir.clone()).join(format!("{}.common.json", _basename.clone()));
    let mut _config_common_file: String = _common_file.clone().to_string_lossy().to_string();
    if !_common_file.is_file() {
        _config_common_file = "".to_string();
    }

    let _mode_file: PathBuf = PathBuf::from(_config_dir.clone()).join(format!(
        "{}.{}.json",
        _basename.clone(),
        _running_mode.clone()
    ));
    let mut _config_mode_file: String = _mode_file.clone().to_string_lossy().to_string();
    if !_mode_file.is_file() {
        _config_mode_file = "".to_string();
    }

    let _platform_file: PathBuf = PathBuf::from(_config_dir.clone()).join(format!(
        "{}.{}.{}.json",
        _basename.clone(),
        _running_mode.clone(),
        std::env::consts::OS
    ));
    let mut _config_platform_file: String = _platform_file.clone().to_string_lossy().to_string();
    if !_platform_file.is_file() {
        _config_platform_file = "".to_string();
    }

    let _config_data: Box<dyn ConfigurationRoot> = DefaultConfigurationBuilder::new()
        // .add_env_vars()
        .add_json_file(&_config_global_file.is().optional())
        .add_json_file(&_config_common_file.is().optional())
        .add_json_file(&_config_mode_file.is().optional())
        .add_json_file(&_config_platform_file.is().optional())
        .build()
        .unwrap();
    let mut _config_options: BTreeMap<String, String> = BTreeMap::new();
    for (key, value) in _config_data.iter(None) {
        // println!("Key: {}, Value: {:?}", key, value);
        _config_options.insert(key.to_string(), value.to_string());
    }

    Ok(RuntimeConfig::new(
        _running_mode,
        _config_dir,
        _config_global_file,
        _config_common_file,
        _config_mode_file,
        _config_platform_file,
        _config_options,
    ))
}

fn get_current_exe() -> Result<PathBuf, RuntimeConfigError> {
    match env::current_exe() {
        Ok(exe_path) => Ok(exe_path),
        Err(e) => {
            return Err(RuntimeConfigError::IoError(e));
        }
    }
}

fn get_process_basename(exe_path: PathBuf) -> Result<String, RuntimeConfigError> {
    if let Some(filename) = exe_path.file_name() {
        if let Some(filename_str) = filename.to_str() {
            // Remove the extension if it exists
            let basename = Path::new(filename_str)
                .file_stem()
                .and_then(|stem| stem.to_str())
                .unwrap_or(filename_str);
            return Ok(basename.to_string());
        }
    }
    Err(RuntimeConfigError::IoError(std::io::Error::new(
        std::io::ErrorKind::Other,
        "Failed to get the process basename",
    )))
}

fn get_process_location(exe_path: PathBuf) -> Result<String, RuntimeConfigError> {
    if let Some(exe_dir) = exe_path.parent() {
        Ok(exe_dir.to_str().map(|s| s.to_owned()).ok_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::Other,
                "Failed to convert executable directory to string",
            )
        })?)
    } else {
        Err(RuntimeConfigError::IoError(std::io::Error::new(
            std::io::ErrorKind::Other,
            "Failed to get the process location",
        )))
    }
}

fn get_running_mode(basename: String) -> Result<String, RuntimeConfigError> {
    let envar: String = format!("RUNNING_MODE_{}", basename.to_uppercase());
    match env::var(&envar) {
        Ok(mode) => Ok(mode),
        Err(env::VarError::NotPresent) => Err(RuntimeConfigError::MissingEnvVar(envar)),
        Err(e) => Err(RuntimeConfigError::VarError(e)),
    }
}
