use chrono::{DateTime, Utc};
use postgres_types::{FromSql, ToSql};
use serde_derive::Deserialize;
use serde_derive::Serialize;
use serde_json::Value;
use static_init::dynamic;

#[path = "./sys_info.rs"]
mod sys_info;
use sys_info::*;

#[path = "./utils.rs"]
mod utils;
use utils::*;

#[dynamic]
pub static PROCESSINFO: Result<ProcessInfo, ProcessInfoError> = get_process_info();

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSql, FromSql)]
pub struct Logrecord {
    pub id: Option<i64>,
    pub ts: Option<DateTime<Utc>>,
    pub loglevel: i64,
    pub message: Message,
    pub app: App,
    pub host: Host,
    pub user: User,
    pub error: Error,
    pub browser: Browser,
    pub voice: Voice,
    pub sound: Sound,
    pub tags: Value,
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
            rfc9557: rfc9557,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSql, FromSql)]
pub struct Message {
    pub lang: String,
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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSql, FromSql)]
pub struct App {
    pub pid: i64,
    pub name: String,
}
impl Default for App {
    fn default() -> Self {
        let _pid: i64 = std::process::id() as i64;
        let _app_name: String = PROCESSINFO.as_ref().unwrap().process_basename.clone();
        Self {
            name: _app_name,
            pid: _pid,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSql, FromSql)]
pub struct Host {
    pub ip: String,
    pub name: String,
}
impl Default for Host {
    fn default() -> Self {
        let _name: String = PROCESSINFO.as_ref().unwrap().process_host.clone();
        let _ip: String = PROCESSINFO.as_ref().unwrap().process_host_ip.clone();
        Self {
            name: _name,
            ip: _ip,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSql, FromSql)]
pub struct User {
    pub info: String,
    pub name: String,
}
impl Default for User {
    fn default() -> Self {
        let _name = PROCESSINFO.as_ref().unwrap().process_uid.clone();
        let _info = PROCESSINFO.as_ref().unwrap().process_user.clone();
        Self {
            name: _name,
            info: _info,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSql, FromSql)]
pub struct Error {
    pub code: String,
    pub stack: String,
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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSql, FromSql)]
pub struct Browser {
    pub name: String,
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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSql, FromSql)]
pub struct Voice {
    pub cron: String,
    pub repeat: i64,
    pub interval: i64,
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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSql, FromSql)]
pub struct Sound {
    pub soundfile: String,
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
