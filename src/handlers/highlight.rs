use anyhow::{Result, anyhow};
use lsp_server::Response;
use lsp_types::{DocumentHighlight, DocumentHighlightKind};

use crate::server::make_response;
use crate::shared::tag_name_at;
use crate::store::Store;

#[must_use]
pub fn handle_document_highlight(req: &lsp_server::Request, store: &Store) -> Response {
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
