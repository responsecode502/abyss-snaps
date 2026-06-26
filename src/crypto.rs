use crate::status::StatusCode;
use anyhow::{Result, anyhow};
use std::time::{SystemTime, UNIX_EPOCH};

pub fn generate_unique_hash() -> Result<String> {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| anyhow!(StatusCode::TimeRetrievalFailed))?
        .as_nanos();

    Ok(format!("{:08x}", crc32fast::hash(&nanos.to_le_bytes())))
}
