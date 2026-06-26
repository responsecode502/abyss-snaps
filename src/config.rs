use crate::status::StatusCode;
use anyhow::Result;
use serde::Deserialize;

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

pub fn load_config(path: &str) -> Result<Vec<MountConfig>> {
    let config_data = std::fs::read_to_string(path).map_err(|_| StatusCode::ConfigReadFailed)?;

    // This inline format applies the ? operator and automatically drops Ok(Vec<MountConfig>) down the return line
    Ok(serde_json::from_str(&config_data).map_err(|_| StatusCode::JsonParseFailed)?)
}
