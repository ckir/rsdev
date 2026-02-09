use chrono::{DateTime, Utc};
use postgres_types::{FromSql, ToSql};
use serde_derive::Deserialize;
use serde_derive::Serialize;
use serde_json::Value;
use static_init::dynamic;
use crate::utils::misc::sys_info::{ProcessInfo, ProcessInfoError, get_process_info};
use crate::utils::misc::utils::current_datetime_rfc9557;

#[dynamic]
/// Statically initialized `ProcessInfo` instance, providing details about the current process.
pub static PROCESSINFO: Result<ProcessInfo, ProcessInfoError> = get_process_info();

/// # Logrecord
///
/// Represents a comprehensive log entry, capturing various details about a system event.
/// This struct is designed to be highly detailed for diagnostic and analytical purposes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSql, FromSql)]
pub struct Logrecord {
    /// Unique identifier for the log record. Typically assigned by the database.
    pub id: Option<i64>,
    /// Timestamp (UTC) when the log record was created.
    pub ts: Option<DateTime<Utc>>,
    /// The severity level of the log (e.g., 0 for Trace, 1 for Debug, 2 for Info, etc.).
    pub loglevel: i64,
    /// Details about the message content.
    pub message: Message,
    /// Information about the application generating the log.
    pub app: App,
    /// Information about the host where the log originated.
    pub host: Host,
    /// Information about the user associated with the log event.
    pub user: User,
    /// Details if the log record represents an error.
    pub error: Error,
    /// Information about the browser context, if applicable.
    pub browser: Browser,
    /// Settings for voice alerts associated with the log.
    pub voice: Voice,
    /// Settings for sound alerts associated with the log.
    pub sound: Sound,
    /// Flexible JSON value for arbitrary tags or additional metadata.
    pub tags: Value,
    /// RFC 9557 formatted timestamp string.
    pub rfc9557: String,
}
impl Default for Logrecord {
    /// Creates a default `Logrecord` instance with predefined or empty values.
    ///
    /// Initializes `rfc9557` with the current UTC datetime in RFC 9557 format.
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
            rfc9557: rfc9557,
        }
    }
}

/// # Message
///
/// Represents the textual content of a log entry, including its language.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSql, FromSql)]
pub struct Message {
    /// The language of the message (e.g., "en" for English).
    pub lang: String,
    /// The actual text content of the message.
    pub text: String,
}
impl Default for Message {
    /// Creates a default `Message` instance with an empty text and "en" as language.
    fn default() -> Self {
        Self {
            text: "".to_string(),
            lang: "en".to_string(),
        }
    }
}

/// # App
///
/// Contains information about the application that generated the log entry.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSql, FromSql)]
pub struct App {
    /// The process ID (PID) of the application.
    pub pid: i64,
    /// The name of the application.
    pub name: String,
}
impl Default for App {
    /// Creates a default `App` instance, populating `name` and `pid` from global process information.
    fn default() -> Self {
        let _pid: i64 = std::process::id() as i64;
        let _app_name: String = PROCESSINFO.as_ref().unwrap().process_basename.clone();
        Self {
            name: _app_name,
            pid: _pid,
        }
    }
}

/// # Host
///
/// Contains information about the host machine where the log originated.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSql, FromSql)]
pub struct Host {
    /// The IP address of the host.
    pub ip: String,
    /// The name of the host.
    pub name: String,
}
impl Default for Host {
    /// Creates a default `Host` instance, populating `name` and `ip` from global process information.
    fn default() -> Self {
        let _name: String = PROCESSINFO.as_ref().unwrap().process_host.clone();
        let _ip: String = PROCESSINFO.as_ref().unwrap().process_host_ip.clone();
        Self {
            name: _name,
            ip: _ip,
        }
    }
}

/// # User
///
/// Contains information about the user associated with the log event.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSql, FromSql)]
pub struct User {
    /// Additional information about the user.
    pub info: String,
    /// The name of the user.
    pub name: String,
}
impl Default for User {
    /// Creates a default `User` instance, populating `name` and `info` from global process information.
    fn default() -> Self {
        let _name = PROCESSINFO.as_ref().unwrap().process_uid.clone();
        let _info = PROCESSINFO.as_ref().unwrap().process_user.clone();
        Self {
            name: _name,
            info: _info,
        }
    }
}

/// # Error
///
/// Details pertaining to an error that occurred, if the log entry is error-related.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSql, FromSql)]
pub struct Error {
    /// A specific error code.
    pub code: String,
    /// The stack trace where the error occurred.
    pub stack: String,
    /// Additional details or a descriptive message about the error.
    pub details: String,
}
impl Default for Error {
    /// Creates a default `Error` instance with empty strings for all fields.
    fn default() -> Self {
        Self {
            code: "".to_string(),
            details: "".to_string(),
            stack: "".to_string(),
        }
    }
}

/// # Browser
///
/// Captures information about the web browser, if the log event is web-related.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSql, FromSql)]
pub struct Browser {
    /// The name of the browser (e.g., "Chrome", "Firefox").
    pub name: String,
    /// The version of the browser.
    pub version: String,
}
impl Default for Browser {
    /// Creates a default `Browser` instance with empty strings for name and version.
    fn default() -> Self {
        Self {
            name: "".to_string(),
            version: "".to_string(),
        }
    }
}

/// # Voice
///
/// Configuration for voice alerts or text-to-speech functionality.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSql, FromSql)]
pub struct Voice {
    /// A cron-style schedule string for voice alerts. (Currently unused).
    pub cron: String,
    /// The number of times a voice alert should repeat.
    pub repeat: i64,
    /// The interval in seconds between repetitions of a voice alert.
    pub interval: i64,
    /// A JSON `Value` representing additional options for the voice synthesizer (e.g., `espeak-ng` arguments).
    pub voptions: Value,
}
impl Default for Voice {
    /// Creates a default `Voice` instance with common `espeak-ng` options for a female voice.
    fn default() -> Self {
        Self {
            cron: "".to_string(),
            repeat: 1,
            interval: 0,
            voptions: serde_json::json!(["-a", "50", "-s", "130", "-p", "80", "-v", "mb-us1"]),
        }
    }
}

/// # Sound
///
/// Configuration for sound alerts.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSql, FromSql)]
pub struct Sound {
    /// The path or identifier of the sound file to play.
    pub soundfile: String,
    /// A JSON `Value` representing additional options for playing the sound,
    /// typically command-line arguments for an audio player.
    pub options: Value,
}
impl Default for Sound {
    /// Creates a default `Sound` instance with an empty soundfile and common audio playback options.
    fn default() -> Self {
        Self {
            soundfile: "".to_string(),
            options: serde_json::json!(["-r", "44100", "-b", "16", "-c", "1"]),
        }
    }
}
