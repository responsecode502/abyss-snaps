use crate::config::MountConfig;
use crate::status::{self, StatusCode};
use anyhow::{Result, anyhow};
use std::ffi::CString;
use std::fs::File;
use std::os::fd::{AsFd, AsRawFd};
use std::path::Path;

pub fn create_snapshots(config: &[MountConfig], hash_str: &str) -> Result<Vec<(String, String)>> {
    let parent_dir = File::open("/mnt/btrfs-root/@snapshots")
        .map_err(|_| anyhow!(StatusCode::SnapshotsDirOpenFailed))?;

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

    if targets
        .iter()
        .any(|(_, name)| Path::new("/mnt/btrfs-root/@snapshots").join(name).exists())
    {
        return Err(anyhow!(StatusCode::HashCollisionDetected));
    }

    targets
        .iter()
        .try_for_each(|(mountpoint, name_str)| -> Result<()> {
            let source_dir =
                File::open(mountpoint).map_err(|_| anyhow!(StatusCode::SourceDirOpenFailed))?;

            let c_name = CString::new(name_str.clone())?;
            let enforce_ro_on_init = mountpoint != "/";

            btrfs_uapi::subvolume::snapshot_create(
                parent_dir.as_fd(),
                source_dir.as_fd(),
                &c_name,
                enforce_ro_on_init,
                &[],
            )
            .map_err(|_| anyhow!(StatusCode::KernelIoctlFailed))?;

            status::emit_success_snapshot(mountpoint, name_str);
            Ok(())
        })?;

    Ok(targets)
}

pub fn restore_snapshot(production_name: &str, snapshot_name: &str) -> Result<()> {
    let root_dir =
        File::open("/mnt/btrfs-root").map_err(|_| anyhow!(StatusCode::SnapshotsDirOpenFailed))?;

    let source_snap_path = Path::new("/mnt/btrfs-root/@snapshots").join(snapshot_name);
    let source_snap_dir =
        File::open(&source_snap_path).map_err(|_| anyhow!(StatusCode::SourceDirOpenFailed))?;

    let c_production = CString::new(production_name)?;

    // INTENTIONAL: Asynchronously purge production tree structure at kernel layer before hot-swapping pointers
    let _ = btrfs_uapi::subvolume::subvolume_delete(root_dir.as_fd(), &c_production);

    btrfs_uapi::subvolume::snapshot_create(
        root_dir.as_fd(),
        source_snap_dir.as_fd(),
        &c_production,
        false, // Restored active environments must be writeable to process system runtimes
        &[],
    )
    .map_err(|_| anyhow!(StatusCode::KernelIoctlFailed))?;

    Ok(())
}

pub fn set_read_only(path: &Path, ro: bool) -> Result<()> {
    let subvol_dir = File::open(path).map_err(|_| anyhow!(StatusCode::PropertySetFailed))?;

    let mut flags = get_subvol_flags(subvol_dir.as_raw_fd())
        .map_err(|_| anyhow!(StatusCode::PropertySetFailed))?;

    if ro {
        flags |= btrfs_uapi::raw::BTRFS_SUBVOL_RDONLY as u64;
    } else {
        flags &= !(btrfs_uapi::raw::BTRFS_SUBVOL_RDONLY as u64);
    }

    set_subvol_flags(subvol_dir.as_raw_fd(), flags)
        .map_err(|_| anyhow!(StatusCode::PropertySetFailed))?;

    Ok(())
}

fn get_subvol_flags(fd: std::os::fd::RawFd) -> Result<u64, nix::Error> {
    let mut flags: u64 = 0;
    // UNSAFE: low-level get subvol flags via direct request macro mapping
    unsafe {
        nix::ioctl_read!(btrfs_get_flags, btrfs_uapi::raw::BTRFS_IOCTL_MAGIC, 25, u64);
        btrfs_get_flags(fd, &mut flags)?;
    }
    Ok(flags)
}

fn set_subvol_flags(fd: std::os::fd::RawFd, flags: u64) -> Result<(), nix::Error> {
    // UNSAFE: low-level set subvol flags via direct request macro mapping
    unsafe {
        nix::ioctl_write_ptr!(btrfs_set_flags, btrfs_uapi::raw::BTRFS_IOCTL_MAGIC, 26, u64);
        btrfs_set_flags(fd, &flags)?;
    }
    Ok(())
}
