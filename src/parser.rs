use lsp_types::{Position, Range};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SepKind {
    Major,
    Minor,
}

#[derive(Debug, Clone)]
pub struct Span {
    pub name: String,
    pub range: Range,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LineKind {
    Blank,
    Separator(SepKind),
    CodeBody,
    ListItem,
    Text,
}

#[derive(Debug, Clone)]
pub struct ParsedLine {
    pub kind: LineKind,
    pub tag_defs: Vec<Span>,
    pub tag_refs: Vec<Span>,
}

#[derive(Debug, Default, Clone)]
pub struct Document {
    pub lines: Vec<ParsedLine>,
}

impl Document {
    #[must_use]
    #[allow(clippy::cast_possible_truncation)]
    pub fn parse(text: &str) -> Self {
        let mut lines = Vec::new();
        let mut in_code = false;
        for (idx, raw) in text.lines().enumerate() {
            lines.push(parse_line(idx as u32, raw, &mut in_code));
        }
        Document { lines }
    }

    pub fn tag_defs(&self) -> impl Iterator<Item = &Span> {
        self.lines.iter().flat_map(|l| l.tag_defs.iter())
    }

    pub fn tag_refs(&self) -> impl Iterator<Item = &Span> {
        self.lines.iter().flat_map(|l| l.tag_refs.iter())
    }
}

#[allow(clippy::similar_names)]
fn parse_line(line_num: u32, raw: &str, in_code: &mut bool) -> ParsedLine {
    let trimmed = raw.trim_end();

    if trimmed.is_empty() {
        return mk(LineKind::Blank, vec![], vec![]);
    }

    if *in_code {
        let ends_code = trimmed == "<" || (!raw.starts_with(' ') && !raw.starts_with('\t'));
        if ends_code {
            *in_code = false;
            if trimmed == "<" {
                return mk(LineKind::CodeBody, vec![], vec![]);
            }
        } else {
            return mk(LineKind::CodeBody, vec![], vec![]);
        }
    }

    if trimmed.len() >= 10 && trimmed.bytes().all(|b| b == b'=') {
        return mk(LineKind::Separator(SepKind::Major), vec![], vec![]);
    }
    if trimmed.len() >= 10 && trimmed.bytes().all(|b| b == b'-') {
        return mk(LineKind::Separator(SepKind::Minor), vec![], vec![]);
    }

    if is_fence_start(trimmed) {
        *in_code = true;
        return mk(LineKind::CodeBody, vec![], vec![]);
    }

    let (tag_defs, tag_refs) = scan_inline(line_num, raw);

    if trimmed.ends_with('>') && !trimmed.ends_with("->") {
        *in_code = true;
    }

    if raw.starts_with("- ") || raw.starts_with("* ") || raw.starts_with("• ") {
        return mk(LineKind::ListItem, tag_defs, tag_refs);
    }

    if tag_defs.is_empty() {
        let after_digits = raw.trim_start_matches(|c: char| c.is_ascii_digit());
        if after_digits.len() < raw.len() && after_digits.starts_with(". ") {
            return mk(LineKind::ListItem, tag_defs, tag_refs);
        }
    }

    mk(LineKind::Text, tag_defs, tag_refs)
}

#[allow(clippy::similar_names)]
fn mk(kind: LineKind, tag_defs: Vec<Span>, tag_refs: Vec<Span>) -> ParsedLine {
    ParsedLine {
        kind,
        tag_defs,
        tag_refs,
    }
}

fn is_fence_start(s: &str) -> bool {
    s.len() > 1 && s.starts_with('>') && s[1..].bytes().all(|b| b.is_ascii_alphabetic())
}

#[allow(clippy::similar_names)]
fn scan_inline(line_num: u32, raw: &str) -> (Vec<Span>, Vec<Span>) {
    let mut tag_defs = Vec::new();
    let mut tag_refs = Vec::new();
    let bytes = raw.as_bytes();
    let len = bytes.len();
    let mut i = 0;

    while i < len {
        match bytes[i] {
            b'*' => {
                if let Some((name, end)) = scan_delimited(raw, i + 1, b'*') {
                    tag_defs.push(make_span(raw, line_num, i, end, name));
                    i = end;
                } else {
                    i += 1;
                }
            }
            b'|' => {
                let at_boundary =
                    i == 0 || matches!(bytes[i - 1], b' ' | b'\t' | b'(' | b'[' | b'|');
                if at_boundary {
                    if let Some((name, end)) = scan_delimited(raw, i + 1, b'|') {
                        tag_refs.push(make_span(raw, line_num, i, end, name));
                        i = end;
                    } else {
                        i += 1;
                    }
                } else {
                    i += 1;
                }
            }
            b'`' => {
                let mut j = i + 1;
                while j < len && bytes[j] != b'`' {
                    j += 1;
                }
                i = j + 1;
            }
            _ => {
                i += 1;
            }
        }
    }

    (tag_defs, tag_refs)
}

#[allow(clippy::cast_possible_truncation)]
fn byte_offset_to_utf16(s: &str, byte_pos: usize) -> u32 {
    s[..byte_pos].chars().map(char::len_utf16).sum::<usize>() as u32
}

#[allow(clippy::cast_possible_truncation)]
fn make_span(raw: &str, line_num: u32, start: usize, end: usize, name: String) -> Span {
    Span {
        name,
        range: Range {
            start: Position {
                line: line_num,
                character: byte_offset_to_utf16(raw, start),
            },
            end: Position {
                line: line_num,
                character: byte_offset_to_utf16(raw, end),
            },
        },
    }
}

fn scan_delimited(raw: &str, start: usize, delim: u8) -> Option<(String, usize)> {
    let bytes = raw.as_bytes();
    let mut end = start;
    while end < bytes.len() {
        if bytes[end] == delim {
            break;
        }
        if bytes[end] == b' ' || bytes[end] == b'\t' {
            return None;
        }
        end += 1;
    }
    if end >= bytes.len() || end == start {
        return None;
    }
    Some((raw[start..end].to_string(), end + 1))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_tag_defs() {
        let doc = Document::parse("*my-tag* some text");
        assert_eq!(doc.tag_defs().count(), 1);
        assert_eq!(doc.tag_defs().next().unwrap().name, "my-tag");
    }

    #[test]
    fn detects_tag_refs() {
        let doc = Document::parse("see |my-tag| for details");
        assert_eq!(doc.tag_refs().count(), 1);
        assert_eq!(doc.tag_refs().next().unwrap().name, "my-tag");
    }

    #[test]
    fn detects_major_separator() {
        let doc = Document::parse(&"=".repeat(78));
        assert_eq!(doc.lines[0].kind, LineKind::Separator(SepKind::Major));
    }

    #[test]
    fn detects_minor_separator() {
        let doc = Document::parse(&"-".repeat(78));
        assert_eq!(doc.lines[0].kind, LineKind::Separator(SepKind::Minor));
    }

    #[test]
    fn code_block_body_is_verbatim() {
        let text = "example >\n    code line\n    another\n<\nnormal";
        let doc = Document::parse(text);
        assert_eq!(doc.lines[1].kind, LineKind::CodeBody);
        assert_eq!(doc.lines[2].kind, LineKind::CodeBody);
        assert_eq!(doc.lines[4].kind, LineKind::Text);
    }

    #[test]
    fn unindented_line_ends_code_block() {
        let text = "example >\n    code\n\nnormal";
        let doc = Document::parse(text);
        assert_eq!(doc.lines[1].kind, LineKind::CodeBody);
        assert_eq!(doc.lines[2].kind, LineKind::Blank);
        assert_eq!(doc.lines[3].kind, LineKind::Text);
    }

    #[test]
    fn blank_does_not_end_code_block() {
        let text = "example >\n    code\n\n    more code\n<\nnormal";
        let doc = Document::parse(text);
        assert_eq!(doc.lines[1].kind, LineKind::CodeBody);
        assert_eq!(doc.lines[2].kind, LineKind::Blank);
        assert_eq!(doc.lines[3].kind, LineKind::CodeBody);
        assert_eq!(doc.lines[4].kind, LineKind::CodeBody);
        assert_eq!(doc.lines[5].kind, LineKind::Text);
    }

    #[test]
    fn pipe_in_code_block_after_blank_not_scanned() {
        let text = "example >\n\n    code with |pipe|\n<";
        let doc = Document::parse(text);
        assert_eq!(doc.tag_refs().count(), 0);
    }

    #[test]
    fn pipe_mid_word_not_scanned_as_taglink() {
        let doc = Document::parse("string|fun()|nil");
        assert_eq!(doc.tag_refs().count(), 0);
    }

    #[test]
    fn pipe_after_comma_not_scanned_as_taglink() {
        let doc = Document::parse("value '+,-,+,|,+,-,+,|'");
        assert_eq!(doc.tag_refs().count(), 0);
    }

    #[test]
    fn pipe_after_backslash_not_scanned_as_taglink() {
        let doc = Document::parse(r"pattern \|alternative\|");
        assert_eq!(doc.tag_refs().count(), 0);
    }

    #[test]
    fn pipe_after_open_paren_is_taglink() {
        let doc = Document::parse("(see |my-tag|)");
        assert_eq!(doc.tag_refs().count(), 1);
        assert_eq!(doc.tag_refs().next().unwrap().name, "my-tag");
    }

    #[test]
    fn pipe_at_line_start_is_taglink() {
        let doc = Document::parse("|my-tag| description");
        assert_eq!(doc.tag_refs().count(), 1);
    }

    #[test]
    fn no_tag_with_space() {
        let doc = Document::parse("* not a tag *");
        assert_eq!(doc.tag_defs().count(), 0);
    }

    #[test]
    fn utf16_multibyte_before_tag_def() {
        let doc = Document::parse("日本語 *foo*");
        let span = doc.tag_defs().next().unwrap();
        assert_eq!(span.range.start.character, 4);
        assert_eq!(span.range.end.character, 9);
    }

    #[test]
    fn utf16_supplementary_plane_before_tag_ref() {
        let doc = Document::parse("𝄞 |bar|");
        let span = doc.tag_refs().next().unwrap();
        assert_eq!(span.range.start.character, 3);
        assert_eq!(span.range.end.character, 8);
    }

    #[test]
    fn code_fence_language_is_code_body() {
        let text = "prose\n>lua\n    code()\n<\nafter";
        let doc = Document::parse(text);
        assert_eq!(doc.lines[1].kind, LineKind::CodeBody);
        assert_eq!(doc.lines[2].kind, LineKind::CodeBody);
        assert_eq!(doc.lines[3].kind, LineKind::CodeBody);
        assert_eq!(doc.lines[4].kind, LineKind::Text);
    }

    #[test]
    fn code_fence_language_no_tags_scanned() {
        let text = ">vim\n    *not-a-tag*\n<";
        let doc = Document::parse(text);
        assert_eq!(doc.tag_defs().count(), 0);
    }

    #[test]
    fn utf16_ascii_unaffected() {
        let doc = Document::parse("hello *baz*");
        let span = doc.tag_defs().next().unwrap();
        assert_eq!(span.range.start.character, 6);
        assert_eq!(span.range.end.character, 11);
    }

    #[test]
    fn dash_list_item_is_list_item() {
        let doc = Document::parse("- item text");
        assert_eq!(doc.lines[0].kind, LineKind::ListItem);
    }

    #[test]
    fn asterisk_list_item_is_list_item() {
        let doc = Document::parse("* item text");
        assert_eq!(doc.lines[0].kind, LineKind::ListItem);
        assert_eq!(doc.tag_defs().count(), 0);
    }

    #[test]
    fn tag_def_not_mistaken_for_list_item() {
        let doc = Document::parse("*my-tag* some text");
        assert_eq!(doc.lines[0].kind, LineKind::Text);
        assert_eq!(doc.tag_defs().count(), 1);
    }

    #[test]
    fn separator_not_mistaken_for_list_item() {
        let doc = Document::parse(&"-".repeat(78));
        assert_eq!(doc.lines[0].kind, LineKind::Separator(SepKind::Minor));
    }

    #[test]
    fn ordered_list_item_is_list_item() {
        let doc = Document::parse("1. First item");
        assert_eq!(doc.lines[0].kind, LineKind::ListItem);
    }

    #[test]
    fn multi_digit_ordered_item_is_list_item() {
        let doc = Document::parse("42. Forty-second item");
        assert_eq!(doc.lines[0].kind, LineKind::ListItem);
    }

    #[test]
    fn version_number_not_list_item() {
        let doc = Document::parse("3.14 is approximately pi");
        assert_eq!(doc.lines[0].kind, LineKind::Text);
    }

    #[test]
    fn numbered_item_with_tag_not_list_item() {
        let doc = Document::parse("1. Introduction\t\t\t*intro*");
        assert_eq!(doc.lines[0].kind, LineKind::Text);
        assert_eq!(doc.tag_defs().count(), 1);
    }
}
