use anyhow::{Context, Result};
use serde::Serialize;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Serialize, Clone, Copy)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CryptoErrorType {
    TimeRetrievalFailed,
}

#[derive(Serialize)]
struct JsonErrorPayload {
    pub error_type: CryptoErrorType,
    pub message: &'static str,
}

pub fn generate_unique_hash() -> Result<String> {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .with_context(|| {
            serde_json::to_string(&JsonErrorPayload {
                error_type: CryptoErrorType::TimeRetrievalFailed,
                message: "System time backwards",
            })
            .unwrap() // UNWRAP: Infallible due to static schema string
        })?
        .as_nanos();

    Ok(format!("{:08x}", crc32fast::hash(&nanos.to_le_bytes())))
}
