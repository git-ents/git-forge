use std::{fmt, fmt::Write as _};

use gix::bstr::ByteSlice;

const MAX_FILE_SIZE: u64 = 1024 * 1024;
const NUL_SCAN_SIZE: usize = 8 * 1024;

/// Load a read-only view of a Git tree or text blob without consulting the worktree.
pub fn load(
    repo: &gix::Repository,
    requested_ref: Option<&str>,
    requested_path: Option<&str>,
) -> Result<Browse, Error> {
    let path = requested_path.unwrap_or("");
    let components = validate_path(path)?;
    let route_path = route_path(&components);

    let ref_name = requested_ref.unwrap_or("HEAD");
    let mut reference = repo
        .find_reference(ref_name)
        .map_err(|error| Error::Reference(error.to_string()))?;
    let reference_name = reference.name().as_bstr().as_bytes().to_owned();
    let mut tree = reference
        .peel_to_tree()
        .map_err(|error| Error::Reference(error.to_string()))?;
    let tree_id = tree.id;

    let view = if components.is_empty() {
        BrowseView::Directory {
            object_id: tree_id,
            entries: list_entries(repo, &tree, &route_path)?,
        }
    } else {
        let (object_id, kind) = find_entry(repo, &mut tree, &components)?;
        match kind {
            gix::objs::tree::EntryKind::Tree => {
                let tree = repo
                    .find_tree(object_id)
                    .map_err(|error| Error::Object(error.to_string()))?;
                BrowseView::Directory {
                    object_id,
                    entries: list_entries(repo, &tree, &route_path)?,
                }
            }
            gix::objs::tree::EntryKind::Blob | gix::objs::tree::EntryKind::BlobExecutable => {
                BrowseView::File(load_file(repo, object_id, path)?)
            }
            kind => {
                return Err(Error::UnsupportedEntryKind {
                    path: path.to_owned(),
                    kind,
                });
            }
        }
    };

    Ok(Browse {
        reference: RefInfo {
            requested: requested_ref.map(str::to_owned),
            name: display_name(&reference_name),
            name_bytes: reference_name,
            selector: percent_encode(reference.name().as_bstr().as_bytes()),
            tree_id,
        },
        path: path.to_owned(),
        route_path,
        breadcrumbs: breadcrumbs(&components),
        view,
    })
}

fn validate_path(path: &str) -> Result<Vec<&str>, Error> {
    if path.is_empty() {
        return Ok(Vec::new());
    }
    if path.starts_with('/') || path.ends_with('/') || path.contains('\0') {
        return Err(Error::InvalidPath(path.to_owned()));
    }

    let components = path.split('/').collect::<Vec<_>>();
    if components
        .iter()
        .any(|component| component.is_empty() || *component == "." || *component == "..")
    {
        return Err(Error::InvalidPath(path.to_owned()));
    }
    Ok(components)
}

fn find_entry<'repo>(
    repo: &'repo gix::Repository,
    tree: &mut gix::Tree<'repo>,
    components: &[&str],
) -> Result<(gix::ObjectId, gix::objs::tree::EntryKind), Error> {
    for (index, component) in components.iter().enumerate() {
        let entry = tree
            .lookup_entry(std::iter::once(*component))
            .map_err(|error| Error::Object(error.to_string()))?
            .ok_or_else(|| Error::NotFound {
                path: components[..=index].join("/"),
            })?;
        let object_id = entry.object_id();
        let kind = entry.mode().kind();

        if index + 1 == components.len() {
            return Ok((object_id, kind));
        }
        if kind != gix::objs::tree::EntryKind::Tree {
            return Err(Error::NotDirectory {
                path: components[..=index].join("/"),
            });
        }
        *tree = repo
            .find_tree(object_id)
            .map_err(|error| Error::Object(error.to_string()))?;
    }

    unreachable!("an empty path is handled before looking up an entry")
}

fn list_entries(
    repo: &gix::Repository,
    tree: &gix::Tree<'_>,
    parent_route: &str,
) -> Result<Vec<Entry>, Error> {
    tree.iter()
        .map(|entry| {
            let entry = entry.map_err(|error| Error::TreeDecode(error.to_string()))?;
            let raw_name = entry.filename().as_bytes().to_owned();
            let path_component = percent_encode(&raw_name);
            let path = join_route(parent_route, &path_component);
            let kind = entry.mode().kind();
            let object_id = entry.object_id();
            let size = match kind {
                gix::objs::tree::EntryKind::Blob | gix::objs::tree::EntryKind::BlobExecutable => {
                    Some(
                        repo.find_header(object_id)
                            .map_err(|error| Error::Object(error.to_string()))?
                            .size(),
                    )
                }
                _ => None,
            };

            Ok(Entry {
                display_name: display_name(&raw_name),
                raw_name,
                path_component,
                path,
                object_id,
                kind,
                size,
            })
        })
        .collect()
}

fn load_file(repo: &gix::Repository, object_id: gix::ObjectId, path: &str) -> Result<File, Error> {
    let header = repo
        .find_header(object_id)
        .map_err(|error| Error::Object(error.to_string()))?;
    if header.kind() != gix::objs::Kind::Blob {
        return Err(Error::UnsupportedObjectKind {
            path: path.to_owned(),
            kind: header.kind(),
        });
    }
    let size = header.size();
    if size > MAX_FILE_SIZE {
        return Err(Error::FileTooLarge {
            path: path.to_owned(),
            size,
            limit: MAX_FILE_SIZE,
        });
    }

    let mut blob = repo
        .find_blob(object_id)
        .map_err(|error| Error::Object(error.to_string()))?;
    let data = std::mem::take(&mut blob.data);
    if let Some(offset) = data[..data.len().min(NUL_SCAN_SIZE)]
        .iter()
        .position(|byte| *byte == 0)
    {
        return Err(Error::NulByte {
            path: path.to_owned(),
            offset,
        });
    }
    let text = String::from_utf8(data).map_err(|_| Error::InvalidUtf8 {
        path: path.to_owned(),
    })?;

    Ok(File {
        text,
        size,
        object_id,
    })
}

fn breadcrumbs(components: &[&str]) -> Vec<Breadcrumb> {
    let mut result = Vec::with_capacity(components.len() + 1);
    result.push(Breadcrumb {
        display: "/".to_owned(),
        path: String::new(),
    });

    let mut path = String::new();
    for component in components {
        let encoded = percent_encode(component.as_bytes());
        path = join_route(&path, &encoded);
        result.push(Breadcrumb {
            display: (*component).to_owned(),
            path: path.clone(),
        });
    }
    result
}

fn route_path(components: &[&str]) -> String {
    components.iter().fold(String::new(), |path, component| {
        join_route(&path, &percent_encode(component.as_bytes()))
    })
}

fn join_route(parent: &str, component: &str) -> String {
    if parent.is_empty() {
        component.to_owned()
    } else {
        format!("{parent}/{component}")
    }
}

fn display_name(bytes: &[u8]) -> String {
    match std::str::from_utf8(bytes) {
        Ok(name) => name.to_owned(),
        Err(_) => String::from_utf8_lossy(bytes).into_owned(),
    }
}

fn percent_encode(bytes: &[u8]) -> String {
    let mut encoded = String::with_capacity(bytes.len());
    for &byte in bytes {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~') {
            encoded.push(byte as char);
        } else {
            write!(encoded, "%{byte:02X}").expect("writing to a String cannot fail");
        }
    }
    encoded
}

/// The selected reference and the requested tree path.
#[derive(Clone, Debug)]
pub struct Browse {
    pub reference: RefInfo,
    /// The validated Git path, using `/` separators and no URL decoding.
    pub path: String,
    /// The path suitable for use in a route. Components are percent encoded.
    pub route_path: String,
    pub breadcrumbs: Vec<Breadcrumb>,
    pub view: BrowseView,
}

/// Metadata needed to display and safely select the resolved reference.
#[derive(Clone, Debug)]
pub struct RefInfo {
    pub requested: Option<String>,
    /// Lossy display text; use `name_bytes` or `selector` for navigation.
    pub name: String,
    pub name_bytes: Vec<u8>,
    pub selector: String,
    pub tree_id: gix::ObjectId,
}

/// A breadcrumb with a safe route path.
#[derive(Clone, Debug)]
pub struct Breadcrumb {
    pub display: String,
    pub path: String,
}

/// The owned content selected by a browse request.
#[derive(Clone, Debug)]
pub enum BrowseView {
    Directory {
        object_id: gix::ObjectId,
        entries: Vec<Entry>,
    },
    File(File),
}

/// An owned entry from a Git tree.
#[derive(Clone, Debug)]
pub struct Entry {
    pub display_name: String,
    pub raw_name: Vec<u8>,
    /// A percent-encoded single path component suitable for a link.
    pub path_component: String,
    /// A percent-encoded path relative to the selected tree root.
    pub path: String,
    pub object_id: gix::ObjectId,
    pub kind: gix::objs::tree::EntryKind,
    /// Present only for blob and executable-blob entries.
    pub size: Option<u64>,
}

/// An owned, validated UTF-8 text blob.
#[derive(Clone, Debug)]
pub struct File {
    pub text: String,
    pub size: u64,
    pub object_id: gix::ObjectId,
}

/// Errors produced while resolving or reading a browse request.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Error {
    InvalidPath(String),
    Reference(String),
    Object(String),
    TreeDecode(String),
    NotFound {
        path: String,
    },
    NotDirectory {
        path: String,
    },
    UnsupportedEntryKind {
        path: String,
        kind: gix::objs::tree::EntryKind,
    },
    UnsupportedObjectKind {
        path: String,
        kind: gix::objs::Kind,
    },
    FileTooLarge {
        path: String,
        size: u64,
        limit: u64,
    },
    NulByte {
        path: String,
        offset: usize,
    },
    InvalidUtf8 {
        path: String,
    },
}

impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidPath(path) => write!(formatter, "invalid Git tree path: {path:?}"),
            Self::Reference(error) => write!(formatter, "unable to resolve reference: {error}"),
            Self::Object(error) => write!(formatter, "unable to read Git object: {error}"),
            Self::TreeDecode(error) => write!(formatter, "unable to decode Git tree: {error}"),
            Self::NotFound { path } => write!(formatter, "path not found: {path}"),
            Self::NotDirectory { path } => {
                write!(formatter, "path component is not a directory: {path}")
            }
            Self::UnsupportedEntryKind { path, kind } => {
                write!(
                    formatter,
                    "unsupported Git tree entry kind at {path}: {kind:?}"
                )
            }
            Self::UnsupportedObjectKind { path, kind } => {
                write!(formatter, "unsupported Git object kind at {path}: {kind:?}")
            }
            Self::FileTooLarge { path, size, limit } => {
                write!(
                    formatter,
                    "file {path} is {size} bytes; maximum is {limit} bytes"
                )
            }
            Self::NulByte { path, offset } => {
                write!(
                    formatter,
                    "file {path} contains a NUL byte at offset {offset}"
                )
            }
            Self::InvalidUtf8 { path } => write!(formatter, "file {path} is not valid UTF-8"),
        }
    }
}

impl std::error::Error for Error {}
