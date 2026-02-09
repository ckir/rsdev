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

/// The default filename for the global configuration file.
const CONFIG_GLOBAL_NAME: &str = "config.global.json";

#[derive(Debug, Error)]
/// # Runtime Configuration Error
///
/// Defines custom error types that can occur during the loading and parsing
/// of system and runtime configuration.
pub enum RuntimeConfigError {
    /// An I/O error occurred, typically when reading configuration files.
    #[error("I/O error occurred: {0}")]
    IoError(#[from] std::io::Error),

    /// A UTF-8 decoding error occurred, for example, when converting bytes to string.
    #[error("UTF-8 error occurred: {0}")]
    Utf8Error(#[from] std::str::Utf8Error),

    /// A command executed during configuration failed with a non-zero exit status.
    #[error("Command failed with non-zero exit status ({status}): {stderr}")]
    ExitStatusError { status: i32, stderr: String },

    /// A general error occurred during command execution.
    #[error("Failed to execute the command: {0}")]
    ExecutionError(String),

    /// An error occurred while accessing environment variables.
    #[error("Environment variable error: {0}")]
    VarError(#[from] env::VarError),

    /// A required environment variable was not found.
    #[error("Environment variable {0} is not present")]
    MissingEnvVar(String),
}

#[derive(Default, Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all(deserialize = "PascalCase"))]
#[serde(rename_all(serialize = "PascalCase"))]
/// # Runtime Configuration
///
/// Holds the application's runtime configuration, detailing how it was loaded
/// and the final resolved configuration options.
pub struct RuntimeConfig {
    /// The detected running mode of the process (e.g., "dev", "prod").
    pub config_running_mode: String,
    /// The base directory where configuration files were searched.
    pub config_dir: String,
    /// The path to the global configuration file, if found.
    pub config_global_file: String,
    /// The path to the common configuration file for this executable, if found.
    pub config_common_file: String,
    /// The path to the mode-specific configuration file, if found.
    pub config_mode_file: String,
    /// The path to the platform-specific configuration file, if found.
    pub config_platform_file: String,
    /// A map of all loaded configuration options (key-value pairs).
    pub config_options: BTreeMap<String, String>,
}

impl RuntimeConfig {
    /// Creates a new `RuntimeConfig` instance with provided details.
    ///
    /// This constructor is typically used internally by `get_runtime_config`
    /// after all configuration files have been processed and options collected.
    ///
    /// # Arguments
    /// * `config_running_mode` - The detected running mode.
    /// * `config_dir` - The directory where configurations were loaded from.
    /// * `config_global_file` - Path to the global config file.
    /// * `config_common_file` - Path to the common config file.
    /// * `config_mode_file` - Path to the mode-specific config file.
    /// * `config_platform_file` - Path to the platform-specific config file.
    /// * `config_options` - A `BTreeMap` of all resolved configuration key-value pairs.
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
    /// Formats the `RuntimeConfig` for display, presenting its various fields
    /// in a human-readable, structured manner.
    ///
    /// This implementation allows `RuntimeConfig` instances to be easily printed
    /// (e.g., with `println!("{}", config);`) for debugging or informational purposes.
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
    /// Retrieves the path of the current executable.
    let _current_exec: PathBuf = get_current_exe()?;
    /// Extracts the basename of the process (executable name without extension).
    let _basename: String = get_process_basename(_current_exec.clone())?.to_owned();
    /// Determines the location of the executable.
    let _location: String = get_process_location(_current_exec.clone())?.to_owned();
    /// Determines the running mode of the application (e.g., "dev", "prod").
    let _running_mode: String = get_running_mode(_basename.clone())?.to_owned();
    /// Determines the configuration directory, prioritizing `CONFIGS_LOCATION` environment variable.
    let _config_dir: String = env::var("CONFIGS_LOCATION").unwrap_or(_location.clone());

    /// Constructs the path to the global configuration file and checks if it exists.
    let _global_file: PathBuf = PathBuf::from(_config_dir.clone()).join(CONFIG_GLOBAL_NAME);
    let mut _config_global_file: String = _global_file.clone().to_string_lossy().to_string();
    if !_global_file.is_file() {
        _config_global_file = "".to_string();
    }

    /// Constructs the path to the common configuration file and checks if it exists.
    let _common_file: PathBuf =
        PathBuf::from(_config_dir.clone()).join(format!("{}.common.json", _basename.clone()));
    let mut _config_common_file: String = _common_file.clone().to_string_lossy().to_string();
    if !_common_file.is_file() {
        _config_common_file = "".to_string();
    }

    /// Constructs the path to the mode-specific configuration file and checks if it exists.
    let _mode_file: PathBuf = PathBuf::from(_config_dir.clone()).join(format!(
        "{}.{}.json",
        _basename.clone(),
        _running_mode.clone()
    ));
    let mut _config_mode_file: String = _mode_file.clone().to_string_lossy().to_string();
    if !_mode_file.is_file() {
        _config_mode_file = "".to_string();
    }

    /// Constructs the path to the platform-specific configuration file and checks if it exists.
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

    /// Builds the configuration by layering global, common, mode-specific, and platform-specific JSON files.
    let _config_data: Box<dyn ConfigurationRoot> = DefaultConfigurationBuilder::new()
        // .add_env_vars() // Environment variables are typically added at a higher level, but can be added here.
        .add_json_file(&_config_global_file.is().optional())
        .add_json_file(&_config_common_file.is().optional())
        .add_json_file(&_config_mode_file.is().optional())
        .add_json_file(&_config_platform_file.is().optional())
        .build()
        .unwrap();
    /// Extracts all key-value pairs from the loaded configuration into a `BTreeMap`.
    let mut _config_options: BTreeMap<String, String> = BTreeMap::new();
    for (key, value) in _config_data.iter(None) {
        // println!("Key: {}, Value: {:?}", key, value);
        _config_options.insert(key.to_string(), value.to_string());
    }

    /// Creates and returns a new `RuntimeConfig` instance with all collected information.
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

/// # Get Current Executable Path
///
/// Retrieves the full path to the current running executable.
///
/// # Returns
/// A `Result<PathBuf, RuntimeConfigError>` containing the `PathBuf` of the executable
/// on success, or a `RuntimeConfigError::IoError` if the path cannot be determined.
fn get_current_exe() -> Result<PathBuf, RuntimeConfigError> {
    match env::current_exe() {
        Ok(exe_path) => Ok(exe_path),
        Err(e) => {
            return Err(RuntimeConfigError::IoError(e));
        }
    }
}

/// # Get Process Basename
///
/// Extracts the base name of the executable (filename without extension) from its full path.
///
/// # Arguments
/// * `exe_path` - The `PathBuf` of the executable.
///
/// # Returns
/// A `Result<String, RuntimeConfigError>` containing the basename as a `String`
/// on success, or a `RuntimeConfigError::IoError` if the basename cannot be determined.
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

/// # Get Process Location
///
/// Retrieves the directory path where the current executable is located.
///
/// # Arguments
/// * `exe_path` - The `PathBuf` of the executable.
///
/// # Returns
/// A `Result<String, RuntimeConfigError>` containing the directory path as a `String`
/// on success, or a `RuntimeConfigError::IoError` if the location cannot be determined.
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

/// # Get Running Mode
///
/// Determines the running mode of the application by checking an environment variable
/// named `RUNNING_MODE_<BASENAME>`, where `<BASENAME>` is the uppercase basename
/// of the executable.
///
/// This allows for environment-specific configuration loading (e.g., "dev", "prod").
///
/// # Arguments
/// * `basename` - The base name of the executable (e.g., "restream").
///
/// # Returns
/// A `Result<String, RuntimeConfigError>` containing the running mode as a `String`
/// on success, or a `RuntimeConfigError` if the environment variable is not set
/// or other environment variable errors occur.
fn get_running_mode(basename: String) -> Result<String, RuntimeConfigError> {
    let envar: String = format!("RUNNING_MODE_{}", basename.to_uppercase());
    match env::var(&envar) {
        Ok(mode) => Ok(mode),
        Err(env::VarError::NotPresent) => Err(RuntimeConfigError::MissingEnvVar(envar)),
        Err(e) => Err(RuntimeConfigError::VarError(e)),
    }
}
