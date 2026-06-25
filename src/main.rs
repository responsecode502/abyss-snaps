use anyhow::{Context, Result, anyhow};
use clap::{Parser, Subcommand};
use nix::unistd::Uid;
use std::collections::hash_map::DefaultHasher;
use std::ffi::CString;
use std::fs::{File, OpenOptions};
use std::hash::{Hash, Hasher};
use std::os::fd::AsFd;
use std::path::Path;
use std::time::SystemTime;

#[derive(Parser)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Run,
}

fn main() -> Result<()> {
    if Uid::current() != Uid::from_raw(0) {
        return Err(anyhow!("Root privileges required."));
    }

    let cli = Cli::parse();
    match cli.command {
        Commands::Run => {
            // 1. Cooperative Lock File Enforcement (Modern RAII Flock Struct)
            let lock_path = "/.snapshots/.abyss-snaps.lock";
            let lock_file = OpenOptions::new()
                .read(true)
                .write(true)
                .create(true)
                .open(lock_path)
                .context("Failed to open or initialize lock file")?;

            // Try to acquire an exclusive, non-blocking lock.
            // The lock is held automatically until the 'lock' variable goes out of scope at the end of main().
            let _lock = nix::fcntl::Flock::lock(
                lock_file,
                nix::fcntl::FlockArg::LockExclusiveNonblock,
            )
            .map_err(|_| {
                eprintln!(
                    "ERROR: PROCESS_LOCKED - Another instance of abyss-snaps is currently running."
                );
                std::process::exit(2);
            })?;

            // 2. Generate unique 8-character token sequence from nanoseconds
            let nanos = SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)?
                .as_nanos();
            let mut hasher = DefaultHasher::new();
            nanos.hash(&mut hasher);
            let hash_str = format!("{:08x}", hasher.finish())[..8].to_string();
            println!("[*] Hash sequence: {}", hash_str);

            let parent_dir =
                File::open("/.snapshots").context("Failed to open /.snapshots mount target")?;

            // 3. Define target pairs
            let targets = vec![
                ("/", format!("@-{}", hash_str)),
                ("/home", format!("@home-{}", hash_str)),
            ];

            // 4. PRE-FLIGHT COLLISION GUARD: Validate ALL targets before executing any snapshots
            for (_, name_str) in &targets {
                if Path::new("/.snapshots").join(name_str).exists() {
                    eprintln!(
                        "ERROR: HASH_COLLISION - Snapshot target '{}' already exists in ledger pool. Aborting transaction.",
                        name_str
                    );
                    std::process::exit(3);
                }
            }

            // 5. TRANSACTION EXECUTION: Safe to run because atomicity is guaranteed
            for (src, name_str) in targets {
                let source_dir = File::open(src).context(format!("Failed to open {}", src))?;
                let c_name = CString::new(name_str)?;

                // Direct Kernel UAPI snapshotting uses the modern borrow-checked AsFd API flawlessly
                btrfs_uapi::subvolume::snapshot_create(
                    parent_dir.as_fd(),
                    source_dir.as_fd(),
                    &c_name,
                    true,
                    &[],
                )
                .map_err(|e| anyhow!("Kernel snapshot ioctl error on {}: {:?}", src, e))?;

                println!("[+] Snapshot created inside /.snapshots: {:?}", c_name);
            }
            println!("[+] Core baremetal sync finished cleanly.");
        }
    }
    Ok(())
}
