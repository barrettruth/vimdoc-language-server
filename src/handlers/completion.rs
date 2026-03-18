use std::collections::HashSet;

use anyhow::Result;
use lsp_server::Response;
use lsp_types::{CompletionItem, CompletionItemKind, CompletionResponse, Position};

use crate::server::make_response;
use crate::store::Store;
use crate::tags::TagIndex;

#[must_use]
pub fn handle_completion(
    req: &lsp_server::Request,
    store: &Store,
    tag_index: &TagIndex,
) -> Response {
    let result = (|| -> Result<Option<CompletionResponse>> {
        let params: lsp_types::CompletionParams = serde_json::from_value(req.params.clone())?;
        let uri = params.text_document_position.text_document.uri;
        let pos = params.text_document_position.position;

        let Some((text, _doc)) = store.get(&uri) else {
            return Ok(None);
        };

        if !is_inside_taglink(text, pos) {
            return Ok(None);
        }

        let mut seen = HashSet::new();
        let mut items = Vec::new();

        if let Some((_text, doc)) = store.get(&uri) {
            for span in doc.tag_defs() {
                if seen.insert(span.name.clone()) {
                    items.push(CompletionItem {
                        label: span.name.clone(),
                        insert_text: Some(format!("{}|", span.name)),
                        kind: Some(CompletionItemKind::KEYWORD),
                        filter_text: Some(span.name.clone()),
                        ..CompletionItem::default()
                    });
                }
            }
        }

        for name in tag_index.all_tag_names() {
            if seen.insert(name.to_string()) {
                items.push(CompletionItem {
                    label: name.to_string(),
                    insert_text: Some(format!("{name}|")),
                    kind: Some(CompletionItemKind::KEYWORD),
                    filter_text: Some(name.to_string()),
                    ..CompletionItem::default()
                });
            }
        }

        Ok(Some(CompletionResponse::Array(items)))
    })();
    make_response(req, result)
}

fn is_inside_taglink(text: &str, pos: Position) -> bool {
    let Some(line) = text.lines().nth(pos.line as usize) else {
        return false;
    };
    let col = pos.character as usize;
    let mut utf16_offset = 0;
    let mut pipes = 0;
    for ch in line.chars() {
        if utf16_offset >= col {
            break;
        }
        if ch == '|' {
            pipes += 1;
        }
        utf16_offset += ch.len_utf16();
    }
    pipes % 2 == 1
}
