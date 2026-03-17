use lsp_types::{Position, Range};

use crate::parser::{Document, Span};

#[must_use]
pub fn tag_name_at(doc: &Document, pos: Position) -> Option<String> {
    find_span_at(doc.tag_refs(), pos)
        .or_else(|| find_span_at(doc.tag_defs(), pos))
        .map(|s| s.name.clone())
}

pub fn find_span_at<'a>(
    mut spans: impl Iterator<Item = &'a Span>,
    pos: Position,
) -> Option<&'a Span> {
    spans.find(|s| position_in_range(pos, s.range))
}

fn position_in_range(pos: Position, range: Range) -> bool {
    let line = pos.line;
    let ch = pos.character;
    line == range.start.line && ch >= range.start.character && ch < range.end.character
}
