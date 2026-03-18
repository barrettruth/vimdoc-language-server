use anyhow::{Result, anyhow};
use lsp_server::Response;
use lsp_types::{Position, Range, TextEdit};

use crate::formatter::{self, FormatOptions};
use crate::server::{Config, make_response, text_end_position};
use crate::store::Store;

#[must_use]
#[allow(clippy::cast_possible_truncation)]
pub fn handle_range_formatting(
    req: &lsp_server::Request,
    store: &Store,
    config: &Config,
) -> Response {
    let result = (|| -> Result<Option<Vec<TextEdit>>> {
        let params: lsp_types::DocumentRangeFormattingParams =
            serde_json::from_value(req.params.clone())?;
        let uri = params.text_document.uri;
        let (text, _doc) = store.get(&uri).ok_or_else(|| anyhow!("unknown uri"))?;
        let raw_lines: Vec<&str> = text.lines().collect();
        let n = raw_lines.len();

        let start_line = params.range.start.line as usize;
        let end_line = (params.range.end.line as usize).min(n.saturating_sub(1));

        if start_line >= n {
            return Ok(None);
        }

        let sub_text = raw_lines[start_line..=end_line].join("\n");
        let opts = FormatOptions {
            line_width: config.line_width,
            reflow: config.reflow,
            normalize_spacing: config.normalize_spacing,
        };
        let new_text = formatter::format_document(&sub_text, &opts);

        if new_text == sub_text {
            return Ok(None);
        }

        let range_end = text_end_position(raw_lines[end_line]);
        Ok(Some(vec![TextEdit {
            range: Range {
                start: Position {
                    line: start_line as u32,
                    character: 0,
                },
                end: Position {
                    line: end_line as u32,
                    character: range_end.character,
                },
            },
            new_text,
        }]))
    })();
    make_response(req, result)
}
