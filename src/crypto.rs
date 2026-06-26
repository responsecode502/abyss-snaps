use anyhow::Result;
use crc32fast::hash;
use std::time::{SystemTime, UNIX_EPOCH};

pub fn generate_unique_hash() -> Result<String> {
    let nanos = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
    let bytes = nanos.to_le_bytes();
    let checksum = hash(&bytes);
    Ok(format!("{:08x}", checksum))
}
