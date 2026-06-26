mod config;
mod crypto;
mod fstab;
mod runner;
mod status;

use anyhow::Result;
use clap::{Parser, Subcommand};
use nix::unistd::Uid;
use status::StatusCode;
use std::fs::OpenOptions;
use std::path::Path;
use std::process::ExitCode;

#[derive(Parser)]
#[command(name = "abyss-snaps", version = "0.1.2")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Run,
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    match cli.command {
        Commands::Run => match run_app() {
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

fn run_app() -> Result<()> {
    if !Uid::current().is_root() {
        return Err(StatusCode::RootRequired.into());
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
        .ok_or(StatusCode::RootSnapshotNotFound)?; // Cleaner: Uses direct enum fallback token conversion

    let root_snap_path = Path::new("/mnt/btrfs-root/@snapshots").join(root_snap_name);

    fstab::generate_and_write_fstab(&config, &root_snap_path, &hash_str)?;

    runner::set_read_only(&root_snap_path, true)?;

    status::emit_success_finished(&hash_str);
    Ok(())
}
