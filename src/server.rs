use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::{Result, anyhow};
use lsp_server::{Connection, Message, Notification, Response};
use lsp_types::{
    Position, PublishDiagnosticsParams, Uri,
    notification::{
        DidChangeTextDocument, DidCloseTextDocument, DidOpenTextDocument,
        Notification as LspNotification, PublishDiagnostics,
    },
    request::{
        CodeActionRequest, Completion, DocumentDiagnosticRequest, DocumentHighlightRequest,
        DocumentLinkRequest, DocumentSymbolRequest, FoldingRangeRequest, Formatting,
        GotoDefinition, HoverRequest, RangeFormatting, References, Rename, Request as LspRequest,
        WorkspaceDiagnosticRequest,
    },
};
use serde::Deserialize;

use crate::diagnostics::{self, DiagnosticLevel};
use crate::formatter::ReflowMode;
use crate::handlers;
use crate::store::Store;
use crate::tags::TagIndex;

#[allow(clippy::struct_excessive_bools)]
pub struct Config {
    pub line_width: usize,
    pub formatting: bool,
    pub reflow: ReflowMode,
    pub normalize_spacing: bool,
    pub diagnostics: bool,
    pub hover: bool,
    pub runtime_tags: bool,
    pub tag_paths: Vec<PathBuf>,
    pub diagnostic_levels: HashMap<String, DiagnosticLevel>,
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct InitOptions {
    #[serde(default)]
    pub tag_paths: Vec<PathBuf>,
    pub runtime_tags: Option<bool>,
    pub line_width: Option<usize>,
    pub formatting: Option<bool>,
    pub diagnostics: Option<bool>,
    pub hover: Option<bool>,
    pub reflow: Option<ReflowMode>,
    pub normalize_spacing: Option<bool>,
    #[serde(default)]
    pub diagnostic_levels: HashMap<String, DiagnosticLevel>,
}

#[allow(clippy::missing_errors_doc)]
pub fn main_loop(connection: &Connection, config: &Config, tag_index: &mut TagIndex) -> Result<()> {
    let mut store = Store::default();

    for msg in &connection.receiver {
        match msg {
            Message::Request(req) => {
                if connection.handle_shutdown(&req)? {
                    return Ok(());
                }
                tracing::debug!(method = %req.method, "handling request");
                let resp = handle_request(&req, &store, config, tag_index);
                connection.sender.send(Message::Response(resp))?;
            }
            Message::Notification(notif) => {
                tracing::debug!(method = %notif.method, "handling notification");
                handle_notification(notif, &mut store, connection, config, tag_index)?;
            }
            Message::Response(_) => {}
        }
    }
    Ok(())
}

fn handle_request(
    req: &lsp_server::Request,
    store: &Store,
    config: &Config,
    tag_index: &mut TagIndex,
) -> Response {
    match req.method.as_str() {
        Formatting::METHOD if config.formatting => handlers::handle_formatting(req, store, config),
        RangeFormatting::METHOD if config.formatting => {
            handlers::handle_range_formatting(req, store, config)
        }
        CodeActionRequest::METHOD => handlers::handle_code_action(req, store, config, tag_index),
        "workspace/symbol" => handlers::handle_workspace_symbol(req, tag_index),
        DocumentSymbolRequest::METHOD => handlers::handle_document_symbol(req, store),
        GotoDefinition::METHOD => handlers::handle_goto_definition(req, store, tag_index),
        DocumentHighlightRequest::METHOD => handlers::handle_document_highlight(req, store),
        FoldingRangeRequest::METHOD => handlers::handle_folding_range(req, store),
        DocumentLinkRequest::METHOD => handlers::handle_document_link(req, store, tag_index),
        Completion::METHOD => handlers::handle_completion(req, store, tag_index),
        HoverRequest::METHOD if config.hover => handlers::handle_hover(req, store, tag_index),
        References::METHOD => handlers::handle_references(req, store, tag_index),
        Rename::METHOD => handlers::handle_rename(req, store, tag_index),
        "textDocument/prepareRename" => handlers::handle_prepare_rename(req, store),
        DocumentDiagnosticRequest::METHOD if config.diagnostics => {
            handlers::handle_document_diagnostic(req, store, tag_index, config)
        }
        WorkspaceDiagnosticRequest::METHOD if config.diagnostics => {
            handlers::handle_workspace_diagnostic(req, tag_index, config)
        }
        _ => Response {
            id: req.id.clone(),
            result: None,
            error: Some(lsp_server::ResponseError {
                code: lsp_server::ErrorCode::MethodNotFound as i32,
                message: format!("unknown method: {}", req.method),
                data: None,
            }),
        },
    }
}

fn handle_notification(
    notif: Notification,
    store: &mut Store,
    connection: &Connection,
    config: &Config,
    tag_index: &mut TagIndex,
) -> Result<()> {
    match notif.method.as_str() {
        DidOpenTextDocument::METHOD => {
            let params: lsp_types::DidOpenTextDocumentParams =
                serde_json::from_value(notif.params)?;
            let uri = params.text_document.uri;
            let text = params.text_document.text;
            store.open(uri.clone(), text);
            if let Some((_text, doc)) = store.get(&uri) {
                tag_index.update_file(&uri, doc);
            }
            if config.diagnostics {
                push_diagnostics(connection, &uri, store, tag_index, config)?;
            }
        }
        DidChangeTextDocument::METHOD => {
            let params: lsp_types::DidChangeTextDocumentParams =
                serde_json::from_value(notif.params)?;
            let uri = params.text_document.uri;
            let text = params
                .content_changes
                .into_iter()
                .last()
                .ok_or_else(|| anyhow!("empty content changes"))?
                .text;
            store.change(&uri, text);
            if let Some((_text, doc)) = store.get(&uri) {
                tag_index.update_file(&uri, doc);
            }
            if config.diagnostics {
                push_diagnostics(connection, &uri, store, tag_index, config)?;
            }
        }
        DidCloseTextDocument::METHOD => {
            let params: lsp_types::DidCloseTextDocumentParams =
                serde_json::from_value(notif.params)?;
            store.close(&params.text_document.uri);
        }
        _ => {}
    }
    Ok(())
}

fn push_diagnostics(
    connection: &Connection,
    uri: &Uri,
    store: &Store,
    tag_index: &TagIndex,
    config: &Config,
) -> Result<()> {
    let diags = store
        .get(uri)
        .map(|(_t, doc)| diagnostics::compute(doc, tag_index, uri, &config.diagnostic_levels))
        .unwrap_or_default();

    tracing::debug!(uri = %uri.as_str(), count = diags.len(), "publishing diagnostics");

    let params = PublishDiagnosticsParams {
        uri: uri.clone(),
        diagnostics: diags,
        version: None,
    };
    let notif = Notification {
        method: PublishDiagnostics::METHOD.to_string(),
        params: serde_json::to_value(params)?,
    };
    connection.sender.send(Message::Notification(notif))?;
    Ok(())
}

pub(crate) fn make_response<T: serde::Serialize>(
    req: &lsp_server::Request,
    result: Result<T>,
) -> Response {
    match result.and_then(|val| serde_json::to_value(val).map_err(Into::into)) {
        Ok(val) => Response {
            id: req.id.clone(),
            result: Some(val),
            error: None,
        },
        Err(e) => Response {
            id: req.id.clone(),
            result: None,
            error: Some(lsp_server::ResponseError {
                code: lsp_server::ErrorCode::InternalError as i32,
                message: e.to_string(),
                data: None,
            }),
        },
    }
}

pub fn uri_to_path(uri: &Uri) -> Option<PathBuf> {
    let s = uri.as_str().strip_prefix("file://")?;
    Some(PathBuf::from(percent_decode(s)))
}

fn percent_decode(s: &str) -> String {
    let mut out = Vec::with_capacity(s.len());
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Some(b) = hex_pair(bytes[i + 1], bytes[i + 2]) {
                out.push(b);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8(out).unwrap_or_else(|e| String::from_utf8_lossy(e.as_bytes()).into_owned())
}

fn hex_pair(hi: u8, lo: u8) -> Option<u8> {
    let h = match hi {
        b'0'..=b'9' => hi - b'0',
        b'A'..=b'F' => hi - b'A' + 10,
        b'a'..=b'f' => hi - b'a' + 10,
        _ => return None,
    };
    let l = match lo {
        b'0'..=b'9' => lo - b'0',
        b'A'..=b'F' => lo - b'A' + 10,
        b'a'..=b'f' => lo - b'a' + 10,
        _ => return None,
    };
    Some(h << 4 | l)
}

pub fn load_tag_path(tag_index: &mut TagIndex, path: &Path) {
    if path.is_dir() {
        let tags_file = path.join("tags");
        if tags_file.exists() {
            if let Err(e) = tag_index.load_tags_file(&tags_file) {
                tracing::warn!(path = %tags_file.display(), error = %e, "failed to load tags file");
            }
        }
    } else if path.exists() {
        if let Err(e) = tag_index.load_tags_file(path) {
            tracing::warn!(path = %path.display(), error = %e, "failed to load tags file");
        }
    }
}

pub(crate) fn text_end_position(text: &str) -> Position {
    let mut line = 0u32;
    let mut character = 0u32;
    for ch in text.chars() {
        if ch == '\n' {
            line += 1;
            character = 0;
        } else {
            #[allow(clippy::cast_possible_truncation)]
            {
                character += ch.len_utf16() as u32;
            }
        }
    }
    Position { line, character }
}
