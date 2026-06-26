use crate::config::MountConfig;
use crate::status::StatusCode;
use anyhow::{Result, anyhow};
use std::path::Path;

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

    std::fs::write(&target_fstab_path, fstab_content)
        .map_err(|_| anyhow!(StatusCode::FstabWriteFailed))
}
