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
/// # Process Info Error
///
/// Defines custom error types that can occur during the retrieval of process
/// and system information.
pub enum ProcessInfoError {
    /// An I/O error occurred, typically when accessing file system or executing commands.
    #[error("I/O error occurred: {0}")]
    IoError(#[from] std::io::Error),

    /// A UTF-8 decoding error occurred, for example, when converting command output to string.
    #[error("UTF-8 error occurred: {0}")]
    Utf8Error(#[from] std::str::Utf8Error),

    /// A command failed with a non-zero exit status.
    #[error("Command failed with non-zero exit status ({status}): {stderr}")]
    ExitStatusError { status: i32, stderr: String },

    /// A general error occurred during command execution.
    #[error("Failed to execute the command: {0}")]
    ExecutionError(String),

    /// An error occurred while accessing environment variables.
    #[error("Environment variable error: {0}")]
    VarError(#[from] env::VarError),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
/// # Process Information
///
/// Holds various pieces of information about the current running process and its environment.
pub struct ProcessInfo {
    /// The full path to the current executable.
    pub process_current_exe: String,
    /// The base name of the executable (filename without extension).
    pub process_basename: String,
    /// The directory where the executable is located.
    pub process_location: String,
    /// The process ID (PID) of the current process.
    pub process_pid: i64,
    /// The user ID (UID) of the user running the process.
    pub process_uid: String,
    /// The user name of the user running the process.
    pub process_user: String,
    /// The hostname of the machine running the process.
    pub process_host: String,
    /// The local IP address of the machine running the process.
    pub process_host_ip: String,
}

impl ProcessInfo {
    /// Creates a new `ProcessInfo` instance with provided details.
    ///
    /// This constructor is typically used internally by `get_process_info`
    /// after all process and system information has been collected.
    ///
    /// # Arguments
    /// * `_current_exec` - The full path to the current executable.
    /// * `_basename` - The base name of the executable.
    /// * `_location` - The directory where the executable is located.
    /// * `_pid` - The process ID.
    /// * `_user` - A tuple containing the user ID and user name.
    /// * `_host` - A tuple containing the hostname and host IP address.
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
    /// Formats the `ProcessInfo` for display, presenting its various fields
    /// in a human-readable, structured manner.
    ///
    /// This implementation allows `ProcessInfo` instances to be easily printed
    /// (e.g., with `println!("{}", info);`) for debugging or informational purposes.
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

/// # Get Process Information
///
/// Collects and returns comprehensive information about the current running process.
///
/// This function aggregates data from various helper functions:
/// - `get_current_exe()` for the executable path.
/// - `get_process_basename()` for the executable's base name.
/// - `get_process_location()` for the executable's directory.
/// - `std::process::id()` for the process ID.
/// - `get_process_user()` for user ID and name.
/// - `get_process_host()` for hostname and IP address.
///
/// # Returns
/// A `Result<ProcessInfo, ProcessInfoError>` containing a `ProcessInfo` struct
/// on success, or an error if any piece of information cannot be retrieved.
pub fn get_process_info() -> Result<ProcessInfo, ProcessInfoError> {
    /// Retrieves the full path to the current executable.
    let _current_exec: PathBuf = get_current_exe()?;
    /// Extracts the base name of the process.
    let _basename: String = get_process_basename(_current_exec.clone())?.to_owned();
    /// Determines the location of the process executable.
    let _location: String = get_process_location(_current_exec.clone())?.to_owned();
    /// Gets the process ID.
    let _pid: i64 = std::process::id() as i64;
    /// Retrieves information about the user running the process.
    let _user: (String, String) = get_process_user()?;
    /// Retrieves information about the host machine.
    let _host: (String, String) = get_process_host()?;

    /// Constructs and returns a new `ProcessInfo` instance.
    Ok(ProcessInfo::new(
        _current_exec.to_string_lossy().into_owned(),
        _basename,
        _location,
        _pid,
        _user,
        _host,
    ))
}

/// # Get Current Executable Path
///
/// Retrieves the full path to the current running executable.
///
/// # Returns
/// A `Result<PathBuf, ProcessInfoError>` containing the `PathBuf` of the executable
/// on success, or a `ProcessInfoError::IoError` if the path cannot be determined.
fn get_current_exe() -> Result<PathBuf, ProcessInfoError> {
    match env::current_exe() {
        Ok(exe_path) => Ok(exe_path),
        Err(e) => {
            return Err(ProcessInfoError::IoError(e));
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
/// A `Result<String, ProcessInfoError>` containing the basename as a `String`
/// on success, or a `ProcessInfoError::IoError` if the basename cannot be determined.
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

/// # Get Process Location
///
/// Retrieves the directory path where the current executable is located.
///
/// # Arguments
/// * `exe_path` - The `PathBuf` of the executable.
///
/// # Returns
/// A `Result<String, ProcessInfoError>` containing the directory path as a `String`
/// on success, or a `ProcessInfoError::IoError` if the location cannot be determined.
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

/// # Get Process User
///
/// Retrieves the user ID and username of the user who owns the current process.
///
/// This function uses platform-specific commands (`whoami` on Windows, `id -u` and `whoami` on Linux)
/// to get the relevant user information.
///
/// # Returns
/// A `Result<(String, String), ProcessInfoError>` containing a tuple of (user ID, username)
/// on success, or an error if the commands fail to execute or return unexpected output.
fn get_process_user() -> Result<(String, String), ProcessInfoError> {
    /// Get user name using `whoami`.
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

    /// Determine the command and arguments for getting the user ID based on the OS.
    let mut program: &str = "id";
    let mut parameter: &str = "-u";
    if cfg!(target_os = "windows") {
        program = "whoami";
        parameter = "/user";
    }
    /// Get user ID using platform-specific command.
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

/// # Get Process Host
///
/// Retrieves the hostname and local IP address of the machine running the process.
///
/// It uses the `hostname` crate for the hostname and `local_ip_address` for the IP.
///
/// # Returns
/// A `Result<(String, String), ProcessInfoError>` containing a tuple of (hostname, IP address)
/// on success, or an error if the information cannot be retrieved.
fn get_process_host() -> Result<(String, String), ProcessInfoError> {
    /// Retrieves the hostname of the local machine.
    let host_name: String = match get() {
        Ok(name) => name.to_string_lossy().into_owned(),
        Err(e) => {
            return Err(ProcessInfoError::IoError(std::io::Error::new(
                std::io::ErrorKind::Other,
                e.to_string(),
            )));
        }
    };
    /// Retrieves the local IP address of the machine.
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
