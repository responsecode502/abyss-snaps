use anyhow::{Context, Result, anyhow};
use clap::{Parser, Subcommand};
use nix::unistd::Uid;
use std::collections::hash_map::DefaultHasher;
use std::ffi::CString;
use std::fs::File;
use std::hash::{Hash, Hasher};
use std::os::fd::BorrowedFd;
use std::os::unix::io::AsRawFd;
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
            let nanos = SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)?
                .as_nanos();
            let mut hasher = DefaultHasher::new();
            nanos.hash(&mut hasher);
            let hash_str = format!("{:08x}", hasher.finish())[..8].to_string();
            println!("[*] Hash sequence: {}", hash_str);

            // FIX: Open your actual fstab mount target directory instead of a phantom path
            let parent_dir =
                File::open("/.snapshots").context("Failed to open /.snapshots mount target")?;
            let parent_fd = unsafe { BorrowedFd::borrow_raw(parent_dir.as_raw_fd()) };

            let targets = vec![
                ("/", format!("@-{}", hash_str)),
                ("/home", format!("@home-{}", hash_str)),
            ];

            for (src, name_str) in targets {
                let source_dir = File::open(src).context(format!("Failed to open {}", src))?;
                let source_fd = unsafe { BorrowedFd::borrow_raw(source_dir.as_raw_fd()) };

                let c_name = CString::new(name_str)?;

                btrfs_uapi::subvolume::snapshot_create(parent_fd, source_fd, &c_name, true, &[])
                    .map_err(|e| anyhow!("Kernel snapshot ioctl error on {}: {:?}", src, e))?;

                println!("[+] Snapshot created inside /.snapshots: {:?}", c_name);
            }
            println!("[+] Core baremetal sync finished cleanly.");
        }
    }
    Ok(())
}
