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
    Text,
}

#[derive(Debug, Clone)]
pub struct ParsedLine {
    pub kind: LineKind,
    pub tag_defs: Vec<Span>,
    pub tag_refs: Vec<Span>,
}

#[derive(Debug, Default)]
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
        *in_code = false;
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

    let (tag_defs, tag_refs) = scan_inline(line_num, raw);

    if trimmed.ends_with('>') && !trimmed.ends_with("->") {
        *in_code = true;
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
                    tag_defs.push(make_span(line_num, i, end, name));
                    i = end;
                } else {
                    i += 1;
                }
            }
            b'|' => {
                if let Some((name, end)) = scan_delimited(raw, i + 1, b'|') {
                    tag_refs.push(make_span(line_num, i, end, name));
                    i = end;
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
fn make_span(line_num: u32, start: usize, end: usize, name: String) -> Span {
    Span {
        name,
        range: Range {
            start: Position {
                line: line_num,
                character: start as u32,
            },
            end: Position {
                line: line_num,
                character: end as u32,
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
    fn blank_ends_code_block() {
        let text = "example >\n    code\n\nnormal";
        let doc = Document::parse(text);
        assert_eq!(doc.lines[1].kind, LineKind::CodeBody);
        assert_eq!(doc.lines[2].kind, LineKind::Blank);
        assert_eq!(doc.lines[3].kind, LineKind::Text);
    }

    #[test]
    fn no_tag_with_space() {
        let doc = Document::parse("* not a tag *");
        assert_eq!(doc.tag_defs().count(), 0);
    }
}
