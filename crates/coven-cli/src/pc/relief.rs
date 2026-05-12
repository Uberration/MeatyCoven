use anyhow::{bail, Result};
use std::path::PathBuf;
use sysinfo::{Pid, Signal, System};

/// Hardcoded cache directories eligible for clearing. Never uses glob expansion.
const USER_CACHE_DIRS: &[&str] = &["Library/Caches"];
const SYSTEM_CACHE_DIR: &str = "/Library/Caches";

/// Kill a process by PID. Requires --confirm flag at the CLI layer.
/// Uses SIGTERM only (no SIGKILL in v1). Re-checks PID identity before signaling.
pub fn kill_by_pid(pid: u32, confirm: bool) -> Result<()> {
    if !confirm {
        bail!("Refusing to kill process {pid} without --confirm. Add --confirm to proceed.");
    }

    let mut sys = System::new_all();
    sys.refresh_all();

    let pid_key = Pid::from_u32(pid);
    let name = sys
        .process(pid_key)
        .map(|p| p.name().to_string())
        .ok_or_else(|| anyhow::anyhow!("No process found with PID {pid}"))?;

    eprintln!("Sending SIGTERM to PID {pid} ({name})...");

    // Re-check identity immediately before signaling to avoid PID reuse mistakes
    sys.refresh_process(pid_key);
    let proc = sys
        .process(pid_key)
        .ok_or_else(|| anyhow::anyhow!("Process {pid} disappeared before signal could be sent"))?;

    // Verify name still matches
    let current_name = proc.name().to_string();
    if current_name != name {
        bail!(
            "PID {pid} identity changed ({name} → {current_name}). Refusing to signal to avoid PID reuse mistake."
        );
    }

    let sent = proc.kill_with(Signal::Term);
    match sent {
        Some(true) => {
            println!("SIGTERM sent to PID {pid} ({name}).");
            Ok(())
        }
        Some(false) | None => {
            bail!("Failed to send SIGTERM to PID {pid} ({name}). Check permissions.");
        }
    }
}

/// Clear user and system caches. Requires --confirm flag at the CLI layer.
/// Only removes contents of the hardcoded list above.
pub fn clear_caches(confirm: bool) -> Result<()> {
    if !confirm {
        bail!("Refusing to clear caches without --confirm. Add --confirm to proceed.");
    }

    let home = dirs_next::home_dir()
        .ok_or_else(|| anyhow::anyhow!("Could not determine home directory"))?;

    let mut cleared = 0usize;
    let mut errors = 0usize;

    for rel in USER_CACHE_DIRS {
        let path = home.join(rel);
        clear_directory_contents(&path, &mut cleared, &mut errors);
    }

    // System cache (may need elevated privs — best-effort)
    let sys_cache = PathBuf::from(SYSTEM_CACHE_DIR);
    clear_directory_contents(&sys_cache, &mut cleared, &mut errors);

    if errors > 0 {
        println!(
            "Cache clear: removed {cleared} item(s), {errors} error(s) (some may require elevated privileges)."
        );
    } else {
        println!("Cache clear: removed {cleared} item(s).");
    }

    Ok(())
}

fn clear_directory_contents(path: &PathBuf, cleared: &mut usize, errors: &mut usize) {
    if !path.exists() {
        return;
    }
    let entries = match std::fs::read_dir(path) {
        Ok(e) => e,
        Err(_) => {
            *errors += 1;
            return;
        }
    };
    for entry in entries.flatten() {
        let entry_path = entry.path();
        let result = if entry_path.is_dir() {
            std::fs::remove_dir_all(&entry_path)
        } else {
            std::fs::remove_file(&entry_path)
        };
        match result {
            Ok(()) => *cleared += 1,
            Err(_) => *errors += 1,
        }
    }
}
