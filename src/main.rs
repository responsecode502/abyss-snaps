mod cli;
mod config;
mod crypto;
mod fstab;
mod runner;
mod status;

use anyhow::Result; // FIXED: Removed the unused `, anyhow` import macro
use status::StatusCode;
use std::fs::File;
use std::fs::OpenOptions;
use std::io::{BufRead, BufReader};
use std::path::Path;
use std::process::ExitCode;

fn main() -> ExitCode {
    let args = cli::Cli::parse_args();

    match args.command {
        cli::Commands::Run => match run_sequence() {
            Ok(()) => ExitCode::from(0),
            Err(err) => {
                if let Some(status_code) = err.downcast_ref::<StatusCode>() {
                    status::emit_error(*status_code);
                } else {
                    eprintln!("{err}");
                }
                ExitCode::from(1)
            }
        },
        cli::Commands::Rollback { hash } => match rollback_sequence(&hash) {
            Ok(()) => ExitCode::from(0),
            Err(err) => {
                if let Some(status_code) = err.downcast_ref::<StatusCode>() {
                    status::emit_error(*status_code);
                } else {
                    eprintln!("{err}");
                }
                ExitCode::from(1)
            }
        },
    }
}

fn run_sequence() -> Result<()> {
    if !nix::unistd::Uid::current().is_root() {
        return Err(StatusCode::RootRequired.into());
    }

    let is_sandbox = check_if_snapshot_booted()?;
    if is_sandbox {
        return Err(StatusCode::InvalidBootedSubvolume.into());
    }

    let lock_file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .open("/mnt/btrfs-root/@snapshots/.abyss-snaps.lock")
        .map_err(|_| StatusCode::LockFileOpenFailed)?;

    let _lock = nix::fcntl::Flock::lock(lock_file, nix::fcntl::FlockArg::LockExclusiveNonblock)
        .map_err(|_| StatusCode::ProcessLocked)?;

    let config = config::load_config("/mnt/btrfs-root/@snapshots/abyss-snaps.json")?;
    let hash_str = crypto::generate_unique_hash()?;
    let targets = runner::create_snapshots(&config, &hash_str)?;

    let (_, root_snap_name) = targets
        .iter()
        .find(|(mnt, _)| mnt == "/")
        .ok_or(StatusCode::RootSnapshotNotFound)?;

    let root_snap_path = Path::new("/mnt/btrfs-root/@snapshots").join(root_snap_name);
    fstab::generate_and_write_fstab(&config, &root_snap_path, &hash_str)?;
    runner::set_read_only(&root_snap_path, true)?;

    status::emit_success_finished(&hash_str);
    Ok(())
}

fn rollback_sequence(hash: &str) -> Result<()> {
    if !nix::unistd::Uid::current().is_root() {
        return Err(StatusCode::RootRequired.into());
    }

    let config = config::load_config("/mnt/btrfs-root/@snapshots/abyss-snaps.json")?;

    for mount in &config {
        if mount.is_dynamic {
            if let Some(sv) = &mount.subvol {
                let historical_snapshot_name = format!("{hash}.{sv}");
                runner::restore_snapshot(sv, &historical_snapshot_name)?;
            }
        }
    }

    status::emit_success_rollback(hash);
    Ok(())
}

fn check_if_snapshot_booted() -> Result<bool> {
    let file = File::open("/proc/self/mountinfo").map_err(|_| StatusCode::ConfigReadFailed)?;
    let reader = BufReader::new(file);

    for line_result in reader.lines() {
        let line = line_result.map_err(|_| StatusCode::ConfigReadFailed)?;
        let fields: Vec<&str> = line.split_whitespace().collect();

        if fields.len() > 4 && fields[4] == "/" {
            let root_subvolume_option = fields[3];
            if root_subvolume_option.contains("/@snapshots/") {
                return Ok(true);
            }
        }
    }
    Ok(false)
}
