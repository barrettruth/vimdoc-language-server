use std::path::{Path, PathBuf};

use anyhow::{Result, anyhow};
use clap::{ArgAction, Parser};
use lsp_server::{Connection, Message, Notification, Response};
use lsp_types::{
    CompletionItem, CompletionItemKind, CompletionOptions, CompletionResponse, DocumentHighlight,
    DocumentHighlightKind, DocumentLink, DocumentSymbol, DocumentSymbolResponse, FoldingRange,
    FoldingRangeKind, GotoDefinitionResponse, Hover, HoverContents, InitializeParams, Location,
    MarkupContent, MarkupKind, OneOf, Position, PublishDiagnosticsParams, Range,
    ServerCapabilities, SymbolKind, TextDocumentSyncCapability, TextDocumentSyncKind, TextEdit,
    Uri, WorkspaceEdit,
    notification::{
        DidChangeTextDocument, DidCloseTextDocument, DidOpenTextDocument,
        Notification as LspNotification, PublishDiagnostics,
    },
    request::{
        Completion, DocumentHighlightRequest, DocumentLinkRequest, DocumentSymbolRequest,
        FoldingRangeRequest, Formatting, GotoDefinition, HoverRequest, References, Rename,
        Request as LspRequest,
    },
};
use serde::Deserialize;

use vimdoc_language_server::{
    diagnostics, formatter,
    parser::{Document, LineKind, Span},
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

#[allow(clippy::struct_excessive_bools)]
struct Config {
    line_width: usize,
    formatting: bool,
    diagnostics: bool,
    hover: bool,
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
            formatting: !cli.no_formatting,
            diagnostics: !cli.no_diagnostics,
            hover: !cli.no_hover,
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
        references_provider: Some(OneOf::Left(true)),
        rename_provider: Some(OneOf::Right(lsp_types::RenameOptions {
            prepare_provider: Some(true),
            work_done_progress_options: lsp_types::WorkDoneProgressOptions::default(),
        })),
        hover_provider: if cli.no_hover {
            None
        } else {
            Some(lsp_types::HoverProviderCapability::Simple(true))
        },
        document_highlight_provider: Some(OneOf::Left(true)),
        folding_range_provider: Some(lsp_types::FoldingRangeProviderCapability::Simple(true)),
        completion_provider: Some(CompletionOptions {
            trigger_characters: Some(vec!["|".to_string()]),
            ..CompletionOptions::default()
        }),
        document_link_provider: Some(lsp_types::DocumentLinkOptions {
            resolve_provider: Some(false),
            work_done_progress_options: lsp_types::WorkDoneProgressOptions::default(),
        }),
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
        Formatting::METHOD if config.formatting => handle_formatting(req, store, config),
        DocumentSymbolRequest::METHOD => handle_document_symbol(req, store),
        GotoDefinition::METHOD => handle_goto_definition(req, store, tag_index),
        DocumentHighlightRequest::METHOD => handle_document_highlight(req, store),
        FoldingRangeRequest::METHOD => handle_folding_range(req, store),
        DocumentLinkRequest::METHOD => handle_document_link(req, store, tag_index),
        Completion::METHOD => handle_completion(req, store, tag_index),
        HoverRequest::METHOD if config.hover => handle_hover(req, store, tag_index),
        References::METHOD => handle_references(req, store, tag_index),
        Rename::METHOD => handle_rename(req, store, tag_index),
        "textDocument/prepareRename" => handle_prepare_rename(req, store),
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

fn handle_folding_range(req: &lsp_server::Request, store: &Store) -> Response {
    let result = (|| -> Result<Option<Vec<FoldingRange>>> {
        let params: lsp_types::FoldingRangeParams = serde_json::from_value(req.params.clone())?;
        let uri = params.text_document.uri;
        let (_text, doc) = store.get(&uri).ok_or_else(|| anyhow!("unknown uri"))?;

        let mut ranges = Vec::new();
        let lines = &doc.lines;
        let total = lines.len();

        let mut section_start: Option<u32> = None;
        let mut code_start: Option<u32> = None;

        #[allow(clippy::cast_possible_truncation)]
        for (i, line) in lines.iter().enumerate() {
            let line_num = i as u32;
            match &line.kind {
                LineKind::Separator(_) => {
                    if let Some(start) = section_start {
                        if line_num > start + 1 {
                            ranges.push(FoldingRange {
                                start_line: start,
                                start_character: None,
                                end_line: line_num - 1,
                                end_character: None,
                                kind: Some(FoldingRangeKind::Region),
                                collapsed_text: None,
                            });
                        }
                    }
                    section_start = Some(line_num);
                }
                LineKind::CodeBody => {
                    if code_start.is_none() {
                        code_start = Some(line_num);
                    }
                }
                _ => {
                    if let Some(start) = code_start.take() {
                        let end = line_num - 1;
                        if end > start {
                            ranges.push(FoldingRange {
                                start_line: start,
                                start_character: None,
                                end_line: end,
                                end_character: None,
                                kind: Some(FoldingRangeKind::Region),
                                collapsed_text: None,
                            });
                        }
                    }
                }
            }
        }

        #[allow(clippy::cast_possible_truncation)]
        if let Some(start) = section_start {
            let end = (total - 1) as u32;
            if end > start {
                ranges.push(FoldingRange {
                    start_line: start,
                    start_character: None,
                    end_line: end,
                    end_character: None,
                    kind: Some(FoldingRangeKind::Region),
                    collapsed_text: None,
                });
            }
        }

        Ok(Some(ranges))
    })();
    make_response(req, result)
}

fn handle_document_link(
    req: &lsp_server::Request,
    store: &Store,
    tag_index: &mut TagIndex,
) -> Response {
    let result = (|| -> Result<Option<Vec<DocumentLink>>> {
        let params: lsp_types::DocumentLinkParams = serde_json::from_value(req.params.clone())?;
        let uri = params.text_document.uri;
        let (_text, doc) = store.get(&uri).ok_or_else(|| anyhow!("unknown uri"))?;

        let mut links = Vec::new();
        for span in doc.tag_refs() {
            let target = doc
                .tag_defs()
                .find(|d| d.name == span.name)
                .map(|d| Location {
                    uri: uri.clone(),
                    range: d.range,
                })
                .or_else(|| {
                    tag_index.resolve(&span.name).map(|e| Location {
                        uri: e.uri,
                        range: e.range,
                    })
                });
            if let Some(loc) = target {
                links.push(DocumentLink {
                    range: span.range,
                    target: Some(loc.uri),
                    tooltip: Some(span.name.clone()),
                    data: None,
                });
            }
        }

        Ok(Some(links))
    })();
    make_response(req, result)
}

fn handle_completion(req: &lsp_server::Request, store: &Store, tag_index: &TagIndex) -> Response {
    let result = (|| -> Result<Option<CompletionResponse>> {
        let params: lsp_types::CompletionParams = serde_json::from_value(req.params.clone())?;
        let uri = params.text_document_position.text_document.uri;

        let mut seen = std::collections::HashSet::new();
        let mut items = Vec::new();

        if let Some((_text, doc)) = store.get(&uri) {
            for span in doc.tag_defs() {
                if seen.insert(span.name.clone()) {
                    items.push(CompletionItem {
                        label: span.name.clone(),
                        kind: Some(CompletionItemKind::REFERENCE),
                        ..CompletionItem::default()
                    });
                }
            }
        }

        for name in tag_index.all_tag_names() {
            if seen.insert(name.to_string()) {
                items.push(CompletionItem {
                    label: name.to_string(),
                    kind: Some(CompletionItemKind::REFERENCE),
                    ..CompletionItem::default()
                });
            }
        }

        Ok(Some(CompletionResponse::Array(items)))
    })();
    make_response(req, result)
}

fn handle_hover(req: &lsp_server::Request, store: &Store, tag_index: &mut TagIndex) -> Response {
    let result = (|| -> Result<Option<Hover>> {
        let params: lsp_types::HoverParams = serde_json::from_value(req.params.clone())?;
        let uri = params.text_document_position_params.text_document.uri;
        let pos = params.text_document_position_params.position;
        let (_text, doc) = store.get(&uri).ok_or_else(|| anyhow!("unknown uri"))?;

        let Some(name) = tag_name_at(doc, pos) else {
            return Ok(None);
        };

        let location = doc
            .tag_defs()
            .find(|d| d.name == name)
            .map(|d| (uri.clone(), d.range))
            .or_else(|| tag_index.resolve(&name).map(|e| (e.uri, e.range)));

        let Some((def_uri, def_range)) = location else {
            return Ok(None);
        };

        let text = if def_uri == uri {
            store.get(&def_uri).map(|(t, _)| t.to_string())
        } else {
            uri_to_path(&def_uri).and_then(|p| std::fs::read_to_string(p).ok())
        };

        let Some(text) = text else {
            return Ok(None);
        };

        let Some(context) = extract_hover_context(&text, def_range.start.line) else {
            return Ok(None);
        };

        Ok(Some(Hover {
            contents: HoverContents::Markup(MarkupContent {
                kind: MarkupKind::PlainText,
                value: context,
            }),
            range: None,
        }))
    })();
    make_response(req, result)
}

fn handle_references(req: &lsp_server::Request, store: &Store, tag_index: &TagIndex) -> Response {
    let result = (|| -> Result<Option<Vec<Location>>> {
        let params: lsp_types::ReferenceParams = serde_json::from_value(req.params.clone())?;
        let uri = params.text_document_position.text_document.uri;
        let pos = params.text_document_position.position;
        let (_text, doc) = store.get(&uri).ok_or_else(|| anyhow!("unknown uri"))?;

        let Some(name) = tag_name_at(doc, pos) else {
            return Ok(None);
        };

        let mut locations: Vec<Location> = tag_index
            .find_references(&name)
            .into_iter()
            .map(|e| Location {
                uri: e.uri,
                range: e.range,
            })
            .collect();

        if params.context.include_declaration {
            if let Some(entries) = tag_index.workspace_defs(&name) {
                for entry in entries {
                    locations.push(Location {
                        uri: entry.uri.clone(),
                        range: entry.range,
                    });
                }
            }
        }

        Ok(Some(locations))
    })();
    make_response(req, result)
}

fn handle_prepare_rename(req: &lsp_server::Request, store: &Store) -> Response {
    let result = (|| -> Result<Option<lsp_types::PrepareRenameResponse>> {
        let params: lsp_types::TextDocumentPositionParams =
            serde_json::from_value(req.params.clone())?;
        let uri = params.text_document.uri;
        let pos = params.position;
        let (_text, doc) = store.get(&uri).ok_or_else(|| anyhow!("unknown uri"))?;

        let span = find_span_at(doc.tag_defs(), pos).or_else(|| find_span_at(doc.tag_refs(), pos));

        Ok(span.map(|s| lsp_types::PrepareRenameResponse::Range(s.range)))
    })();
    make_response(req, result)
}

#[allow(clippy::similar_names)]
fn handle_rename(req: &lsp_server::Request, store: &Store, tag_index: &TagIndex) -> Response {
    let result = (|| -> Result<Option<WorkspaceEdit>> {
        let params: lsp_types::RenameParams = serde_json::from_value(req.params.clone())?;
        let uri = params.text_document_position.text_document.uri;
        let pos = params.text_document_position.position;
        let new_name = params.new_name;
        let (_text, doc) = store.get(&uri).ok_or_else(|| anyhow!("unknown uri"))?;

        let span = find_span_at(doc.tag_defs(), pos).or_else(|| find_span_at(doc.tag_refs(), pos));

        let Some(span) = span else {
            return Ok(None);
        };
        let old_name = span.name.clone();
        let is_def = doc.tag_defs().any(|d| d.name == old_name);

        let new_def_text = format!("*{new_name}*");
        let new_ref_text = format!("|{new_name}|");

        #[allow(clippy::mutable_key_type)]
        let mut changes: std::collections::HashMap<Uri, Vec<TextEdit>> =
            std::collections::HashMap::new();

        collect_rename_edits(
            doc,
            &uri,
            &old_name,
            &new_def_text,
            &new_ref_text,
            &mut changes,
        );

        if is_def {
            for (ws_uri, ws_doc) in tag_index.workspace_docs() {
                if *ws_uri != uri {
                    collect_rename_edits(
                        ws_doc,
                        ws_uri,
                        &old_name,
                        &new_def_text,
                        &new_ref_text,
                        &mut changes,
                    );
                }
            }
        }

        Ok(Some(WorkspaceEdit {
            changes: Some(changes),
            ..WorkspaceEdit::default()
        }))
    })();
    make_response(req, result)
}

#[allow(clippy::mutable_key_type, clippy::similar_names)]
fn collect_rename_edits(
    doc: &Document,
    uri: &Uri,
    old_name: &str,
    new_def_text: &str,
    new_ref_text: &str,
    changes: &mut std::collections::HashMap<Uri, Vec<TextEdit>>,
) {
    let edits: Vec<TextEdit> = doc
        .tag_defs()
        .filter(|d| d.name == old_name)
        .map(|d| TextEdit {
            range: d.range,
            new_text: new_def_text.to_string(),
        })
        .chain(
            doc.tag_refs()
                .filter(|r| r.name == old_name)
                .map(|r| TextEdit {
                    range: r.range,
                    new_text: new_ref_text.to_string(),
                }),
        )
        .collect();
    if !edits.is_empty() {
        changes.entry(uri.clone()).or_default().extend(edits);
    }
}

fn extract_hover_context(text: &str, line: u32) -> Option<String> {
    let lines: Vec<&str> = text.lines().collect();
    let line = line as usize;
    if line >= lines.len() {
        return None;
    }
    let start = line.saturating_sub(1);
    let end = (line + 4).min(lines.len());
    Some(lines[start..end].join("\n"))
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
                .last()
                .ok_or_else(|| anyhow!("empty content changes"))?
                .text;
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

fn text_end_position(text: &str) -> Position {
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

fn position_in_range(pos: Position, range: Range) -> bool {
    let line = pos.line;
    let ch = pos.character;
    line == range.start.line && ch >= range.start.character && ch < range.end.character
}
