use anyhow::{Result, anyhow};
use lsp_server::Response;
use lsp_types::{GotoDefinitionResponse, Location};

use crate::server::make_response;
use crate::shared::tag_name_at;
use crate::store::Store;
use crate::tags::TagIndex;

#[must_use]
pub fn handle_goto_definition(
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
