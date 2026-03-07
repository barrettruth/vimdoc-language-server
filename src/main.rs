use std::path::PathBuf;

use anyhow::{Result, anyhow};
use clap::{ArgAction, Parser};
use lsp_server::{Connection, Message, Notification, Response};
use lsp_types::{
    DocumentSymbol, DocumentSymbolResponse, GotoDefinitionResponse, InitializeParams, Location,
    OneOf, Position, PublishDiagnosticsParams, Range, ServerCapabilities, SymbolKind,
    TextDocumentSyncCapability, TextDocumentSyncKind, TextEdit, Uri,
    notification::{
        DidChangeTextDocument, DidCloseTextDocument, DidOpenTextDocument,
        Notification as LspNotification, PublishDiagnostics,
    },
    request::{DocumentSymbolRequest, Formatting, GotoDefinition, Request as LspRequest},
};

use vimdoc_language_server::{diagnostics, formatter, store::Store};

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
        ..Default::default()
    })?;

    let init_params: InitializeParams =
        serde_json::from_value(connection.initialize(server_caps)?)?;
    let _ = init_params;

    main_loop(&connection, &cli)?;

    io_threads.join()?;
    Ok(())
}

fn main_loop(connection: &Connection, cli: &Cli) -> Result<()> {
    let mut store = Store::default();

    for msg in &connection.receiver {
        match msg {
            Message::Request(req) => {
                if connection.handle_shutdown(&req)? {
                    return Ok(());
                }
                let resp = handle_request(&req, &store, cli);
                connection.sender.send(Message::Response(resp))?;
            }
            Message::Notification(notif) => {
                handle_notification(notif, &mut store, connection, cli)?;
            }
            Message::Response(_) => {}
        }
    }
    Ok(())
}

fn handle_request(req: &lsp_server::Request, store: &Store, cli: &Cli) -> Response {
    match req.method.as_str() {
        Formatting::METHOD => {
            let result = (|| -> Result<Option<Vec<TextEdit>>> {
                let params: lsp_types::DocumentFormattingParams =
                    serde_json::from_value(req.params.clone())?;
                let uri = params.text_document.uri;
                let (text, _doc) = store.get(&uri).ok_or_else(|| anyhow!("unknown uri"))?;
                let new_text = formatter::format_document(text, cli.line_width);
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

        DocumentSymbolRequest::METHOD => {
            let result = (|| -> Result<Option<DocumentSymbolResponse>> {
                let params: lsp_types::DocumentSymbolParams =
                    serde_json::from_value(req.params.clone())?;
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

        GotoDefinition::METHOD => {
            let result = (|| -> Result<Option<GotoDefinitionResponse>> {
                let params: lsp_types::GotoDefinitionParams =
                    serde_json::from_value(req.params.clone())?;
                let uri = params.text_document_position_params.text_document.uri;
                let pos = params.text_document_position_params.position;
                let (_text, doc) = store.get(&uri).ok_or_else(|| anyhow!("unknown uri"))?;

                let tag_name = doc
                    .tag_refs()
                    .find(|r| position_in_range(pos, r.range))
                    .map(|r| r.name.clone())
                    .or_else(|| {
                        doc.tag_defs()
                            .find(|d| position_in_range(pos, d.range))
                            .map(|d| d.name.clone())
                    });

                let Some(name) = tag_name else {
                    return Ok(None);
                };

                let def = doc.tag_defs().find(|d| d.name == name);
                Ok(def.map(|d| {
                    GotoDefinitionResponse::Scalar(Location {
                        uri: uri.clone(),
                        range: d.range,
                    })
                }))
            })();
            make_response(req, result)
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
    cli: &Cli,
) -> Result<()> {
    match notif.method.as_str() {
        DidOpenTextDocument::METHOD => {
            let params: lsp_types::DidOpenTextDocumentParams =
                serde_json::from_value(notif.params)?;
            let uri = params.text_document.uri;
            let text = params.text_document.text;
            store.open(uri.clone(), text);
            if !cli.no_diagnostics {
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
            if !cli.no_diagnostics {
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
