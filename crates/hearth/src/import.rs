//! Import external sources into the content-addressed store.
//!
//! Supported sources:
//! - `dir`: a local directory (blob writes parallelized)
//! - `tarball`: a `.tar` or `.tar.gz` archive
//! - `oci`: an OCI image reference (not yet implemented)
//!
//! # Future optimization
//!
//! Bulk imports could write a packfile directly instead of one loose object
//! per blob. This would reduce filesystem overhead (fewer inodes, one
//! sequential write) at the cost of implementation complexity. The current
//! loose-object approach with parallel writes and skip-existing is fast
//! enough for toolchain-sized imports.

use std::collections::BTreeMap;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::thread;

use git2::{Oid, Repository};

use crate::Error;
use crate::store::Store;

// ---------------------------------------------------------------------------
// Directory import
// ---------------------------------------------------------------------------

/// Import a local directory into the store, returning the root tree OID.
///
/// Blob writes are parallelized across available CPU cores. Blobs already
/// present in the object store are skipped (hash check only, no compression).
pub fn import_dir(store: &Store, path: &Path) -> Result<Oid, Error> {
    if !path.is_dir() {
        return Err(Error::Config(format!(
            "not a directory: {}",
            path.display()
        )));
    }

    let mut entries = Vec::new();
    collect_dir_entries(path, path, &mut entries)?;

    let blob_results = write_blobs_parallel(store.root(), &entries)?;

    let mut root = TreeNode::Dir(BTreeMap::new());
    for (components, oid, mode) in &blob_results {
        let refs: Vec<&str> = components.iter().map(String::as_str).collect();
        insert_tree_node(&mut root, &refs, *oid, *mode);
    }

    let oid = write_tree_node(store.repo(), root)?;
    store.create_tree_ref(oid)?;
    Ok(oid)
}

struct DirEntry {
    components: Vec<String>,
    abs_path: PathBuf,
    kind: DirEntryKind,
}

enum DirEntryKind {
    File { executable: bool },
    Symlink,
}

fn collect_dir_entries(
    root: &Path,
    current: &Path,
    out: &mut Vec<DirEntry>,
) -> Result<(), Error> {
    let mut children: Vec<_> = fs::read_dir(current)?.collect::<Result<Vec<_>, _>>()?;
    children.sort_by_key(fs::DirEntry::file_name);

    for child in children {
        let ft = child.file_type()?;
        let abs = child.path();
        let rel = abs
            .strip_prefix(root)
            .map_err(|e| Error::Config(e.to_string()))?;
        let components: Vec<String> = rel
            .components()
            .filter_map(|c| match c {
                std::path::Component::Normal(s) => s.to_str().map(String::from),
                _ => None,
            })
            .collect();

        if ft.is_dir() {
            collect_dir_entries(root, &abs, out)?;
        } else if ft.is_symlink() {
            out.push(DirEntry {
                components,
                abs_path: abs,
                kind: DirEntryKind::Symlink,
            });
        } else {
            let executable = file_mode(&abs) == 0o100_755;
            out.push(DirEntry {
                components,
                abs_path: abs,
                kind: DirEntryKind::File { executable },
            });
        }
    }
    Ok(())
}

fn write_blobs_parallel(
    store_root: &Path,
    entries: &[DirEntry],
) -> Result<Vec<(Vec<String>, Oid, i32)>, Error> {
    if entries.is_empty() {
        return Ok(Vec::new());
    }

    let n_threads = thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4)
        .min(entries.len());

    let chunk_size = entries.len().div_ceil(n_threads);

    let results: Vec<Result<Vec<_>, Error>> = thread::scope(|s| {
        let handles: Vec<_> = entries
            .chunks(chunk_size)
            .map(|chunk| {
                s.spawn(move || {
                    let repo = Repository::open_bare(store_root)?;
                    let mut out = Vec::with_capacity(chunk.len());
                    for entry in chunk {
                        let (oid, mode) = write_dir_entry_blob(&repo, entry)?;
                        out.push((entry.components.clone(), oid, mode));
                    }
                    Ok::<_, Error>(out)
                })
            })
            .collect();

        handles.into_iter().map(|h| h.join().unwrap()).collect()
    });

    let mut all = Vec::with_capacity(entries.len());
    for r in results {
        all.extend(r?);
    }
    Ok(all)
}

fn write_dir_entry_blob(repo: &Repository, entry: &DirEntry) -> Result<(Oid, i32), Error> {
    match entry.kind {
        DirEntryKind::File { executable } => {
            let content = fs::read(&entry.abs_path)?;
            let oid = write_blob(repo, &content)?;
            let mode = if executable { 0o100_755 } else { 0o100_644 };
            Ok((oid, mode))
        }
        DirEntryKind::Symlink => {
            let target = read_symlink_bytes(&entry.abs_path)?;
            let oid = write_blob(repo, &target)?;
            Ok((oid, 0o12_0000))
        }
    }
}

/// Write a blob, skipping compression if it already exists.
///
/// libgit2's `git_odb_write` hashes first and short-circuits when the
/// object is already present, so this is a thin wrapper for clarity.
fn write_blob(repo: &Repository, content: &[u8]) -> Result<Oid, Error> {
    Ok(repo.blob(content)?)
}

#[cfg(unix)]
fn read_symlink_bytes(path: &Path) -> Result<Vec<u8>, Error> {
    use std::os::unix::ffi::OsStrExt;
    let target = fs::read_link(path)?;
    Ok(target.as_os_str().as_bytes().to_vec())
}

#[cfg(not(unix))]
fn read_symlink_bytes(_path: &Path) -> Result<Vec<u8>, Error> {
    Ok(Vec::new())
}

#[cfg(unix)]
fn file_mode(path: &Path) -> i32 {
    use std::os::unix::fs::PermissionsExt;
    match fs::metadata(path) {
        Ok(m) if m.permissions().mode() & 0o111 != 0 => 0o100_755,
        _ => 0o100_644,
    }
}

#[cfg(not(unix))]
fn file_mode(_path: &Path) -> i32 {
    0o100_644
}

// ---------------------------------------------------------------------------
// Tarball import
// ---------------------------------------------------------------------------

/// Import a tarball (`.tar` or `.tar.gz`) into the store.
///
/// `strip_prefix` removes N leading path components from each entry, similar
/// to `tar --strip-components=N`.
pub fn import_tarball(store: &Store, path: &Path, strip_prefix: usize) -> Result<Oid, Error> {
    let file = fs::File::open(path)?;
    let reader: Box<dyn Read> = if is_gzipped(path) {
        Box::new(flate2::read::GzDecoder::new(file))
    } else {
        Box::new(file)
    };

    let mut archive = tar::Archive::new(reader);
    let repo = store.repo();
    let mut root = TreeNode::Dir(BTreeMap::new());

    for entry in archive.entries()? {
        let mut entry = entry?;
        let path = entry.path()?.into_owned();

        if entry.header().entry_type().is_dir() {
            continue;
        }

        let components: Vec<&str> = path
            .components()
            .filter_map(|c| match c {
                std::path::Component::Normal(s) => s.to_str(),
                _ => None,
            })
            .collect();

        if components.len() <= strip_prefix {
            continue;
        }
        let components = &components[strip_prefix..];

        let entry_type = entry.header().entry_type();
        if entry_type.is_symlink() {
            if let Some(target) = entry.link_name()? {
                let target_str = target.to_str().ok_or_else(|| {
                    Error::Config("non-UTF-8 symlink target in tarball".into())
                })?;
                let oid = write_blob(repo,target_str.as_bytes())?;
                insert_tree_node(&mut root, components, oid, 0o12_0000);
            }
        } else {
            let mut content = Vec::new();
            entry.read_to_end(&mut content)?;
            let oid = write_blob(repo,&content)?;
            let mode = if entry.header().mode()? & 0o111 != 0 {
                0o100_755
            } else {
                0o100_644
            };
            insert_tree_node(&mut root, components, oid, mode);
        }
    }

    let oid = write_tree_node(repo, root)?;
    store.create_tree_ref(oid)?;
    Ok(oid)
}

fn is_gzipped(path: &Path) -> bool {
    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("");
    name.ends_with(".gz") || name.ends_with(".tgz")
}

// ---------------------------------------------------------------------------
// OCI import (stub)
// ---------------------------------------------------------------------------

/// Import an OCI image into the store.
pub fn import_oci(_store: &Store, _image_ref: &str) -> Result<Oid, Error> {
    todo!("OCI import")
}

// ---------------------------------------------------------------------------
// In-memory tree builder
// ---------------------------------------------------------------------------
//
// Used by both directory and tarball imports. Tar entries arrive in flat,
// arbitrary order, so we accumulate the nested structure in memory before
// flushing to Git tree objects bottom-up. The directory import reuses this
// after parallel blob writing.

enum TreeNode {
    Blob { oid: Oid, mode: i32 },
    Dir(BTreeMap<String, TreeNode>),
}

fn insert_tree_node(root: &mut TreeNode, components: &[&str], oid: Oid, mode: i32) {
    let TreeNode::Dir(children) = root else {
        return;
    };

    if components.len() == 1 {
        children.insert(components[0].to_string(), TreeNode::Blob { oid, mode });
        return;
    }

    let child = children
        .entry(components[0].to_string())
        .or_insert_with(|| TreeNode::Dir(BTreeMap::new()));
    insert_tree_node(child, &components[1..], oid, mode);
}

fn write_tree_node(repo: &Repository, node: TreeNode) -> Result<Oid, Error> {
    let TreeNode::Dir(children) = node else {
        return Err(Error::Config("expected directory node at root".into()));
    };

    let mut builder = repo.treebuilder(None)?;
    for (name, child) in children {
        match child {
            TreeNode::Blob { oid, mode } => {
                builder.insert(&name, oid, mode)?;
            }
            TreeNode::Dir(_) => {
                let subtree_oid = write_tree_node(repo, TreeNode::Dir(unsafe_take(child)))?;
                builder.insert(&name, subtree_oid, 0o040_000)?;
            }
        }
    }
    Ok(builder.write()?)
}

/// Extract the inner `BTreeMap` from a `TreeNode::Dir`.
///
/// # Panics
///
/// Panics if `node` is not `TreeNode::Dir`. The call-site guarantees this
/// via the surrounding `match` arm.
fn unsafe_take(node: TreeNode) -> BTreeMap<String, TreeNode> {
    match node {
        TreeNode::Dir(map) => map,
        TreeNode::Blob { .. } => unreachable!(),
    }
}
