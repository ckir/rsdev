//! # System Information Utility
//!
//! Provides detailed information about the current process, including executable path,
//! process ID, user information, and host system details.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::result::Result;
use std::{env, fmt};

use hostname::get;
use local_ip_address::local_ip;
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Errors that can occur while retrieving process or system information.
#[derive(Debug, Error)]
pub enum ProcessInfoError {
    /// Errors related to file system operations.
    #[error("I/O error occurred: {0}")]
    IoError(#[from] std::io::Error),

    /// Errors related to UTF-8 string conversions.
    #[error("UTF-8 error occurred: {0}")]
    Utf8Error(#[from] std::str::Utf8Error),

    /// Errors resulting from a command execution failure.
    #[error("Command failed with non-zero exit status ({status}): {stderr}")]
    ExitStatusError {
        /// The exit status code.
        status: i32,
        /// The error message from stderr.
        stderr: String,
    },

    /// Errors occurring when a command fails to execute.
    #[error("Failed to execute the command: {0}")]
    ExecutionError(String),

    /// Errors related to standard environment variable access.
    #[error("Environment variable error: {0}")]
    VarError(#[from] env::VarError),
}

/// A container for comprehensive process and system metadata.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProcessInfo {
    /// The absolute path to the current executable.
    pub process_current_exe: String,
    /// The stem (basename) of the executable file.
    pub process_basename: String,
    /// The directory containing the executable.
    pub process_location: String,
    /// The system process identifier.
    pub process_pid: i64,
    /// The unique identifier of the user running the process.
    pub process_uid: String,
    /// The human-readable name of the user.
    pub process_user: String,
    /// The network hostname of the system.
    pub process_host: String,
    /// The primary local IP address of the system.
    pub process_host_ip: String,
}

impl ProcessInfo {
    /// Creates a new `ProcessInfo` instance.
    pub fn new(
        current_exec: String,
        basename: String,
        location: String,
        pid: i64,
        user: (String, String),
        host: (String, String),
    ) -> Self {
        Self {
            process_current_exe: current_exec,
            process_basename: basename,
            process_location: location,
            process_pid: pid,
            process_uid: user.0,
            process_user: user.1,
            process_host: host.0,
            process_host_ip: host.1,
        }
    }
}

impl fmt::Display for ProcessInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "ProcessInfo\n    Current exe: {},\n    Basename: {},\n    Location: {},\n    Pid: {},\n    User id: {},\n    User name: {},\n    Host: {},\n    Host ip: {}\n",
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

/// Retrieves all available process and system information.
///
/// # Errors
/// Returns a `ProcessInfoError` if any piece of information cannot be retrieved.
pub fn get_process_info() -> Result<ProcessInfo, ProcessInfoError> {
    // // Resolve executable path
    let current_exec: PathBuf = get_current_exe()?;
    // // Resolve process name
    let basename: String = get_process_basename(current_exec.clone())?.to_owned();
    // // Resolve executable location
    let location: String = get_process_location(current_exec.clone())?.to_owned();
    // // Resolve process ID
    let pid: i64 = std::process::id() as i64;
    // // Resolve user metadata
    let user: (String, String) = get_process_user()?;
    // // Resolve host metadata
    let host: (String, String) = get_process_host()?;

    Ok(ProcessInfo::new(
        current_exec.to_string_lossy().into_owned(),
        basename,
        location,
        pid,
        user,
        host,
    ))
}

/// Retrieves the path of the current running executable.
fn get_current_exe() -> Result<PathBuf, ProcessInfoError> {
    match env::current_exe() {
        Ok(exe_path) => Ok(exe_path),
        Err(e) => Err(ProcessInfoError::IoError(e)),
    }
}

/// Extracts the stem (basename) of the process from its executable path.
fn get_process_basename(exe_path: PathBuf) -> Result<String, ProcessInfoError> {
    if let Some(filename) = exe_path.file_name() {
        if let Some(filename_str) = filename.to_str() {
            // // Remove the extension if it exists
            let basename = Path::new(filename_str)
                .file_stem()
                .and_then(|stem| stem.to_str())
                .unwrap_or(filename_str);
            return Ok(basename.to_string());
        }
    }
    Err(ProcessInfoError::IoError(std::io::Error::other(
        "Failed to get the process basename",
    )))
}

/// Retrieves the directory containing the current executable.
fn get_process_location(exe_path: PathBuf) -> Result<String, ProcessInfoError> {
    if let Some(exe_dir) = exe_path.parent() {
        Ok(exe_dir.to_str().map(|s| s.to_owned()).ok_or_else(|| {
            std::io::Error::other("Failed to convert executable directory to string")
        })?)
    } else {
        Err(ProcessInfoError::IoError(std::io::Error::other(
            "Failed to get the process location",
        )))
    }
}

/// Retrieves the current user's ID and name by executing system commands.
fn get_process_user() -> Result<(String, String), ProcessInfoError> {
    // // Get user name via whoami
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

    // // Get user ID (UID on Unix, SID on Windows)
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
                    // // The user ID is the last token in the output
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

/// Retrieves the hostname and local IP address.
fn get_process_host() -> Result<(String, String), ProcessInfoError> {
    let host_name: String = match get() {
        Ok(name) => name.to_string_lossy().into_owned(),
        Err(e) => {
            return Err(ProcessInfoError::IoError(std::io::Error::other(
                e.to_string(),
            )));
        }
    };
    let host_ip: String = match local_ip() {
        Ok(ip) => ip.to_string(),
        Err(e) => {
            return Err(ProcessInfoError::IoError(std::io::Error::other(
                e.to_string(),
            )));
        }
    };

    Ok((host_name, host_ip))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_process_info_display() {
        let info = ProcessInfo::new(
            "/bin/test".into(),
            "test".into(),
            "/bin".into(),
            1234,
            ("uid123".into(), "user123".into()),
            ("host123".into(), "127.0.0.1".into()),
        );
        let output = format!("{}", info);
        assert!(output.contains("Pid: 1234"));
        assert!(output.contains("Basename: test"));
    }

    #[test]
    fn test_get_process_info_basic() {
        // // This should succeed on most systems where the required tools exist
        let result = get_process_info();
        if let Ok(info) = result {
            assert!(info.process_pid > 0);
            assert!(!info.process_user.is_empty());
        }
    }
}
