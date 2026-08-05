use std::path::{Path, PathBuf};

use gix_forge::{Binding, Comment, EntityOps};
use lsp_types::{
    CodeAction, CodeActionKind, CodeActionOrCommand, CodeActionParams, Hover, HoverContents,
    InitializeParams, InitializeResult, Location, MarkupContent, MarkupKind, OneOf, Position,
    Range, ServerCapabilities, SymbolInformation, SymbolKind, TextDocumentSyncCapability,
    TextDocumentSyncKind, Url, WorkspaceSymbolParams,
};
use tower_lsp::{LanguageServer, jsonrpc::Result, lsp_types};

pub struct Backend {
    root: PathBuf,
}

impl Backend {
    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }

    fn repo(&self) -> anyhow::Result<gix::Repository> {
        Ok(gix::open(&self.root)?)
    }

    fn comment_location(&self, comment: &Comment) -> Option<(Url, Range)> {
        let Binding::Position(anchor) = comment.binding.as_ref()? else {
            return None;
        };
        let relative = Path::new(&anchor.identity.path);
        if relative.is_absolute()
            || relative
                .components()
                .any(|component| matches!(component, std::path::Component::ParentDir))
        {
            return None;
        }
        let root = self.root.canonicalize().ok()?;
        let path = root.join(relative).canonicalize().ok()?;
        path.strip_prefix(&root).ok()?;
        let contents = std::fs::read(&path).ok()?;
        let start = position_at_offset(&contents, anchor.identity.span.start)?;
        let end = position_at_offset(&contents, anchor.identity.span.end)?;
        Some((Url::from_file_path(path).ok()?, Range { start, end }))
    }

    fn comments(&self) -> anyhow::Result<Vec<Comment>> {
        let repo = self.repo()?;
        let mut comments = Vec::new();
        for id in Comment::list(&repo)? {
            if let Some(comment) = Comment::load_from_repo(&repo, &id)? {
                comments.push(comment);
            }
        }
        Ok(comments)
    }
}

#[tower_lsp::async_trait]
impl LanguageServer for Backend {
    async fn initialize(&self, _: InitializeParams) -> Result<InitializeResult> {
        Ok(InitializeResult {
            capabilities: ServerCapabilities {
                text_document_sync: Some(TextDocumentSyncCapability::Kind(
                    TextDocumentSyncKind::FULL,
                )),
                hover_provider: Some(lsp_types::HoverProviderCapability::Simple(true)),
                workspace_symbol_provider: Some(OneOf::Left(true)),
                code_action_provider: Some(lsp_types::CodeActionProviderCapability::Simple(true)),
                ..ServerCapabilities::default()
            },
            server_info: Some(lsp_types::ServerInfo {
                name: "git-forge comments".to_owned(),
                version: Some(env!("CARGO_PKG_VERSION").to_owned()),
            }),
        })
    }

    async fn shutdown(&self) -> Result<()> {
        Ok(())
    }

    async fn symbol(
        &self,
        params: WorkspaceSymbolParams,
    ) -> Result<Option<Vec<SymbolInformation>>> {
        let query = params.query.to_lowercase();
        let symbols = self
            .comments()
            .map_err(|_| tower_lsp::jsonrpc::Error::internal_error())?
            .into_iter()
            .filter(|comment| {
                query.is_empty()
                    || comment.body.to_lowercase().contains(&query)
                    || comment.subject.to_lowercase().contains(&query)
                    || comment.author.to_lowercase().contains(&query)
            })
            .map(|comment| {
                let (uri, range) = self.comment_location(&comment).unwrap_or_else(|| {
                    (
                        Url::from_file_path(self.root.join(".git-forge-comments"))
                            .expect("path is absolute"),
                        Range::default(),
                    )
                });
                SymbolInformation {
                    name: format!(
                        "{}: {}",
                        comment.subject,
                        comment.body.lines().next().unwrap_or_default()
                    ),

                    kind: SymbolKind::STRING,
                    tags: None,
                    #[allow(deprecated)]
                    deprecated: None,
                    location: Location::new(uri, range),
                    container_name: Some("git-forge comments".to_owned()),
                }
            })
            .collect();
        Ok(Some(symbols))
    }

    async fn hover(&self, params: lsp_types::HoverParams) -> Result<Option<Hover>> {
        let Some(path) = params
            .text_document_position_params
            .text_document
            .uri
            .to_file_path()
            .ok()
        else {
            return Ok(None);
        };
        let Ok(relative) = path.strip_prefix(&self.root) else {
            return Ok(None);
        };
        let position = params.text_document_position_params.position;
        let Some(comment) = self
            .comments()
            .map_err(|_| tower_lsp::jsonrpc::Error::internal_error())?
            .into_iter()
            .find(|comment| {
                let Some(Binding::Position(anchor)) = comment.binding.as_ref() else {
                    return false;
                };
                if Path::new(&anchor.identity.path) != relative {
                    return false;
                }
                self.comment_location(comment)
                    .is_some_and(|(_, range)| position >= range.start && position <= range.end)
            })
        else {
            return Ok(None);
        };

        Ok(Some(Hover {
            contents: HoverContents::Markup(MarkupContent {
                kind: MarkupKind::Markdown,
                value: format!(
                    "**{}** ({})\n\n{}",
                    comment.author, comment.subject, comment.body
                ),
            }),
            range: self.comment_location(&comment).map(|(_, range)| range),
        }))
    }

    async fn code_action(
        &self,
        params: CodeActionParams,
    ) -> Result<Option<Vec<CodeActionOrCommand>>> {
        let uri = params.text_document.uri;
        let arguments = serde_json::json!({
            "uri": uri,
            "range": params.range,
        });
        Ok(Some(vec![CodeActionOrCommand::CodeAction(CodeAction {
            title: "Create git-forge comment".to_owned(),
            kind: Some(CodeActionKind::EMPTY),
            diagnostics: None,
            edit: None,
            command: Some(lsp_types::Command {
                title: "Create git-forge comment".to_owned(),
                command: "git-forge.comment.add".to_owned(),
                arguments: Some(vec![arguments]),
            }),
            is_preferred: Some(true),
            disabled: None,
            data: None,
        })]))
    }
}

fn position_at_offset(contents: &[u8], offset: u64) -> Option<Position> {
    let offset = usize::try_from(offset).ok()?;
    let prefix = contents.get(..offset)?;
    let line = prefix.iter().filter(|&&byte| byte == b'\n').count();
    let line_prefix = prefix.rsplit(|&byte| byte == b'\n').next()?;
    let character = std::str::from_utf8(line_prefix)
        .ok()?
        .encode_utf16()
        .count();
    Some(Position {
        line: u32::try_from(line).ok()?,
        character: u32::try_from(character).ok()?,
    })
}
