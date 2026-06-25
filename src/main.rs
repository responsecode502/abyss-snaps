use anyhow::{Context, Result, anyhow};
use clap::{Parser, Subcommand};
use nix::unistd::Uid;
use serde::Deserialize;
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

#[derive(Deserialize, Clone)]
struct MountConfig {
    device: String,
    mountpoint: String,
    fstype: String,
    options: Vec<String>,
    subvol: Option<String>,
    dump: u32,
    pass: u32,
    r#type: String,
}

fn main() -> Result<()> {
    if Uid::current() != Uid::from_raw(0) {
        return Err(anyhow!("Root privileges required."));
    }
    let cli = Cli::parse();
    match cli.command {
        Commands::Run => {
            let lock_path = "/.snapshots/.abyss-snaps.lock";
            let lock_file = OpenOptions::new()
                .read(true)
                .write(true)
                .create(true)
                .open(lock_path)
                .context("Failed to open lock file")?;
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
            let config_path = "/.snapshots/abyss-snaps.json";
            let config_data = std::fs::read_to_string(config_path)
                .context("Failed to read configuration file from /.snapshots/abyss-snaps.json")?;
            let config: Vec<MountConfig> = serde_json::from_str(&config_data)
                .context("Failed to parse configuration JSON layout structure")?;
            let nanos = SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)?
                .as_nanos();
            let mut hasher = DefaultHasher::new();
            nanos.hash(&mut hasher);
            let hash_str = format!("{:08x}", hasher.finish())[..8].to_string();
            println!("[*] Hash sequence: {}", hash_str);
            let parent_dir =
                File::open("/.snapshots").context("Failed to open /.snapshots mount target")?;
            let mut targets = Vec::new();
            for mount in &config {
                if mount.r#type == "DYNAMIC" {
                    if let Some(ref sv) = mount.subvol {
                        let name_str = format!("{}-{}", sv, hash_str);
                        targets.push((mount.mountpoint.clone(), name_str));
                    }
                }
            }
            for (_, name_str) in &targets {
                if Path::new("/.snapshots").join(name_str).exists() {
                    eprintln!(
                        "ERROR: HASH_COLLISION - Snapshot target '{}' already exists in ledger pool. Aborting transaction.",
                        name_str
                    );
                    std::process::exit(3);
                }
            }
            for (src, name_str) in &targets {
                let source_dir = File::open(&src).context(format!("Failed to open {}", src))?;
                let c_name = CString::new(name_str.clone())?;
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
            let root_snap_name = format!("@-{}", hash_str);
            let root_snap_path = Path::new("/.snapshots").join(&root_snap_name);
            std::process::Command::new("btrfs")
                .args([
                    "property",
                    "set",
                    root_snap_path.to_str().unwrap(),
                    "ro",
                    "false",
                ])
                .status()
                .context("Failed to toggle root snapshot property to read-write")?;
            let mut sorted_config = Vec::new();
            if let Some(m) = config.iter().find(|m| m.mountpoint == "/") {
                sorted_config.push(m.clone());
            }
            if let Some(m) = config.iter().find(|m| m.mountpoint == "/mnt/btrfs-root") {
                sorted_config.push(m.clone());
            }
            if let Some(m) = config.iter().find(|m| m.mountpoint == "/.snapshots") {
                sorted_config.push(m.clone());
            }
            for m in &config {
                if m.mountpoint != "/"
                    && m.mountpoint != "/mnt/btrfs-root"
                    && m.mountpoint != "/.snapshots"
                {
                    sorted_config.push(m.clone());
                }
            }
            let mut fstab_content = String::new();
            for mount in &sorted_config {
                let mut opts = mount.options.clone();
                if mount.r#type == "DYNAMIC" {
                    if let Some(ref sv) = mount.subvol {
                        // INDESTRUCTIBLE POOL FIX: Route paths relative to the master root pool pool target
                        opts.push(format!("subvol=/@snapshots/{}-{}", sv, hash_str));
                    }
                } else if let Some(ref sv) = mount.subvol {
                    opts.push(format!("subvol={}", sv));
                }
                let opts_str = if opts.is_empty() {
                    "defaults".to_string()
                } else {
                    opts.join(",")
                };
                fstab_content.push_str(&format!(
                    "{}\t{}\t{}\t{}\t{}\t{}\n",
                    mount.device, mount.mountpoint, mount.fstype, opts_str, mount.dump, mount.pass
                ));
            }
            let target_fstab_path = root_snap_path.join("etc/fstab");
            std::fs::write(&target_fstab_path, fstab_content)
                .context("Failed to write generated fstab file inside target root snapshot")?;
            println!("[+] Target snapshot /etc/fstab configuration generated successfully.");
            std::process::Command::new("btrfs")
                .args([
                    "property",
                    "set",
                    root_snap_path.to_str().unwrap(),
                    "ro",
                    "true",
                ])
                .status()
                .context("Failed to restore read-only locking parameter")?;
            println!("[+] Core baremetal sequence finished cleanly.");
        }
    }
    Ok(())
}
