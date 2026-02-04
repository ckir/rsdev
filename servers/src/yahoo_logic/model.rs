use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientMessage {
    pub subscribe: Option<Vec<String>>,
    pub unsubscribe: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ServerMessage {
    pub r#type: String,
    pub message: Option<Value>,
    pub error: Option<String>,
    pub ack: Option<bool>,
}

// #[derive(Debug, Clone)]
// pub enum UpstreamEvent {
//     Open,
//     Close,
//     Error(String),
//     Message(Vec<u8>),
// }
