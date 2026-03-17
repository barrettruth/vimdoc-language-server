use anyhow::{Result, anyhow};
use lsp_server::Response;
use lsp_types::Location;

use crate::server::make_response;
use crate::shared::tag_name_at;
use crate::store::Store;
use crate::tags::TagIndex;

#[must_use]
pub fn handle_references(
    req: &lsp_server::Request,
    store: &Store,
    tag_index: &TagIndex,
) -> Response {
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
