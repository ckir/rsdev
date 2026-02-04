use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientMessage {
    pub subscribe: Option<Vec<String>>,
    pub unsubscribe: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ServerMessage {
    pub r#type: String,
    pub message: Option<String>, // Base64 encoded protobuf
    pub error: Option<String>,
}

// #[derive(Debug, Clone)]
// pub enum UpstreamEvent {
//     Open,
//     Close,
//     Error(String),
//     Message(Vec<u8>),
// }
