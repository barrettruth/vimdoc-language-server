use std::collections::HashMap;

use anyhow::{Result, anyhow};
use lsp_server::Response;
use lsp_types::{TextEdit, Uri, WorkspaceEdit};

use crate::parser::Document;
use crate::server::make_response;
use crate::shared::find_span_at;
use crate::store::Store;
use crate::tags::TagIndex;

#[must_use]
#[allow(clippy::similar_names)]
pub fn handle_rename(req: &lsp_server::Request, store: &Store, tag_index: &TagIndex) -> Response {
    let result = (|| -> Result<Option<WorkspaceEdit>> {
        let params: lsp_types::RenameParams = serde_json::from_value(req.params.clone())?;
        let uri = params.text_document_position.text_document.uri;
        let pos = params.text_document_position.position;
        let new_name = params.new_name;
        let (_text, doc) = store.get(&uri).ok_or_else(|| anyhow!("unknown uri"))?;

        if new_name.is_empty() || new_name.chars().any(char::is_whitespace) {
            return Err(anyhow!(
                "invalid tag name: must be non-empty with no whitespace"
            ));
        }

        if tag_index.workspace_defs(&new_name).is_some() {
            return Err(anyhow!("tag *{new_name}* already exists"));
        }

        let span = find_span_at(doc.tag_refs(), pos).or_else(|| find_span_at(doc.tag_defs(), pos));

        let Some(span) = span else {
            return Ok(None);
        };
        let old_name = span.name.clone();

        let new_def_text = format!("*{new_name}*");
        let new_ref_text = format!("|{new_name}|");

        #[allow(clippy::mutable_key_type)]
        let mut changes: HashMap<Uri, Vec<TextEdit>> = HashMap::new();

        collect_rename_edits(
            doc,
            &uri,
            &old_name,
            &new_def_text,
            &new_ref_text,
            &mut changes,
        );

        for (ws_uri, ws_doc) in tag_index.workspace_docs() {
            if *ws_uri != uri {
                collect_rename_edits(
                    ws_doc,
                    ws_uri,
                    &old_name,
                    &new_def_text,
                    &new_ref_text,
                    &mut changes,
                );
            }
        }

        Ok(Some(WorkspaceEdit {
            changes: Some(changes),
            ..WorkspaceEdit::default()
        }))
    })();
    make_response(req, result)
}

#[allow(clippy::mutable_key_type, clippy::similar_names)]
fn collect_rename_edits(
    doc: &Document,
    uri: &Uri,
    old_name: &str,
    new_def_text: &str,
    new_ref_text: &str,
    changes: &mut HashMap<Uri, Vec<TextEdit>>,
) {
    let edits: Vec<TextEdit> = doc
        .tag_defs()
        .filter(|d| d.name == old_name)
        .map(|d| TextEdit {
            range: d.range,
            new_text: new_def_text.to_string(),
        })
        .chain(
            doc.tag_refs()
                .filter(|r| r.name == old_name)
                .map(|r| TextEdit {
                    range: r.range,
                    new_text: new_ref_text.to_string(),
                }),
        )
        .collect();
    if !edits.is_empty() {
        changes.entry(uri.clone()).or_default().extend(edits);
    }
}
