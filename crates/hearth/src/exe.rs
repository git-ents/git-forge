//! Environment activation: enter a shell, or emit direnv-compatible output.

use std::fs;
use std::process;

use git2::Oid;

use crate::{Error, store::Store};

/// Isolation level for environment activation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(u8)]
pub enum Isolation {
    /// No isolation: inherit host environment, no PATH changes.
    #[default]
    Host = 0,
    /// Declared inputs only: PATH replaced with env tree bins.
    Workspace = 1,
    /// Read-only inputs: PATH replaced, store chmod'd read-only, writes captured.
    ReadOnly = 2,
}

impl Isolation {
    /// Convert from a numeric isolation level.
    pub fn from_u8(n: u8) -> Result<Self, Error> {
        match n {
            0 => Ok(Self::Host),
            1 => Ok(Self::Workspace),
            2 => Ok(Self::ReadOnly),
            _ => todo!("isolation level {n} requires VM support"),
        }
    }
}

/// Enter an environment by spawning a shell inside it.
///
/// Returns the exit status of the spawned shell.
pub fn enter(
    store: &Store,
    tree_oid: Oid,
    isolation: Isolation,
) -> Result<process::ExitStatus, Error> {
    let store_path = store.materialize(tree_oid)?;

    let capture = if isolation == Isolation::ReadOnly {
        let id = run_id();
        let dir = store.root().join("runs").join(&id).join("capture");
        fs::create_dir_all(&dir)?;
        set_read_only_recursive(&store_path)?;
        Some(dir)
    } else {
        None
    };

    spawn_shell(&store_path, capture.as_deref(), tree_oid, isolation)
}

/// Print shell-eval-able direnv output for an environment tree.
///
/// Replaces PATH with `<tree>/bin` and exports `HEARTH_ENV`.
pub fn direnv_output(env_path: &std::path::Path, tree_oid: Oid) {
    let bin = env_path.join("bin");
    println!("export PATH=\"{}\"", bin.display());
    println!("export HEARTH_ENV=\"{tree_oid}\"");
    println!("export HEARTH_ISOLATION=\"1\"");
}

fn spawn_shell(
    env_path: &std::path::Path,
    capture: Option<&std::path::Path>,
    tree_oid: Oid,
    isolation: Isolation,
) -> Result<process::ExitStatus, Error> {
    let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".into());

    let mut cmd = process::Command::new(&shell);
    cmd.env("HEARTH_ENV", tree_oid.to_string());
    cmd.env("HEARTH_ISOLATION", (isolation as u8).to_string());

    match isolation {
        Isolation::Host => {
            // Inherit host PATH unchanged.
        }
        Isolation::Workspace | Isolation::ReadOnly => {
            // Replace PATH with only the env tree's bin/.
            let bin = env_path.join("bin");
            cmd.env("PATH", bin.display().to_string());
        }
    }

    if let Some(capture_dir) = capture {
        cmd.env("TMPDIR", capture_dir);
    }

    Ok(cmd.status()?)
}

fn run_id() -> String {
    let t = std::time::SystemTime::now()
        .duration_since(std::time::SystemTime::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    format!("{t:x}-{}", process::id())
}

#[cfg(unix)]
fn set_read_only_recursive(path: &std::path::Path) -> Result<(), Error> {
    use std::os::unix::fs::PermissionsExt;
    for entry in fs::read_dir(path)? {
        let entry = entry?;
        let meta = entry.metadata()?;
        let p = entry.path();
        if meta.is_dir() {
            set_read_only_recursive(&p)?;
        } else if meta.is_file() {
            let mut perms = meta.permissions();
            let mode = perms.mode() & !0o222;
            perms.set_mode(mode);
            fs::set_permissions(&p, perms)?;
        }
    }
    Ok(())
}

#[cfg(not(unix))]
fn set_read_only_recursive(_path: &std::path::Path) -> Result<(), Error> {
    Ok(())
}
