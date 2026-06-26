use crate::config::MountConfig;
use anyhow::{Context, Result, anyhow};
use serde::Serialize;
use std::ffi::CString;
use std::fs::File;
use std::os::fd::{AsFd, AsRawFd};
use std::path::Path;

#[derive(Serialize, Clone, Copy)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum RunnerErrorType {
    SnapshotsDirOpenFailed,
    SourceDirOpenFailed,
    HashCollisionDetected,
    KernelIoctlFailed,
    PropertySetFailed,
}

#[derive(Serialize, Clone, Copy)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum RunnerSuccessType {
    SnapshotCreated,
}

#[derive(Serialize)]
struct JsonErrorPayload {
    pub error_type: RunnerErrorType,
    pub message: &'static str,
}

#[derive(Serialize)]
struct JsonSuccessPayload<'a> {
    pub event: RunnerSuccessType,
    pub source: &'a str,
    pub name: &'a str,
}

pub fn create_snapshots(config: &[MountConfig], hash_str: &str) -> Result<Vec<(String, String)>> {
    let parent_dir = File::open("/mnt/btrfs-root/@snapshots").with_context(|| {
        serde_json::to_string(&JsonErrorPayload {
            error_type: RunnerErrorType::SnapshotsDirOpenFailed,
            message: "Snapshots storage path missing",
        })
        .unwrap() // UNWRAP: Infallible due to static schema string
    })?;

    let targets: Vec<(String, String)> = config
        .iter()
        .filter(|mount| mount.is_dynamic)
        .filter_map(|mount| {
            mount.subvol.as_ref().map(|sv| {
                let name_str = format!("{hash_str}.{sv}");
                (mount.mountpoint.clone(), name_str)
            })
        })
        .collect();

    if let Some((_, collided_name)) = targets
        .iter()
        .find(|(_, name)| Path::new("/mnt/btrfs-root/@snapshots").join(name).exists())
    {
        return Err(anyhow!(
            serde_json::to_string(&JsonErrorPayload {
                error_type: RunnerErrorType::HashCollisionDetected,
                message: "Snapshot variant already exists",
            })
            .unwrap() // UNWRAP: Infallible due to static schema string
        ));
    }

    targets
        .iter()
        .try_for_each(|(src, name_str)| -> Result<()> {
            let source_dir = File::open(src).with_context(|| {
                serde_json::to_string(&JsonErrorPayload {
                    error_type: RunnerErrorType::SourceDirOpenFailed,
                    message: "Failed to open target path",
                })
                .unwrap() // UNWRAP: Infallible due to static schema string
            })?;

            let c_name = CString::new(name_str.clone())?;

            // INTENTIONAL: Set read-only argument to false to keep snapshot Read-Write
            btrfs_uapi::subvolume::snapshot_create(
                parent_dir.as_fd(),
                source_dir.as_fd(),
                &c_name,
                false,
                &[],
            )
            .map_err(|_| {
                anyhow!(
                    serde_json::to_string(&JsonErrorPayload {
                        error_type: RunnerErrorType::KernelIoctlFailed,
                        message: "Kernel snapshot creation failed",
                    })
                    .unwrap() // UNWRAP: Infallible due to static schema string
                )
            })?;

            let log_line = serde_json::to_string(&JsonSuccessPayload {
                event: RunnerSuccessType::SnapshotCreated,
                source: src,
                name: name_str,
            })
            .unwrap(); // UNWRAP: Infallible due to static schema string

            println!("{}", log_line);
            Ok(())
        })?;

    Ok(targets)
}

pub fn set_read_only(path: &Path, ro: bool) -> Result<()> {
    let subvol_dir = File::open(path).with_context(|| {
        serde_json::to_string(&JsonErrorPayload {
            error_type: RunnerErrorType::PropertySetFailed,
            message: "Failed to open target to change property",
        })
        .unwrap() // UNWRAP: Infallible due to static schema string
    })?;

    let mut flags = get_subvol_flags(subvol_dir.as_raw_fd()).map_err(|_| {
        anyhow!(
            serde_json::to_string(&JsonErrorPayload {
                error_type: RunnerErrorType::PropertySetFailed,
                message: "Failed to get active flags",
            })
            .unwrap() // UNWRAP: Infallible due to static schema string
        )
    })?;

    if ro {
        flags |= btrfs_uapi::raw::BTRFS_SUBVOL_RDONLY as u64;
    } else {
        flags &= !(btrfs_uapi::raw::BTRFS_SUBVOL_RDONLY as u64);
    }

    set_subvol_flags(subvol_dir.as_raw_fd(), flags).map_err(|_| {
        anyhow!(
            serde_json::to_string(&JsonErrorPayload {
                error_type: RunnerErrorType::PropertySetFailed,
                message: "Failed to set active flags",
            })
            .unwrap() // UNWRAP: Infallible due to static schema string
        )
    })?;

    Ok(())
}

fn get_subvol_flags(fd: std::os::fd::RawFd) -> Result<u64, nix::Error> {
    let mut flags: u64 = 0;
    // UNSAFE: low-level get subvol flags
    unsafe {
        nix::ioctl_read!(btrfs_get_flags, btrfs_uapi::raw::BTRFS_IOCTL_MAGIC, 25, u64);
        btrfs_get_flags(fd, &mut flags)?;
    }
    Ok(flags)
}

fn set_subvol_flags(fd: std::os::fd::RawFd, flags: u64) -> Result<(), nix::Error> {
    // UNSAFE: low-level set subvol flags
    unsafe {
        nix::ioctl_write_ptr!(btrfs_set_flags, btrfs_uapi::raw::BTRFS_IOCTL_MAGIC, 26, u64);
        btrfs_set_flags(fd, &flags)?;
    }
    Ok(())
}
