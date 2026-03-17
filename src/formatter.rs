use crate::parser::{Document, LineKind, SepKind};

#[must_use]
pub fn format_document(text: &str, line_width: usize) -> String {
    let doc = Document::parse(text);
    let raw_lines: Vec<&str> = text.lines().collect();
    let n = doc.lines.len();
    let mut out = Vec::with_capacity(n);
    let mut i = 0;

    while i < n {
        let pl = &doc.lines[i];
        match &pl.kind {
            LineKind::Blank => {
                out.push(String::new());
                i += 1;
            }
            LineKind::Separator(kind) => {
                let ch = match kind {
                    SepKind::Major => '=',
                    SepKind::Minor => '-',
                };
                out.push(ch.to_string().repeat(line_width));
                i += 1;
            }
            LineKind::CodeBody => {
                out.push(raw_lines[i].to_string());
                i += 1;
            }
            LineKind::Text => {
                if pl.tag_defs.is_empty() {
                    let indent = leading_whitespace(raw_lines[i]);
                    if indent.is_empty() {
                        let mut j = i;
                        while j < n
                            && doc.lines[j].kind == LineKind::Text
                            && doc.lines[j].tag_defs.is_empty()
                            && leading_whitespace(raw_lines[j]).is_empty()
                        {
                            j += 1;
                        }
                        let words: Vec<&str> = raw_lines[i..j]
                            .iter()
                            .flat_map(|l| l.split_whitespace())
                            .collect();
                        reflow_words(&words, line_width, &mut out);
                        i = j;
                    } else {
                        out.push(raw_lines[i].trim_end().to_string());
                        i += 1;
                    }
                } else {
                    out.push(format_heading(raw_lines[i], pl, line_width));
                    i += 1;
                }
            }
        }
    }

    let mut result = out.join("\n");
    if text.ends_with('\n') {
        result.push('\n');
    }
    result
}

fn utf16_col_to_byte(s: &str, utf16: usize) -> usize {
    let mut col = 0usize;
    for (byte_pos, ch) in s.char_indices() {
        if col >= utf16 {
            return byte_pos;
        }
        col += ch.len_utf16();
    }
    s.len()
}

fn format_heading(raw: &str, pl: &crate::parser::ParsedLine, line_width: usize) -> String {
    let tag_start_utf16 = pl.tag_defs[0].range.start.character as usize;
    let tag_start = utf16_col_to_byte(raw, tag_start_utf16);

    if tag_start == 0 {
        return raw.trim_end().to_string();
    }

    let left = raw[..tag_start].trim_end();
    let right: String = pl
        .tag_defs
        .iter()
        .map(|s| format!("*{}*", s.name))
        .collect::<Vec<_>>()
        .join(" ");

    if left.len() + 1 + right.len() >= line_width {
        return format!("{left} {right}");
    }

    let spaces = line_width - left.len() - right.len();
    format!("{left}{}{right}", " ".repeat(spaces))
}

fn leading_whitespace(s: &str) -> &str {
    let trimmed = s.trim_start_matches([' ', '\t']);
    &s[..s.len() - trimmed.len()]
}

fn reflow_words(words: &[&str], line_width: usize, out: &mut Vec<String>) {
    if words.is_empty() {
        return;
    }
    let mut line = String::new();
    for word in words {
        if line.is_empty() {
            line.push_str(word);
        } else if line.len() + 1 + word.len() <= line_width {
            line.push(' ');
            line.push_str(word);
        } else {
            out.push(line);
            line = word.to_string();
        }
    }
    if !line.is_empty() {
        out.push(line);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_major_separator() {
        let result = format_document(&"=".repeat(40), 78);
        assert_eq!(result.trim_end(), &"=".repeat(78));
    }

    #[test]
    fn normalizes_minor_separator() {
        let result = format_document(&"-".repeat(40), 78);
        assert_eq!(result.trim_end(), &"-".repeat(78));
    }

    #[test]
    fn reflows_prose() {
        let input = "word1 word2\nword3 word4";
        let result = format_document(input, 78);
        assert_eq!(result, "word1 word2 word3 word4");
    }

    #[test]
    fn preserves_code_block() {
        let input = "example >\n    indented code\n<\nafter";
        let result = format_document(input, 78);
        assert!(result.contains("    indented code"));
    }

    #[test]
    fn idempotent_separator() {
        let input = format!("{}\n", "=".repeat(78));
        let once = format_document(&input, 78);
        let twice = format_document(&once, 78);
        assert_eq!(once, twice);
    }

    #[test]
    fn aligns_heading_tag_right() {
        let result = format_document("Introduction *intro*\n", 30);
        assert_eq!(result, "Introduction           *intro*\n");
    }

    #[test]
    fn heading_tag_at_column_zero_preserved() {
        let result = format_document("*intro* Introduction\n", 30);
        assert_eq!(result, "*intro* Introduction\n");
    }

    #[test]
    fn heading_tag_fallback_when_line_too_long() {
        let result = format_document("A very long heading        *tag*\n", 20);
        assert_eq!(result, "A very long heading *tag*\n");
    }
}
