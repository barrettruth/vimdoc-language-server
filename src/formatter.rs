use serde::Deserialize;

use crate::parser::{Document, LineKind, SepKind};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ReflowMode {
    #[default]
    Always,
    OnlyIfTooLong,
    Never,
}

pub struct FormatOptions {
    pub line_width: usize,
    pub reflow: ReflowMode,
    pub normalize_spacing: bool,
}

impl Default for FormatOptions {
    fn default() -> Self {
        Self {
            line_width: 78,
            reflow: ReflowMode::default(),
            normalize_spacing: false,
        }
    }
}

pub(crate) fn display_width(s: &str) -> usize {
    s.chars().count()
}

#[must_use]
pub fn format_document(text: &str, opts: &FormatOptions) -> String {
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
                out.push(ch.to_string().repeat(opts.line_width));
                i += 1;
            }
            LineKind::CodeBody => {
                out.push(raw_lines[i].to_string());
                i += 1;
            }
            LineKind::ListItem => {
                out.push(raw_lines[i].trim_end().to_string());
                i += 1;
            }
            LineKind::Text => {
                if pl.tag_defs.is_empty() {
                    let indent = leading_whitespace(raw_lines[i]);
                    if indent.is_empty() {
                        if raw_lines[i].contains('\t') || is_pipe_table_row(raw_lines[i]) {
                            out.push(raw_lines[i].trim_end().to_string());
                            i += 1;
                        } else {
                            match opts.reflow {
                                ReflowMode::Never => {
                                    out.push(raw_lines[i].trim_end().to_string());
                                    i += 1;
                                }
                                ReflowMode::Always | ReflowMode::OnlyIfTooLong => {
                                    i = emit_prose_paragraph(
                                        &raw_lines, &doc, opts, i, n, &mut out,
                                    );
                                }
                            }
                        }
                    } else {
                        out.push(raw_lines[i].trim_end().to_string());
                        i += 1;
                    }
                } else {
                    out.push(format_heading(raw_lines[i], pl, opts.line_width));
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

fn emit_prose_paragraph(
    raw_lines: &[&str],
    doc: &Document,
    opts: &FormatOptions,
    start: usize,
    n: usize,
    out: &mut Vec<String>,
) -> usize {
    let mut j = start;
    while j < n
        && doc.lines[j].kind == LineKind::Text
        && doc.lines[j].tag_defs.is_empty()
        && leading_whitespace(raw_lines[j]).is_empty()
        && !raw_lines[j].contains('\t')
        && !is_pipe_table_row(raw_lines[j])
    {
        j += 1;
    }
    if opts.reflow == ReflowMode::OnlyIfTooLong
        && raw_lines[start..j]
            .iter()
            .all(|l| display_width(l) <= opts.line_width)
    {
        for line in &raw_lines[start..j] {
            out.push(line.trim_end().to_string());
        }
        return j;
    }
    let num_lines = j - start;
    let mut tokens: Vec<(&str, usize)> = Vec::new();
    let mut pending_space: usize = 0;
    for (idx, line) in raw_lines[start..j].iter().enumerate() {
        let is_last_line = idx == num_lines - 1;
        let line_tokens = split_words_with_spacing(line);
        let len = line_tokens.len();
        for (k, (word, trailing)) in line_tokens.into_iter().enumerate() {
            tokens.push((word, pending_space));
            pending_space = if opts.normalize_spacing || (!is_last_line && k == len - 1) {
                1
            } else {
                trailing
            };
        }
    }
    reflow_tokens(&tokens, opts.line_width, out);
    j
}

pub(crate) fn utf16_col_to_byte(s: &str, utf16: usize) -> usize {
    let mut col = 0usize;
    for (byte_pos, ch) in s.char_indices() {
        if col >= utf16 {
            return byte_pos;
        }
        col += ch.len_utf16();
    }
    s.len()
}

#[allow(clippy::cast_possible_truncation)]
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

    if display_width(left) + 1 + display_width(&right) >= line_width {
        return format!("{left} {right}");
    }

    let spaces = line_width - display_width(left) - display_width(&right);
    format!("{left}{}{right}", " ".repeat(spaces))
}

fn leading_whitespace(s: &str) -> &str {
    let trimmed = s.trim_start_matches([' ', '\t']);
    &s[..s.len() - trimmed.len()]
}

fn is_pipe_table_row(s: &str) -> bool {
    let trimmed = s.trim_end();
    trimmed.starts_with('|') && trimmed.len() > 1 && trimmed.ends_with('|')
}

fn split_words_with_spacing(s: &str) -> Vec<(&str, usize)> {
    let bytes = s.as_bytes();
    let mut result = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        while i < bytes.len() && (bytes[i] == b' ' || bytes[i] == b'\t') {
            i += 1;
        }
        if i >= bytes.len() {
            break;
        }
        let start = i;
        while i < bytes.len() && bytes[i] != b' ' && bytes[i] != b'\t' {
            i += 1;
        }
        let word = &s[start..i];
        let sp_start = i;
        while i < bytes.len() && (bytes[i] == b' ' || bytes[i] == b'\t') {
            i += 1;
        }
        result.push((word, i - sp_start));
    }
    result
}

fn reflow_tokens(tokens: &[(&str, usize)], line_width: usize, out: &mut Vec<String>) {
    if tokens.is_empty() {
        return;
    }
    let mut line = String::new();
    for (word, pre_space) in tokens {
        let pre_space = *pre_space;
        if line.is_empty() {
            line.push_str(word);
        } else if display_width(&line) + 1 + display_width(word) <= line_width {
            let sp = pre_space.min(line_width - display_width(&line) - display_width(word));
            for _ in 0..sp {
                line.push(' ');
            }
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
        let result = format_document(&"=".repeat(40), &FormatOptions::default());
        assert_eq!(result.trim_end(), &"=".repeat(78));
    }

    #[test]
    fn normalizes_minor_separator() {
        let result = format_document(&"-".repeat(40), &FormatOptions::default());
        assert_eq!(result.trim_end(), &"-".repeat(78));
    }

    #[test]
    fn reflows_prose() {
        let input = "word1 word2\nword3 word4";
        let result = format_document(input, &FormatOptions::default());
        assert_eq!(result, "word1 word2 word3 word4");
    }

    #[test]
    fn preserves_code_block() {
        let input = "example >\n    indented code\n<\nafter";
        let result = format_document(input, &FormatOptions::default());
        assert!(result.contains("    indented code"));
    }

    #[test]
    fn idempotent_separator() {
        let input = format!("{}\n", "=".repeat(78));
        let once = format_document(&input, &FormatOptions::default());
        let twice = format_document(&once, &FormatOptions::default());
        assert_eq!(once, twice);
    }

    #[test]
    fn aligns_heading_tag_right() {
        let opts = FormatOptions {
            line_width: 30,
            ..Default::default()
        };
        let result = format_document("Introduction *intro*\n", &opts);
        assert_eq!(result, "Introduction           *intro*\n");
    }

    #[test]
    fn heading_tag_at_column_zero_preserved() {
        let opts = FormatOptions {
            line_width: 30,
            ..Default::default()
        };
        let result = format_document("*intro* Introduction\n", &opts);
        assert_eq!(result, "*intro* Introduction\n");
    }

    #[test]
    fn preserves_code_fence_with_language() {
        let input = "prose\n>lua\n    code()\n<\nafter\n";
        let result = format_document(input, &FormatOptions::default());
        assert_eq!(result, input);
    }

    #[test]
    fn prose_not_merged_into_code_fence() {
        let input = "This is prose.\n>lua\n    code()\n<\n";
        let result = format_document(input, &FormatOptions::default());
        assert_eq!(result, input);
    }

    #[test]
    fn heading_tag_fallback_when_line_too_long() {
        let opts = FormatOptions {
            line_width: 20,
            ..Default::default()
        };
        let result = format_document("A very long heading        *tag*\n", &opts);
        assert_eq!(result, "A very long heading *tag*\n");
    }

    #[test]
    fn list_items_not_merged() {
        let input = "- item 1\n- item 2\n- item 3\n";
        let result = format_document(input, &FormatOptions::default());
        assert_eq!(result, input);
    }

    #[test]
    fn list_item_not_merged_with_preceding_prose() {
        let input = "Prose intro.\n- Item.\n";
        let result = format_document(input, &FormatOptions::default());
        assert_eq!(result, input);
    }

    #[test]
    fn asterisk_list_item_preserved() {
        let input = "* item text\n";
        let result = format_document(input, &FormatOptions::default());
        assert_eq!(result, input);
    }

    #[test]
    fn tab_command_ref_preserved() {
        let input = "CTRL-V\t\tInsert next non-digit literally.\n";
        let result = format_document(input, &FormatOptions::default());
        assert_eq!(result, input);
    }

    #[test]
    fn tab_line_not_merged_with_adjacent_prose() {
        let input = "Prose before.\nCTRL-V\t\tDescription.\nProse after.\n";
        let result = format_document(input, &FormatOptions::default());
        assert_eq!(result, input);
    }

    #[test]
    fn tab_idempotent() {
        let input = "CTRL-V\t\tInsert next non-digit literally.\n\t\tcontinuation line.\n";
        let once = format_document(input, &FormatOptions::default());
        let twice = format_document(&once, &FormatOptions::default());
        assert_eq!(once, twice);
    }

    #[test]
    fn ordered_list_items_not_merged() {
        let input = "1. First item\n2. Second item\n3. Third item\n";
        let result = format_document(input, &FormatOptions::default());
        assert_eq!(result, input);
    }

    #[test]
    fn ordered_list_not_merged_with_prose() {
        let input = "Intro text.\n1. First item\n2. Second item\n";
        let result = format_document(input, &FormatOptions::default());
        assert_eq!(result, input);
    }

    #[test]
    fn double_space_after_period_preserved() {
        let input = "First sentence.  Second sentence.\n";
        let result = format_document(input, &FormatOptions::default());
        assert_eq!(result, input);
    }

    #[test]
    fn double_space_preserved_during_reflow() {
        let input = "The quick brown fox.  The lazy dog sat.\n";
        let result = format_document(input, &FormatOptions::default());
        assert_eq!(result, input);
    }

    #[test]
    fn line_break_joins_with_single_space() {
        let input = "word1 word2\nword3 word4";
        let result = format_document(input, &FormatOptions::default());
        assert_eq!(result, "word1 word2 word3 word4");
    }

    #[test]
    fn multi_space_internal_preserved() {
        let input = "Vi      \"the original\".\n";
        let result = format_document(input, &FormatOptions::default());
        assert_eq!(result, input);
    }

    #[test]
    fn reflow_never_preserves_line_breaks() {
        let input = "word1 word2\nword3 word4";
        let opts = FormatOptions {
            reflow: ReflowMode::Never,
            ..Default::default()
        };
        let result = format_document(input, &opts);
        assert_eq!(result, input);
    }

    #[test]
    fn reflow_only_if_too_long_skips_short_paragraph() {
        let input = "Short line.\nAnother short line.\n";
        let opts = FormatOptions {
            reflow: ReflowMode::OnlyIfTooLong,
            ..Default::default()
        };
        let result = format_document(input, &opts);
        assert_eq!(result, input);
    }

    #[test]
    fn reflow_only_if_too_long_reflows_overlong_paragraph() {
        let input = format!("{}\n", "word ".repeat(20).trim_end());
        let opts = FormatOptions {
            reflow: ReflowMode::OnlyIfTooLong,
            ..Default::default()
        };
        let result = format_document(&input, &opts);
        assert_ne!(result, input);
        assert!(result.lines().all(|l| l.len() <= 78));
    }

    #[test]
    fn pipe_table_padded_preserved() {
        let input = "\
| Command  | List           |
| -------- | -------------- |
| `files`  | find or fd     |
| `buffers` | open buffers  |
";
        let result = format_document(input, &FormatOptions::default());
        assert_eq!(result, input);
    }

    #[test]
    fn pipe_table_tight_preserved() {
        let input = "\
|Prefix     |Behavior                           |
|-----------|-----------------------------------|
|`no prefix`|Files                              |
|`$`        |Buffers                            |
";
        let result = format_document(input, &FormatOptions::default());
        assert_eq!(result, input);
    }

    #[test]
    fn pipe_table_not_merged_with_adjacent_prose() {
        let input = "\
Prose before the table.

| Command  | List       |
| -------- | ---------- |
| `files`  | find or fd |

Prose after the table.
";
        let result = format_document(input, &FormatOptions::default());
        assert_eq!(result, input);
    }

    #[test]
    fn pipe_table_idempotent() {
        let input = "\
| Key       | Command           | Key       | Command           |
| ----------| ------------------| ----------| ------------------|
| `<C-\\>`    | buffers           | `<C-p>`     | files             |
";
        let once = format_document(input, &FormatOptions::default());
        let twice = format_document(&once, &FormatOptions::default());
        assert_eq!(once, twice);
    }

    #[test]
    fn pipe_table_prose_after_not_blocked() {
        let input = "\
| Col | Val |

word1 word2
word3 word4
";
        let result = format_document(input, &FormatOptions::default());
        assert!(result.contains("| Col | Val |"));
        assert!(result.contains("word1 word2 word3 word4"));
    }

    #[test]
    fn normalize_spacing_collapses_double_space() {
        let input = "First sentence.  Second sentence.\n";
        let opts = FormatOptions {
            normalize_spacing: true,
            ..Default::default()
        };
        let result = format_document(input, &opts);
        assert_eq!(result, "First sentence. Second sentence.\n");
    }
}
