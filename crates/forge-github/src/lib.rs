//! GitHub import/export adapter for the forge store.

pub mod client;
pub mod config;
pub mod export;
pub mod import;
pub mod state;

/// Summary of a single sync run.
#[derive(Debug, Default)]
pub struct SyncReport {
    /// Number of entities newly imported.
    pub imported: usize,
    /// Number of entities exported to the remote.
    pub exported: usize,
    /// Number of entities skipped (already present in sync state).
    pub skipped: usize,
    /// Number of entities that failed.
    pub failed: usize,
}
