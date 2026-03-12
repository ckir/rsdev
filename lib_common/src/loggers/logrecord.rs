//! # Log Record Definition
//!
//! Defines the structured log record format (`Logrecord`) used across the 
//! entire rsdev ecosystem. This structure is designed for compatibility with 
//! PostgreSQL (via `ToSql`/`FromSql`) and JSON serialization.

use chrono::{DateTime, Utc};
use postgres_types::{FromSql, ToSql};
use serde_derive::Deserialize;
use serde_derive::Serialize;
use serde_json::Value;
use static_init::dynamic;
use crate::utils::misc::sys_info::{ProcessInfo, ProcessInfoError, get_process_info};
use crate::utils::misc::utils::current_datetime_rfc9557;

/// Global static storage for current process information.
#[dynamic]
pub static PROCESSINFO: Result<ProcessInfo, ProcessInfoError> = get_process_info();

/// A complete, structured log record.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSql, FromSql)]
pub struct Logrecord {
    /// Optional database identifier.
    pub id: Option<i64>,
    /// Timestamp of the log entry.
    pub ts: Option<DateTime<Utc>>,
    /// Integer representation of the log level.
    pub loglevel: i64,
    /// Message content and metadata.
    pub message: Message,
    /// Information about the originating application.
    pub app: App,
    /// Information about the originating host.
    pub host: Host,
    /// Information about the user context.
    pub user: User,
    /// Detailed error information if applicable.
    pub error: Error,
    /// Web browser metadata if applicable.
    pub browser: Browser,
    /// Voice synthesis metadata for audible alerts.
    pub voice: Voice,
    /// Sound playback metadata for audible alerts.
    pub sound: Sound,
    /// Arbitrary tags for filtering and categorization.
    pub tags: Value,
    /// RFC9557 formatted timestamp string.
    pub rfc9557: String,
}

impl Default for Logrecord {
    fn default() -> Self {
        let rfc9557: String = current_datetime_rfc9557();

        Self {
            id: None,
            ts: None,
            loglevel: 0,
            app: App::default(),
            host: Host::default(),
            user: User::default(),
            message: Message::default(),
            error: Error::default(),
            browser: Browser::default(),
            voice: Voice::default(),
            sound: Sound::default(),
            tags: serde_json::json!([]),
            rfc9557,
        }
    }
}

/// Represents a log message.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSql, FromSql)]
pub struct Message {
    /// ISO language code (e.g., "en").
    pub lang: String,
    /// The actual log message text.
    pub text: String,
}

impl Default for Message {
    fn default() -> Self {
        Self {
            text: "".to_string(),
            lang: "en".to_string(),
        }
    }
}

/// Represents the originating application metadata.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSql, FromSql)]
pub struct App {
    /// The process identifier (PID).
    pub pid: i64,
    /// The name of the process.
    pub name: String,
}

impl Default for App {
    fn default() -> Self {
        let pid = std::process::id() as i64;
        // // Safely extract process name or default to empty
        let name = PROCESSINFO
            .as_ref()
            .map(|i| i.process_basename.clone())
            .unwrap_or_default();
        Self { name, pid }
    }
}

/// Represents the host system metadata.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSql, FromSql)]
pub struct Host {
    /// The IP address of the host.
    pub ip: String,
    /// The hostname.
    pub name: String,
}

impl Default for Host {
    fn default() -> Self {
        // // Safely extract host info or default to empty
        let name = PROCESSINFO
            .as_ref()
            .map(|i| i.process_host.clone())
            .unwrap_or_default();
        let ip = PROCESSINFO
            .as_ref()
            .map(|i| i.process_host_ip.clone())
            .unwrap_or_default();
        Self { name, ip }
    }
}

/// Represents the user context for the log.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSql, FromSql)]
pub struct User {
    /// Additional user information.
    pub info: String,
    /// The username or user identifier.
    pub name: String,
}

impl Default for User {
    fn default() -> Self {
        // // Safely extract user info or default to empty
        let name = PROCESSINFO
            .as_ref()
            .map(|i| i.process_uid.clone())
            .unwrap_or_default();
        let info = PROCESSINFO
            .as_ref()
            .map(|i| i.process_user.clone())
            .unwrap_or_default();
        Self { name, info }
    }
}

/// Represents detailed error information.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSql, FromSql)]
pub struct Error {
    /// Error code or identifier.
    pub code: String,
    /// Stack trace if available.
    pub stack: String,
    /// Human-readable error details.
    pub details: String,
}

impl Default for Error {
    fn default() -> Self {
        Self {
            code: "".to_string(),
            details: "".to_string(),
            stack: "".to_string(),
        }
    }
}

/// Metadata related to a web browser context.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSql, FromSql)]
pub struct Browser {
    /// Browser name.
    pub name: String,
    /// Browser version.
    pub version: String,
}

impl Default for Browser {
    fn default() -> Self {
        Self {
            name: "".to_string(),
            version: "".to_string(),
        }
    }
}

/// Voice synthesis configuration for audible notifications.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSql, FromSql)]
pub struct Voice {
    /// Cron expression for scheduled alerts.
    pub cron: String,
    /// Number of times to repeat the voice alert.
    pub repeat: i64,
    /// Interval in seconds between repetitions.
    pub interval: i64,
    /// Additional options for the TTS engine.
    pub voptions: Value,
}

impl Default for Voice {
    fn default() -> Self {
        Self {
            cron: "".to_string(),
            repeat: 1,
            interval: 0,
            voptions: serde_json::json!(["-a", "50", "-s", "130", "-p", "80", "-v", "mb-us1"]),
        }
    }
}

/// Sound playback configuration for alerts.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSql, FromSql)]
pub struct Sound {
    /// Path to the sound file.
    pub soundfile: String,
    /// Playback options (sample rate, bit depth, channels).
    pub options: Value,
}

impl Default for Sound {
    fn default() -> Self {
        Self {
            soundfile: "".to_string(),
            options: serde_json::json!(["-r", "44100", "-b", "16", "-c", "1"]),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_logrecord_default() {
        let log = Logrecord::default();
        assert_eq!(log.loglevel, 0);
        assert_eq!(log.message.lang, "en");
        assert!(!log.rfc9557.is_empty());
    }

    #[test]
    fn test_serialization_cycle() {
        let log = Logrecord::default();
        let serialized = serde_json::to_string(&log).unwrap();
        let deserialized: Logrecord = serde_json::from_str(&serialized).unwrap();
        assert_eq!(log, deserialized);
    }

    #[test]
    fn test_voice_default_options() {
        let voice = Voice::default();
        assert!(voice.voptions.is_array());
        assert_eq!(voice.repeat, 1);
    }
}
