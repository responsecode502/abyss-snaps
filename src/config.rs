use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Clone)]
pub struct MountConfig {
    pub device: String,
    pub mountpoint: String,
    pub fstype: String,
    pub options: Vec<String>,
    pub subvol: Option<String>,
    pub dump: u8,
    pub pass: u8,
    pub is_dynamic: bool,
}

#[derive(Serialize, Clone, Copy)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ConfigErrorType {
    ConfigReadFailed,
    JsonParseFailed,
}

#[derive(Serialize)]
struct JsonErrorPayload {
    pub error_type: ConfigErrorType,
    pub message: &'static str,
}

pub fn load_config(path: &str) -> Result<Vec<MountConfig>> {
    let config_data = std::fs::read_to_string(path).with_context(|| {
        serde_json::to_string(&JsonErrorPayload {
            error_type: ConfigErrorType::ConfigReadFailed,
            message: "Config read failed",
        })
        .unwrap() // UNWRAP: Infallible due to static schema string
    })?;

    serde_json::from_str(&config_data).with_context(|| {
        serde_json::to_string(&JsonErrorPayload {
            error_type: ConfigErrorType::JsonParseFailed,
            message: "Wrong json schema",
        })
        .unwrap() // UNWRAP: Infallible due to static schema string
    })
}
