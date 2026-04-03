//! PID-file guard: prevents two forge-server instances for the same repo.

use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};

use anyhow::{Result, bail};

/// A guard that removes the PID file when dropped.
///
/// The file is held open with an exclusive `flock` for the lifetime of the
/// guard. Dropping the guard releases the lock and removes the file.
pub struct PidGuard {
    path: PathBuf,
    _file: File,
}

impl PidGuard {
    /// Acquire the PID file at `git_dir/forge-server.pid`.
    ///
    /// Uses an exclusive advisory lock (`flock`) to detect a running instance.
    /// The PID written into the file is informational; the lock is the real
    /// guard.
    pub fn acquire(git_dir: &Path) -> Result<Self> {
        let path = git_dir.join("forge-server.pid");

        let mut file = File::options()
            .create(true)
            .truncate(false)
            .write(true)
            .read(true)
            .open(&path)?;

        if file.try_lock().is_err() {
            let pid = fs::read_to_string(&path).unwrap_or_default();
            let pid = pid.trim();
            bail!("forge-server already running (pid {pid}) for this repository",);
        }

        file.set_len(0)?;
        write!(file, "{}", std::process::id())?;

        Ok(Self { path, _file: file })
    }
}

impl Drop for PidGuard {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}
