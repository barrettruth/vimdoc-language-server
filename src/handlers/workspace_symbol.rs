use anyhow::Result;
use lsp_server::Response;
use lsp_types::{Location, SymbolInformation, SymbolKind};

use crate::server::make_response;
use crate::tags::{TagEntry, TagIndex};

#[must_use]
pub fn handle_workspace_symbol(req: &lsp_server::Request, tag_index: &mut TagIndex) -> Response {
    let result = (|| -> Result<Option<Vec<SymbolInformation>>> {
        let params: lsp_types::WorkspaceSymbolParams = serde_json::from_value(req.params.clone())?;
        let query = params.query.to_lowercase();
        let mut symbols: Vec<SymbolInformation> = tag_index
            .workspace_entries()
            .filter(|(name, _)| query.is_empty() || name.to_lowercase().contains(&query))
            .map(|(name, entry)| make_symbol(name, entry))
            .collect();

        for (name, entry) in tag_index.resolve_external_entries() {
            if query.is_empty() || name.to_lowercase().contains(&query) {
                symbols.push(make_symbol(&name, &entry));
            }
        }

        Ok(Some(symbols))
    })();
    make_response(req, result)
}

#[allow(deprecated)]
fn make_symbol(name: &str, entry: &TagEntry) -> SymbolInformation {
    SymbolInformation {
        name: name.to_string(),
        kind: SymbolKind::KEY,
        location: Location {
            uri: entry.uri.clone(),
            range: entry.range,
        },
        tags: None,
        deprecated: None,
        container_name: None,
    }
}
