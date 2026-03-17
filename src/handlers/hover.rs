use anyhow::{Result, anyhow};
use lsp_server::Response;
use lsp_types::{Hover, HoverContents, MarkupContent, MarkupKind};

use crate::parser::{Document, LineKind};
use crate::server::{make_response, uri_to_path};
use crate::shared::tag_name_at;
use crate::store::Store;
use crate::tags::TagIndex;

#[must_use]
pub fn handle_hover(
    req: &lsp_server::Request,
    store: &Store,
    tag_index: &mut TagIndex,
) -> Response {
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

        let def_doc = Document::parse(&text);
        let Some(context) = extract_hover_context(&def_doc, &text, def_range.start.line) else {
            return Ok(None);
        };

        Ok(Some(Hover {
            contents: HoverContents::Markup(MarkupContent {
                kind: MarkupKind::Markdown,
                value: format!("```vim\n{context}\n```"),
            }),
            range: None,
        }))
    })();
    make_response(req, result)
}

fn extract_hover_context(doc: &Document, text: &str, line: u32) -> Option<String> {
    let text_lines: Vec<&str> = text.lines().collect();
    let line = line as usize;
    if line >= text_lines.len() || line >= doc.lines.len() {
        return None;
    }

    let start = line;

    let mut end = line + 1;
    let limit = text_lines.len().min(doc.lines.len());
    while end < limit {
        match doc.lines[end].kind {
            LineKind::Separator(_) => break,
            LineKind::Blank if end > line + 1 => break,
            _ => end += 1,
        }
    }

    Some(text_lines[start..end].join("\n"))
}
