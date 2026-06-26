use crate::config::MountConfig;
use anyhow::{Context, Result};
use serde::Serialize;
use std::path::Path;

#[derive(Serialize, Clone, Copy)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum FstabErrorType {
    FstabWriteFailed,
}

#[derive(Serialize)]
struct JsonErrorPayload {
    pub error_type: FstabErrorType,
    pub message: &'static str,
}

pub fn generate_and_write_fstab(
    config: &[MountConfig],
    root_snap_path: &Path,
    hash_str: &str,
) -> Result<()> {
    let mut fstab_content = String::from("# Generated automatically by abyss-snaps\n");

    for mount in config {
        let mut opts = mount.options.clone();

        if let Some(sv) = &mount.subvol {
            let payload = if mount.is_dynamic {
                format!("subvol=/@snapshots/{hash_str}.{sv}")
            } else {
                format!("subvol={sv}")
            };
            opts.push(payload);
        }

        // INTENTIONAL: We don`t force 'defaults' for empty strings
        let opts_str = opts.join(",");

        fstab_content.push_str(&format!(
            "{}\t{}\t{}\t{}\t{}\t{}\n",
            mount.device, mount.mountpoint, mount.fstype, opts_str, mount.dump, mount.pass
        ));
    }

    let target_fstab_path = root_snap_path.join("etc/fstab");

    std::fs::write(&target_fstab_path, fstab_content).with_context(|| {
        serde_json::to_string(&JsonErrorPayload {
            error_type: FstabErrorType::FstabWriteFailed,
            message: "Fstab write failed",
        })
        .unwrap() // UNWRAP: Infallible due to static schema string
    })
}
