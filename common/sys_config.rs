use crate::{ProcessInfo, PROCESSINFO};
use config::{ext::*, *};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::path::PathBuf;

/// # Runtime Configuration
///
/// Holds the application's runtime configuration, typically loaded from multiple
/// JSON files and environment variables.
#[derive(Default, Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all(deserialize = "PascalCase"))]
#[serde(rename_all(serialize = "PascalCase"))]
pub struct RuntimeConfig {
    /// A map of configuration keys and their corresponding string values.
    /// Keys are expected to be in PascalCase.
    pub process_config: BTreeMap<String, String>,
}

impl RuntimeConfig {
    /// Creates a new `RuntimeConfig` instance by loading configuration from
    /// various sources:
    /// 1.  Environment variables.
    /// 2.  A global configuration file (`global_config.json`).
    /// 3.  A common configuration file specific to the executable (`<executable_name>.common.json`).
    /// 4.  A mode-specific configuration file (`<executable_name>.<running_mode>.json`).
    /// 5.  A platform-specific configuration file (`<executable_name>.<running_mode>.<os>.json`).
    ///
    /// The configuration files are searched for in the directory specified by the
    /// `CONFIGS_LOCATION` environment variable, or defaults to the executable's path.
    ///
    /// Configuration sources are layered, with later sources overriding earlier ones.
    ///
    /// # Returns
    /// A `RuntimeConfig` instance containing the merged configuration.
    pub fn new() -> Self {
        /// Retrieves process information to determine executable name, path, and running mode.
        let process_info: &ProcessInfo = &*PROCESSINFO;

        let executable_path: String = process_info.process_location.clone();
        /// Determines the location for configuration files. Prioritizes `CONFIGS_LOCATION` env var.
        let configs_location: String =
            env::var("CONFIGS_LOCATION").unwrap_or(executable_path.clone());
        eprintln!("Config files Location: {}", configs_location.clone());

        let running_mode: String = process_info.process_running_mode.clone();
        let executable_name: String = process_info.process_basename.clone();

        /// Defines the path for the global configuration file (`global_config.json`).
        let config_global_name: String = "global_config.json".to_string();
        let config_global_file: PathBuf =
            PathBuf::from(configs_location.clone()).join(config_global_name.clone());
        let config_global: String = config_global_file.to_str().unwrap_or_default().to_string();
        if !config_global_file.exists() {
            eprintln!(
                "Config file [Global  ]({:?}): not found",
                config_global.clone()
            );
        } else {
            eprintln!("Config file [Global  ]({:?}): found", config_global.clone());
        }

        /// Defines the path for the common configuration file (`<executable_name>.common.json`).
        let config_common_name: String = format!("{}.common.json", executable_name.clone());
        let config_common_file: PathBuf =
            PathBuf::from(configs_location.clone()).join(config_common_name.clone());
        let config_common: String = config_common_file.to_str().unwrap_or_default().to_string();
        if !config_common_file.exists() {
            eprintln!(
                "Config file [Common  ]({:?}): not found",
                config_common.clone()
            );
        } else {
            eprintln!("Config file [Common  ]({:?}): found", config_common.clone());
        }

        /// Defines the path for the mode-specific configuration file (`<executable_name>.<running_mode>.json`).
        let config_mode_name: String =
            format!("{}.{}.json", executable_name.clone(), running_mode.clone());
        let config_mode_file: PathBuf =
            PathBuf::from(configs_location.clone()).join(config_mode_name.clone());
        let config_mode: String = config_mode_file.to_str().unwrap_or_default().to_string();
        if !config_mode_file.exists() {
            eprintln!("Config file [Mode    ]({:?}): not found", config_mode.clone());
        } else {
            eprintln!("Config file [Mode    ]({:?}): found", config_mode.clone());
        }

        /// Defines the path for the platform-specific configuration file (`<executable_name>.<running_mode>.<os>.json`).
        let config_platform_name: String = format!(
            "{}.{}.{}.json",
            executable_name.clone(),
            running_mode.clone(),
            std::env::consts::OS
        );
        let config_platform_file: PathBuf =
            PathBuf::from(configs_location.clone()).join(config_platform_name.clone());
        let config_platform: String = config_platform_file
            .to_str()
            .unwrap_or_default()
            .to_string();
        if !config_platform_file.exists() {
            eprintln!(
                "Config file [Platform]({:?}): not found",
                config_platform.clone()
            );
        } else {
            eprintln!(
                "Config file [Platform]({:?}): found",
                config_platform.clone()
            );
        }

        /// Builds the final configuration by layering environment variables and JSON files.
        /// Later sources override earlier ones.
        let config_data: Box<dyn ConfigurationRoot> = DefaultConfigurationBuilder::new()
            .add_env_vars()
            .add_json_file(&config_common.is().optional())
            .add_json_file(&config_mode.is().optional())
            .add_json_file(&config_platform.is().optional())
            .build()
            .unwrap();

        /// Iterates through the loaded configuration to extract all key-value pairs into a `BTreeMap`.
        let mut config_keys: BTreeMap<String, String> = BTreeMap::new();
        for (key, value) in config_data.iter(None) {
            // println!("Key: {}, Value: {:?}", key, value);
            config_keys.insert(key.to_string(), value.to_string());
        }
        Self {
            process_config: config_keys,
        }
    }
}

impl std::fmt::Display for RuntimeConfig {
    /// Formats the `RuntimeConfig` for display, listing all loaded configuration key-value pairs.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "Runtime Configuration:")?;
        for (key, value) in &self.process_config {
            writeln!(f, "  {}: {}", key, value)?;
        }
        Ok(())
    }
}
