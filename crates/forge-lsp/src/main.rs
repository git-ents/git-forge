//! Forge LSP server — publishes inline diagnostics for active review comments.

use std::collections::HashMap;
use std::sync::RwLock;

use git2::Repository;
use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::{
    Diagnostic, DiagnosticSeverity, DidChangeTextDocumentParams, DidOpenTextDocumentParams,
    DidSaveTextDocumentParams, InitializeParams, InitializeResult, InitializedParams, MessageType,
    Position, Range, SaveOptions, ServerCapabilities, TextDocumentSyncCapability,
    TextDocumentSyncKind, TextDocumentSyncOptions, TextDocumentSyncSaveOptions, Url,
};
use tower_lsp::{Client, LanguageServer, LspService, Server};

use git_forge::Store;
use git_forge::comment::{self, Anchor, Comment, object_comment_ref};
use git_forge::refs::walk_tree;
use git_forge::review::ReviewState;

struct ForgeLanguageServer {
    client: Client,
    repo_path: RwLock<Option<std::path::PathBuf>>,
}

impl ForgeLanguageServer {
    fn new(client: Client) -> Self {
        Self {
            client,
            repo_path: RwLock::new(None),
        }
    }

    /// Open the git repository for the workspace.
    fn open_repo(&self) -> Option<Repository> {
        let path = self.repo_path.read().ok()?;
        let path = path.as_ref()?;
        Repository::discover(path).ok()
    }

    /// Collect all blob OIDs that are targets of open reviews.
    fn active_review_blobs(repo: &Repository) -> HashMap<String, Vec<String>> {
        let store = Store::new(repo);
        let Ok(reviews) = store.list_reviews_by_state(&ReviewState::Open) else {
            return HashMap::new();
        };

        let mut blob_paths: HashMap<String, Vec<String>> = HashMap::new();
        for review in &reviews {
            let Ok(oid) = git2::Oid::from_str(&review.target.head) else {
                continue;
            };
            let Ok(obj) = repo.find_object(oid, None) else {
                continue;
            };
            let mut files = Vec::new();
            match obj.kind() {
                Some(git2::ObjectType::Blob) => {
                    files.push(("(blob)".to_string(), review.target.head.clone()));
                }
                Some(git2::ObjectType::Tree) => {
                    if let Ok(tree) = repo.find_tree(oid) {
                        walk_tree(repo, &tree, "", &mut files);
                    }
                }
                Some(git2::ObjectType::Commit) => {
                    if let Ok(commit) = repo.find_commit(oid)
                        && let Ok(tree) = commit.tree()
                    {
                        walk_tree(repo, &tree, "", &mut files);
                    }
                }
                _ => {}
            }
            for (path, blob_oid) in files {
                blob_paths.entry(blob_oid).or_default().push(path);
            }
        }
        blob_paths
    }

    /// Hash file content to a git blob OID (same algorithm as `git hash-object`).
    fn hash_content(repo: &Repository, content: &[u8]) -> Option<String> {
        repo.blob(content).ok().map(|oid| oid.to_string())
    }

    /// Build diagnostics for a document whose content hashes to `blob_oid`.
    fn diagnostics_for_blob(repo: &Repository, blob_oid: &str) -> Vec<Diagnostic> {
        let ref_name = object_comment_ref(blob_oid);
        let Ok(comments) = comment::list_comments(repo, &ref_name) else {
            return Vec::new();
        };

        let mut diagnostics = Vec::new();
        for c in &comments {
            if c.resolved || c.replaces.is_some() {
                continue;
            }
            let range = comment_range(c);
            let source = c
                .migrated_from
                .as_ref()
                .map_or("forge", |_| "forge (migrated)");
            diagnostics.push(Diagnostic {
                range,
                severity: Some(DiagnosticSeverity::HINT),
                source: Some(source.to_string()),
                message: format!("{}: {}", c.author_name, c.body),
                ..Default::default()
            });
        }
        diagnostics
    }

    /// Publish diagnostics for a single document.
    async fn refresh_document(&self, uri: &Url, content: &str) {
        let Some(repo) = self.open_repo() else {
            return;
        };

        let Some(blob_oid) = Self::hash_content(&repo, content.as_bytes()) else {
            return;
        };

        let blob_paths = Self::active_review_blobs(&repo);
        if !blob_paths.contains_key(&blob_oid) {
            self.client
                .publish_diagnostics(uri.clone(), Vec::new(), None)
                .await;
            return;
        }

        let diagnostics = Self::diagnostics_for_blob(&repo, &blob_oid);
        self.client
            .publish_diagnostics(uri.clone(), diagnostics, None)
            .await;
    }
}

/// Map a comment's anchor range to an LSP Range.
#[allow(clippy::cast_possible_truncation)]
fn comment_range(comment: &Comment) -> Range {
    if let Some(Anchor::Object {
        range: Some(range), ..
    }) = &comment.anchor
        && let Some((start, end)) = parse_line_range(range)
    {
        return Range {
            start: Position {
                line: start.saturating_sub(1) as u32,
                character: 0,
            },
            end: Position {
                line: end as u32,
                character: 0,
            },
        };
    }
    Range {
        start: Position {
            line: 0,
            character: 0,
        },
        end: Position {
            line: 0,
            character: 0,
        },
    }
}

fn parse_line_range(range: &str) -> Option<(usize, usize)> {
    let (a, b) = range.split_once('-')?;
    Some((a.parse().ok()?, b.parse().ok()?))
}

#[tower_lsp::async_trait]
impl LanguageServer for ForgeLanguageServer {
    async fn initialize(&self, params: InitializeParams) -> Result<InitializeResult> {
        if let Some(root) = params.root_uri.as_ref().and_then(|u| u.to_file_path().ok())
            && let Ok(mut path) = self.repo_path.write()
        {
            *path = Some(root);
        }

        Ok(InitializeResult {
            capabilities: ServerCapabilities {
                text_document_sync: Some(TextDocumentSyncCapability::Options(
                    TextDocumentSyncOptions {
                        open_close: Some(true),
                        change: Some(TextDocumentSyncKind::FULL),
                        save: Some(TextDocumentSyncSaveOptions::SaveOptions(SaveOptions {
                            include_text: Some(true),
                        })),
                        ..Default::default()
                    },
                )),
                ..Default::default()
            },
            ..Default::default()
        })
    }

    async fn initialized(&self, _: InitializedParams) {
        self.client
            .log_message(MessageType::INFO, "forge-lsp initialized")
            .await;
    }

    async fn shutdown(&self) -> Result<()> {
        Ok(())
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        self.refresh_document(&params.text_document.uri, &params.text_document.text)
            .await;
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        if let Some(change) = params.content_changes.last() {
            self.refresh_document(&params.text_document.uri, &change.text)
                .await;
        }
    }

    async fn did_save(&self, params: DidSaveTextDocumentParams) {
        if let Some(text) = &params.text {
            self.refresh_document(&params.text_document.uri, text).await;
        }
    }
}

#[tokio::main]
async fn main() {
    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();
    let (service, socket) = LspService::new(ForgeLanguageServer::new);
    Server::new(stdin, stdout, socket).serve(service).await;
}
