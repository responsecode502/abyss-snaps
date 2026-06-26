mod config;
mod crypto;
mod fstab;
mod runner;

use anyhow::{Context, Result, anyhow};
use clap::{Parser, Subcommand};
use nix::unistd::Uid;
use serde::Serialize;
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

#[derive(Serialize, Clone, Copy)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum MainErrorType {
    RootRequired,
    LockFileOpenFailed,
    ProcessLocked,
    CryptoGenerationFailed,
    SnapshotFreezeFailed,
}

#[derive(Serialize, Clone, Copy)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum MainSuccessType {
    SequenceFinished,
}

#[derive(Serialize)]
struct JsonErrorPayload {
    pub error_type: MainErrorType,
    pub message: &'static str,
}

#[derive(Serialize)]
struct JsonSuccessPayload {
    pub event: MainSuccessType,
    pub hash: String,
    pub message: &'static str,
}

fn main() -> ExitCode {
    match run_app() {
        Ok(()) => ExitCode::from(0),
        Err(err) => {
            eprintln!("{err}");
            ExitCode::from(1)
        }
    }
}

fn run_app() -> Result<()> {
    if !Uid::current().is_root() {
        return Err(anyhow!(
            serde_json::to_string(&JsonErrorPayload {
                error_type: MainErrorType::RootRequired,
                message: "Root required",
            })
            .unwrap() // UNWRAP: Infallible due to static schema string
        ));
    }

    let cli = Cli::parse();
    match cli.command {
        Commands::Run => {
            let lock_file = OpenOptions::new()
                .read(true)
                .write(true)
                .create(true)
                .open("/mnt/btrfs-root/@snapshots/.abyss-snaps.lock")
                .with_context(|| {
                    serde_json::to_string(&JsonErrorPayload {
                        error_type: MainErrorType::LockFileOpenFailed,
                        message: "Lock initialization failed",
                    })
                    .unwrap() // UNWRAP: Infallible due to static schema string
                })?;

            let _lock =
                nix::fcntl::Flock::lock(lock_file, nix::fcntl::FlockArg::LockExclusiveNonblock)
                    .map_err(|_| {
                        anyhow!(
                            serde_json::to_string(&JsonErrorPayload {
                                error_type: MainErrorType::ProcessLocked,
                                message: "Instance locked",
                            })
                            .unwrap() // UNWRAP: Infallible due to static schema string
                        )
                    })?;

            let config = config::load_config("/mnt/btrfs-root/@snapshots/abyss-snaps.json")?;

            let hash_str = crypto::generate_unique_hash().with_context(|| {
                serde_json::to_string(&JsonErrorPayload {
                    error_type: MainErrorType::CryptoGenerationFailed,
                    message: "Hash math failed",
                })
                .unwrap() // UNWRAP: Infallible due to static schema string
            })?;

            let targets = runner::create_snapshots(&config, &hash_str)?;

            if let Some((_, root_snap_name)) = targets.iter().find(|(mnt, _)| mnt == "/") {
                let root_snap_path = Path::new("/mnt/btrfs-root/@snapshots").join(root_snap_name);

                fstab::generate_and_write_fstab(&config, &root_snap_path, &hash_str)?;

                runner::set_read_only(&root_snap_path, true).with_context(|| {
                    serde_json::to_string(&JsonErrorPayload {
                        error_type: MainErrorType::SnapshotFreezeFailed,
                        message: "Tree freeze failed",
                    })
                    .unwrap() // UNWRAP: Infallible due to static schema string
                })?;
            }

            let log_line = serde_json::to_string(&JsonSuccessPayload {
                event: MainSuccessType::SequenceFinished,
                hash: hash_str,
                message: "Sequence finished",
            })
            .unwrap(); // UNWRAP: Infallible due to static schema string

            println!("{log_line}");
        }
    }
    Ok(())
}
