use anyhow::{Result, anyhow};
use lsp_server::Response;
use lsp_types::{DocumentLink, Location};

use crate::server::make_response;
use crate::store::Store;
use crate::tags::TagIndex;

#[must_use]
pub fn handle_document_link(
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
