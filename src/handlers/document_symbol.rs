use anyhow::{Result, anyhow};
use lsp_server::Response;
use lsp_types::{DocumentSymbol, DocumentSymbolResponse, SymbolKind};

use crate::server::make_response;
use crate::store::Store;

#[must_use]
pub fn handle_document_symbol(req: &lsp_server::Request, store: &Store) -> Response {
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
