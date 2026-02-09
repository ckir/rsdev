// #![allow(dead_code)]
// #![allow(unused_variables)]
// #![allow(unused_imports)]
// #![allow(unreachable_code)]

use std::path::{Path, PathBuf};
use std::process::Command;
use std::result::Result;
use std::{env, fmt};

use serde::{Deserialize, Serialize};

use hostname::get;

use local_ip_address::local_ip;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum ProcessInfoError {
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
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProcessInfo {
    pub process_current_exe: String,
    pub process_basename: String,
    pub process_location: String,
    pub process_pid: i64,
    pub process_uid: String,
    pub process_user: String,
    pub process_host: String,
    pub process_host_ip: String,
}

impl ProcessInfo {
    pub fn new(
        _current_exec: String,
        _basename: String,
        _location: String,
        _pid: i64,
        _user: (String, String),
        _host: (String, String),
    ) -> Self {
        Self {
            process_current_exe: _current_exec,
            process_basename: _basename,
            process_location: _location,
            process_pid: _pid,
            process_uid: _user.0,
            process_user: _user.1,
            process_host: _host.0,
            process_host_ip: _host.1,
        }
    }
}

impl fmt::Display for ProcessInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "ProcessInfo
    Current exe: {},
    Basename: {},
    Location: {},
    Pid: {},
    User id: {},
    User name: {},
    Host: {},
    Host ip: {}
",
            self.process_current_exe,
            self.process_basename,
            self.process_location,
            self.process_pid,
            self.process_uid,
            self.process_user,
            self.process_host,
            self.process_host_ip,
        )
    }
}

pub fn get_process_info() -> Result<ProcessInfo, ProcessInfoError> {
    let _current_exec: PathBuf = get_current_exe()?;
    let _basename: String = get_process_basename(_current_exec.clone())?.to_owned();
    let _location: String = get_process_location(_current_exec.clone())?.to_owned();
    let _pid: i64 = std::process::id() as i64;
    let _user: (String, String) = get_process_user()?;
    let _host: (String, String) = get_process_host()?;

    Ok(ProcessInfo::new(
        _current_exec.to_string_lossy().into_owned(),
        _basename,
        _location,
        _pid,
        _user,
        _host,
    ))
}

fn get_current_exe() -> Result<PathBuf, ProcessInfoError> {
    match env::current_exe() {
        Ok(exe_path) => Ok(exe_path),
        Err(e) => {
            return Err(ProcessInfoError::IoError(e));
        }
    }
}

fn get_process_basename(exe_path: PathBuf) -> Result<String, ProcessInfoError> {
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
    Err(ProcessInfoError::IoError(std::io::Error::new(
        std::io::ErrorKind::Other,
        "Failed to get the process basename",
    )))
}

fn get_process_location(exe_path: PathBuf) -> Result<String, ProcessInfoError> {
    if let Some(exe_dir) = exe_path.parent() {
        Ok(exe_dir.to_str().map(|s| s.to_owned()).ok_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::Other,
                "Failed to convert executable directory to string",
            )
        })?)
    } else {
        Err(ProcessInfoError::IoError(std::io::Error::new(
            std::io::ErrorKind::Other,
            "Failed to get the process location",
        )))
    }
}

fn get_process_user() -> Result<(String, String), ProcessInfoError> {
    // Get user name
    let user_name: String = match Command::new("whoami").output() {
        Ok(output) => {
            if output.status.success() {
                let output_str: &str = std::str::from_utf8(&output.stdout)?;
                output_str.trim().to_string()
            } else {
                return Err(ProcessInfoError::ExitStatusError {
                    status: output.status.code().unwrap_or(-1),
                    stderr: std::str::from_utf8(&output.stderr)?.trim().to_string(),
                });
            }
        }
        Err(e) => {
            return Err(ProcessInfoError::ExecutionError(e.to_string()));
        }
    };

    // Get user ID
    let mut program: &str = "id";
    let mut parameter: &str = "-u";
    if cfg!(target_os = "windows") {
        program = "whoami";
        parameter = "/user";
    }
    let user_id: String = match Command::new(program).arg(parameter).output() {
        Ok(output) => {
            if output.status.success() {
                let output_str: &str = std::str::from_utf8(&output.stdout)?;
                if cfg!(target_os = "windows") {
                    // The user ID is the last token in the output
                    output_str.split_whitespace().last().unwrap().to_string()
                } else {
                    output_str.trim().to_string()
                }
            } else {
                return Err(ProcessInfoError::ExitStatusError {
                    status: output.status.code().unwrap_or(-1),
                    stderr: std::str::from_utf8(&output.stderr)?.trim().to_string(),
                });
            }
        }
        Err(e) => {
            return Err(ProcessInfoError::ExecutionError(e.to_string()));
        }
    };

    Ok((user_id, user_name))
}

fn get_process_host() -> Result<(String, String), ProcessInfoError> {
    let host_name: String = match get() {
        Ok(name) => name.to_string_lossy().into_owned(),
        Err(e) => {
            return Err(ProcessInfoError::IoError(std::io::Error::new(
                std::io::ErrorKind::Other,
                e.to_string(),
            )));
        }
    };
    let host_ip: String = match local_ip() {
        Ok(ip) => ip.to_string(),
        Err(e) => {
            return Err(ProcessInfoError::IoError(std::io::Error::new(
                std::io::ErrorKind::Other,
                e.to_string(),
            )));
        }
    };

    Ok((host_name, host_ip))
}
