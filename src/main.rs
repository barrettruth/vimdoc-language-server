use std::path::{Path, PathBuf};

use anyhow::{Result, anyhow};
use clap::{ArgAction, Parser};
use lsp_server::{Connection, Message, Notification, Response};
use lsp_types::{
    DocumentHighlight, DocumentHighlightKind, DocumentSymbol, DocumentSymbolResponse,
    GotoDefinitionResponse, InitializeParams, Location, OneOf, Position, PublishDiagnosticsParams,
    Range, ServerCapabilities, SymbolKind, TextDocumentSyncCapability, TextDocumentSyncKind,
    TextEdit, Uri,
    notification::{
        DidChangeTextDocument, DidCloseTextDocument, DidOpenTextDocument,
        Notification as LspNotification, PublishDiagnostics,
    },
    request::{
        DocumentHighlightRequest, DocumentSymbolRequest, Formatting, GotoDefinition,
        Request as LspRequest,
    },
};
use serde::Deserialize;

use vimdoc_language_server::{
    diagnostics, formatter,
    parser::{Document, Span},
    store::Store,
    tags::TagIndex,
};

#[derive(Parser)]
#[command(version, about = "Language server for vim help files")]
#[allow(clippy::struct_excessive_bools)]
struct Cli {
    #[arg(long, short = 'v', action = ArgAction::Count)]
    verbose: u8,

    #[arg(long, value_name = "PATH")]
    log_file: Option<PathBuf>,

    #[arg(long, default_value_t = 78, value_name = "N")]
    line_width: usize,

    #[arg(long)]
    no_formatting: bool,

    #[arg(long)]
    no_diagnostics: bool,

    #[arg(long)]
    no_hover: bool,

    #[arg(long)]
    print_config_schema: bool,

    #[arg(long, value_name = "PATH")]
    tag_path: Vec<PathBuf>,

    #[arg(long)]
    no_runtime_tags: bool,
}

struct Config {
    line_width: usize,
    diagnostics: bool,
    runtime_tags: bool,
    tag_paths: Vec<PathBuf>,
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct InitOptions {
    #[serde(default)]
    tag_paths: Vec<PathBuf>,
    runtime_tags: Option<bool>,
}

impl Config {
    fn from_cli_and_init(cli: &Cli, init_opts: InitOptions) -> Self {
        let mut tag_paths = cli.tag_path.clone();
        tag_paths.extend(init_opts.tag_paths);

        Self {
            line_width: cli.line_width,
            diagnostics: !cli.no_diagnostics,
            runtime_tags: init_opts.runtime_tags.unwrap_or(!cli.no_runtime_tags),
            tag_paths,
        }
    }
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    if cli.print_config_schema {
        println!("{{}}");
        return Ok(());
    }

    let (connection, io_threads) = Connection::stdio();

    let server_caps = serde_json::to_value(ServerCapabilities {
        text_document_sync: Some(TextDocumentSyncCapability::Kind(TextDocumentSyncKind::FULL)),
        document_formatting_provider: if cli.no_formatting {
            None
        } else {
            Some(OneOf::Left(true))
        },
        document_symbol_provider: Some(OneOf::Left(true)),
        definition_provider: Some(OneOf::Left(true)),
        document_highlight_provider: Some(OneOf::Left(true)),
        ..Default::default()
    })?;

    let init_params: InitializeParams =
        serde_json::from_value(connection.initialize(server_caps)?)?;

    let init_opts: InitOptions = init_params
        .initialization_options
        .and_then(|v| serde_json::from_value(v).ok())
        .unwrap_or_default();

    let config = Config::from_cli_and_init(&cli, init_opts);

    let workspace_root = init_params
        .workspace_folders
        .as_ref()
        .and_then(|wf| wf.first())
        .and_then(|f| uri_to_path(&f.uri))
        .or_else(|| {
            #[allow(deprecated)]
            init_params.root_uri.as_ref().and_then(uri_to_path)
        });

    let mut tag_index = TagIndex::new();

    if let Some(ref root) = workspace_root {
        let _ = tag_index.scan_workspace(root);
    }

    for tp in &config.tag_paths {
        load_tag_path(&mut tag_index, tp);
    }

    if config.runtime_tags {
        if let Ok(runtime) = std::env::var("VIMRUNTIME") {
            let tags_file = Path::new(&runtime).join("doc/tags");
            if tags_file.exists() {
                let _ = tag_index.load_tags_file(&tags_file);
            }
        }
    }

    main_loop(&connection, &config, &mut tag_index)?;

    io_threads.join()?;
    Ok(())
}

fn load_tag_path(tag_index: &mut TagIndex, path: &Path) {
    if path.is_dir() {
        let tags_file = path.join("tags");
        if tags_file.exists() {
            let _ = tag_index.load_tags_file(&tags_file);
        }
    } else if path.exists() {
        let _ = tag_index.load_tags_file(path);
    }
}

fn uri_to_path(uri: &Uri) -> Option<PathBuf> {
    let s = uri.as_str();
    s.strip_prefix("file://").map(PathBuf::from)
}

fn main_loop(connection: &Connection, config: &Config, tag_index: &mut TagIndex) -> Result<()> {
    let mut store = Store::default();

    for msg in &connection.receiver {
        match msg {
            Message::Request(req) => {
                if connection.handle_shutdown(&req)? {
                    return Ok(());
                }
                let resp = handle_request(&req, &store, config, tag_index);
                connection.sender.send(Message::Response(resp))?;
            }
            Message::Notification(notif) => {
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
        Formatting::METHOD => handle_formatting(req, store, config),
        DocumentSymbolRequest::METHOD => handle_document_symbol(req, store),
        GotoDefinition::METHOD => handle_goto_definition(req, store, tag_index),
        DocumentHighlightRequest::METHOD => handle_document_highlight(req, store),
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

fn handle_formatting(req: &lsp_server::Request, store: &Store, config: &Config) -> Response {
    let result = (|| -> Result<Option<Vec<TextEdit>>> {
        let params: lsp_types::DocumentFormattingParams =
            serde_json::from_value(req.params.clone())?;
        let uri = params.text_document.uri;
        let (text, _doc) = store.get(&uri).ok_or_else(|| anyhow!("unknown uri"))?;
        let new_text = formatter::format_document(text, config.line_width);
        if new_text == text {
            return Ok(None);
        }
        let end = text_end_position(text);
        Ok(Some(vec![TextEdit {
            range: Range {
                start: Position {
                    line: 0,
                    character: 0,
                },
                end,
            },
            new_text,
        }]))
    })();
    make_response(req, result)
}

fn handle_document_symbol(req: &lsp_server::Request, store: &Store) -> Response {
    let result = (|| -> Result<Option<DocumentSymbolResponse>> {
        let params: lsp_types::DocumentSymbolParams = serde_json::from_value(req.params.clone())?;
        let uri = params.text_document.uri;
        let (_text, doc) = store.get(&uri).ok_or_else(|| anyhow!("unknown uri"))?;
        let symbols: Vec<DocumentSymbol> = doc
            .tag_defs()
            .map(|span| {
                #[allow(deprecated)]
                DocumentSymbol {
                    name: span.name.clone(),
                    kind: SymbolKind::KEY,
                    range: span.range,
                    selection_range: span.range,
                    detail: None,
                    tags: None,
                    deprecated: None,
                    children: None,
                }
            })
            .collect();
        Ok(Some(DocumentSymbolResponse::Nested(symbols)))
    })();
    make_response(req, result)
}

fn handle_goto_definition(
    req: &lsp_server::Request,
    store: &Store,
    tag_index: &mut TagIndex,
) -> Response {
    let result = (|| -> Result<Option<GotoDefinitionResponse>> {
        let params: lsp_types::GotoDefinitionParams = serde_json::from_value(req.params.clone())?;
        let uri = params.text_document_position_params.text_document.uri;
        let pos = params.text_document_position_params.position;
        let (_text, doc) = store.get(&uri).ok_or_else(|| anyhow!("unknown uri"))?;

        let Some(name) = tag_name_at(doc, pos) else {
            return Ok(None);
        };

        let def = doc.tag_defs().find(|d| d.name == name);
        if let Some(d) = def {
            return Ok(Some(GotoDefinitionResponse::Scalar(Location {
                uri: uri.clone(),
                range: d.range,
            })));
        }

        Ok(tag_index.resolve(&name).map(|entry| {
            GotoDefinitionResponse::Scalar(Location {
                uri: entry.uri,
                range: entry.range,
            })
        }))
    })();
    make_response(req, result)
}

fn handle_document_highlight(req: &lsp_server::Request, store: &Store) -> Response {
    let result =
        (|| -> Result<Option<Vec<DocumentHighlight>>> {
            let params: lsp_types::DocumentHighlightParams =
                serde_json::from_value(req.params.clone())?;
            let uri = params.text_document_position_params.text_document.uri;
            let pos = params.text_document_position_params.position;
            let (_text, doc) = store.get(&uri).ok_or_else(|| anyhow!("unknown uri"))?;

            let Some(name) = tag_name_at(doc, pos) else {
                return Ok(None);
            };

            let mut highlights: Vec<DocumentHighlight> = doc
                .tag_defs()
                .filter(|d| d.name == name)
                .map(|d| DocumentHighlight {
                    range: d.range,
                    kind: Some(DocumentHighlightKind::WRITE),
                })
                .collect();

            highlights.extend(doc.tag_refs().filter(|r| r.name == name).map(|r| {
                DocumentHighlight {
                    range: r.range,
                    kind: Some(DocumentHighlightKind::READ),
                }
            }));

            Ok(Some(highlights))
        })();
    make_response(req, result)
}

fn tag_name_at(doc: &Document, pos: Position) -> Option<String> {
    find_span_at(doc.tag_refs(), pos)
        .or_else(|| find_span_at(doc.tag_defs(), pos))
        .map(|s| s.name.clone())
}

fn find_span_at<'a>(mut spans: impl Iterator<Item = &'a Span>, pos: Position) -> Option<&'a Span> {
    spans.find(|s| position_in_range(pos, s.range))
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
                push_diagnostics(connection, &uri, store)?;
            }
        }
        DidChangeTextDocument::METHOD => {
            let params: lsp_types::DidChangeTextDocumentParams =
                serde_json::from_value(notif.params)?;
            let uri = params.text_document.uri;
            let text = params
                .content_changes
                .into_iter()
                .next()
                .map(|c| c.text)
                .unwrap_or_default();
            store.change(&uri, text);
            if let Some((_text, doc)) = store.get(&uri) {
                tag_index.update_file(&uri, doc);
            }
            if config.diagnostics {
                push_diagnostics(connection, &uri, store)?;
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

fn push_diagnostics(connection: &Connection, uri: &Uri, store: &Store) -> Result<()> {
    let diags = store
        .get(uri)
        .map(|(_t, doc)| diagnostics::compute(doc))
        .unwrap_or_default();

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

fn make_response<T: serde::Serialize>(req: &lsp_server::Request, result: Result<T>) -> Response {
    match result {
        Ok(val) => Response {
            id: req.id.clone(),
            result: serde_json::to_value(val).ok(),
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

fn text_end_position(text: &str) -> Position {
    let mut line = 0u32;
    let mut character = 0u32;
    for ch in text.chars() {
        if ch == '\n' {
            line += 1;
            character = 0;
        } else {
            character += 1;
        }
    }
    Position { line, character }
}

fn position_in_range(pos: Position, range: Range) -> bool {
    let line = pos.line;
    let ch = pos.character;
    line == range.start.line && ch >= range.start.character && ch < range.end.character
}
