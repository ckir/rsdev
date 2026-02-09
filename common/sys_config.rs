use crate::{ProcessInfo, PROCESSINFO};
use config::{ext::*, *};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::path::PathBuf;

#[derive(Default, Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all(deserialize = "PascalCase"))]
#[serde(rename_all(serialize = "PascalCase"))]
pub struct RuntimeConfig {
    pub process_config: BTreeMap<String, String>,
}

impl RuntimeConfig {
    pub fn new() -> Self {
        let process_info: &ProcessInfo = &*PROCESSINFO;

        let executable_path: String = process_info.process_location.clone();
        let configs_location: String =
            env::var("CONFIGS_LOCATION").unwrap_or(executable_path.clone());
        eprintln!("Config files Location: {}", configs_location.clone());

        let running_mode: String = process_info.process_running_mode.clone();
        let executable_name: String = process_info.process_basename.clone();

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

        let config_data: Box<dyn ConfigurationRoot> = DefaultConfigurationBuilder::new()
            .add_env_vars()
            .add_json_file(&config_common.is().optional())
            .add_json_file(&config_mode.is().optional())
            .add_json_file(&config_platform.is().optional())
            .build()
            .unwrap();

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
