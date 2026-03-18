use anyhow::{Result, anyhow};
use lsp_server::Response;
use lsp_types::{Position, Range, TextEdit};

use crate::formatter::{self, FormatOptions};
use crate::server::{Config, make_response, text_end_position};
use crate::store::Store;

#[must_use]
pub fn handle_formatting(req: &lsp_server::Request, store: &Store, config: &Config) -> Response {
    let result = (|| -> Result<Option<Vec<TextEdit>>> {
        let params: lsp_types::DocumentFormattingParams =
            serde_json::from_value(req.params.clone())?;
        let uri = params.text_document.uri;
        let (text, _doc) = store.get(&uri).ok_or_else(|| anyhow!("unknown uri"))?;
        let new_text = formatter::format_document(
            text,
            &FormatOptions {
                line_width: config.line_width,
                reflow: config.reflow,
                normalize_spacing: config.normalize_spacing,
            },
        );
        if new_text == text {
            return Ok(None);
        }
        let end = text_end_position(text);
        Ok(Some(vec![TextEdit {
            range: Range {
                start: Position {
                    line: 0,
                    character: 0,
                },
                end,
            },
            new_text,
        }]))
    })();
    make_response(req, result)
}
