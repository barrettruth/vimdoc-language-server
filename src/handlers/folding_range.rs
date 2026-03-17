use anyhow::{Result, anyhow};
use lsp_server::Response;
use lsp_types::{FoldingRange, FoldingRangeKind};

use crate::parser::LineKind;
use crate::server::make_response;
use crate::store::Store;

#[must_use]
pub fn handle_folding_range(req: &lsp_server::Request, store: &Store) -> Response {
    let result = (|| -> Result<Option<Vec<FoldingRange>>> {
        let params: lsp_types::FoldingRangeParams = serde_json::from_value(req.params.clone())?;
        let uri = params.text_document.uri;
        let (_text, doc) = store.get(&uri).ok_or_else(|| anyhow!("unknown uri"))?;

        let mut ranges = Vec::new();
        let lines = &doc.lines;
        let total = lines.len();

        let mut section_start: Option<u32> = None;
        let mut code_start: Option<u32> = None;

        #[allow(clippy::cast_possible_truncation)]
        for (i, line) in lines.iter().enumerate() {
            let line_num = i as u32;
            match &line.kind {
                LineKind::Separator(_) => {
                    if let Some(start) = section_start {
                        if line_num > start + 1 {
                            ranges.push(FoldingRange {
                                start_line: start,
                                start_character: None,
                                end_line: line_num - 1,
                                end_character: None,
                                kind: Some(FoldingRangeKind::Region),
                                collapsed_text: None,
                            });
                        }
                    }
                    section_start = Some(line_num);
                }
                LineKind::CodeBody => {
                    if code_start.is_none() {
                        code_start = Some(line_num);
                    }
                }
                _ => {
                    if let Some(start) = code_start.take() {
                        let end = line_num - 1;
                        if end > start {
                            ranges.push(FoldingRange {
                                start_line: start,
                                start_character: None,
                                end_line: end,
                                end_character: None,
                                kind: Some(FoldingRangeKind::Region),
                                collapsed_text: None,
                            });
                        }
                    }
                }
            }
        }

        #[allow(clippy::cast_possible_truncation)]
        {
            if let Some(start) = code_start {
                let end = (total - 1) as u32;
                if end > start {
                    ranges.push(FoldingRange {
                        start_line: start,
                        start_character: None,
                        end_line: end,
                        end_character: None,
                        kind: Some(FoldingRangeKind::Region),
                        collapsed_text: None,
                    });
                }
            }

            if let Some(start) = section_start {
                let end = (total - 1) as u32;
                if end > start {
                    ranges.push(FoldingRange {
                        start_line: start,
                        start_character: None,
                        end_line: end,
                        end_character: None,
                        kind: Some(FoldingRangeKind::Region),
                        collapsed_text: None,
                    });
                }
            }
        }

        Ok(Some(ranges))
    })();
    make_response(req, result)
}
