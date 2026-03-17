use anyhow::{Result, anyhow};
use lsp_server::Response;

use crate::server::make_response;
use crate::shared::find_span_at;
use crate::store::Store;

#[must_use]
pub fn handle_prepare_rename(req: &lsp_server::Request, store: &Store) -> Response {
    let result = (|| -> Result<Option<lsp_types::PrepareRenameResponse>> {
        let params: lsp_types::TextDocumentPositionParams =
            serde_json::from_value(req.params.clone())?;
        let uri = params.text_document.uri;
        let pos = params.position;
        let (_text, doc) = store.get(&uri).ok_or_else(|| anyhow!("unknown uri"))?;

        let span = find_span_at(doc.tag_refs(), pos).or_else(|| find_span_at(doc.tag_defs(), pos));

        Ok(span.map(|s| {
            let mut range = s.range;
            range.start.character += 1;
            range.end.character -= 1;
            lsp_types::PrepareRenameResponse::RangeWithPlaceholder {
                range,
                placeholder: s.name.clone(),
            }
        }))
    })();
    make_response(req, result)
}
