use std::collections::HashMap;

use anyhow::Result;
use lsp_server::Response;
use lsp_types::{
    CodeAction, CodeActionKind, CodeActionOrCommand, CodeActionParams, Position, Range, TextEdit,
    Uri, WorkspaceEdit,
};

use crate::formatter::{self, FormatOptions, utf16_col_to_byte};
use crate::parser::{Document, LineKind, SepKind, byte_offset_to_utf16};
use crate::server::{Config, make_response, text_end_position};
use crate::store::Store;
use crate::tags::TagIndex;

#[allow(clippy::mutable_key_type)]
fn make_action(
    title: &str,
    kind: CodeActionKind,
    uri: &Uri,
    edit: TextEdit,
) -> CodeActionOrCommand {
    CodeActionOrCommand::CodeAction(CodeAction {
        title: title.to_string(),
        kind: Some(kind),
        edit: Some(WorkspaceEdit {
            changes: Some(HashMap::from([(uri.clone(), vec![edit])])),
            ..Default::default()
        }),
        ..Default::default()
    })
}

fn find_block_range(doc: &Document, cursor_line: usize) -> Option<(usize, usize)> {
    let lines = &doc.lines;
    let n = lines.len();
    if cursor_line >= n {
        return None;
    }
    let line = &lines[cursor_line];
    match &line.kind {
        LineKind::Blank | LineKind::CodeBody => None,
        LineKind::Separator(_) => Some((cursor_line, cursor_line)),
        LineKind::Text if !line.tag_defs.is_empty() => Some((cursor_line, cursor_line)),
        LineKind::Text | LineKind::ListItem => {
            let mut start = cursor_line;
            while start > 0 {
                let prev = &lines[start - 1];
                match &prev.kind {
                    LineKind::Text if prev.tag_defs.is_empty() => start -= 1,
                    LineKind::ListItem => start -= 1,
                    _ => break,
                }
            }
            let mut end = cursor_line;
            while end + 1 < n {
                let next = &lines[end + 1];
                match &next.kind {
                    LineKind::Text if next.tag_defs.is_empty() => end += 1,
                    LineKind::ListItem => end += 1,
                    _ => break,
                }
            }
            Some((start, end))
        }
    }
}

#[allow(clippy::cast_possible_truncation)]
fn collect_format_action(
    actions: &mut Vec<CodeActionOrCommand>,
    doc: &Document,
    raw_lines: &[&str],
    cursor_line: usize,
    config: &Config,
    uri: &Uri,
) {
    let Some((start, end)) = find_block_range(doc, cursor_line) else {
        return;
    };
    let sub_text = raw_lines[start..=end].join("\n");
    let opts = FormatOptions {
        line_width: config.line_width,
        reflow: config.reflow,
        normalize_spacing: config.normalize_spacing,
    };
    let new_text = formatter::format_document(&sub_text, &opts);
    if new_text == sub_text {
        return;
    }
    let range_end = text_end_position(raw_lines[end]);
    let edit = TextEdit {
        range: Range {
            start: Position {
                line: start as u32,
                character: 0,
            },
            end: Position {
                line: end as u32,
                character: range_end.character,
            },
        },
        new_text,
    };
    actions.push(make_action(
        "Format this block",
        CodeActionKind::SOURCE,
        uri,
        edit,
    ));
}

#[allow(clippy::cast_possible_truncation)]
fn collect_separator_convert(
    actions: &mut Vec<CodeActionOrCommand>,
    doc: &Document,
    raw_lines: &[&str],
    cursor_line: usize,
    config: &Config,
    uri: &Uri,
) {
    if cursor_line >= doc.lines.len() {
        return;
    }
    let end_char = text_end_position(raw_lines[cursor_line]).character;
    let range = Range {
        start: Position {
            line: cursor_line as u32,
            character: 0,
        },
        end: Position {
            line: cursor_line as u32,
            character: end_char,
        },
    };
    match &doc.lines[cursor_line].kind {
        LineKind::Separator(SepKind::Major) => {
            actions.push(make_action(
                "Convert to minor separator",
                CodeActionKind::REFACTOR,
                uri,
                TextEdit {
                    range,
                    new_text: "-".repeat(config.line_width),
                },
            ));
        }
        LineKind::Separator(SepKind::Minor) => {
            actions.push(make_action(
                "Convert to major separator",
                CodeActionKind::REFACTOR,
                uri,
                TextEdit {
                    range,
                    new_text: "=".repeat(config.line_width),
                },
            ));
        }
        _ => {}
    }
}

fn word_byte_range_at(raw: &str, byte_pos: usize) -> Option<(usize, usize)> {
    let bytes = raw.as_bytes();
    let n = bytes.len();
    if n == 0 {
        return None;
    }
    let at = byte_pos.min(n - 1);
    let is_word = |b: u8| !matches!(b, b' ' | b'\t' | b'|' | b'*');
    if !is_word(bytes[at]) {
        return None;
    }
    let mut start = byte_pos;
    while start > 0 && is_word(bytes[start - 1]) {
        start -= 1;
    }
    let mut end = byte_pos;
    while end < n && is_word(bytes[end]) {
        end += 1;
    }
    if start >= end {
        return None;
    }
    Some((start, end))
}

#[allow(clippy::cast_possible_truncation)]
fn collect_taglink_toggle(
    actions: &mut Vec<CodeActionOrCommand>,
    doc: &Document,
    raw_lines: &[&str],
    cursor_line: usize,
    cursor_char: u32,
    uri: &Uri,
    tag_index: &TagIndex,
) {
    if cursor_line >= doc.lines.len() {
        return;
    }
    let line = &doc.lines[cursor_line];
    let raw = raw_lines[cursor_line];

    for span in &line.tag_refs {
        if span.range.start.character <= cursor_char && cursor_char <= span.range.end.character {
            actions.push(make_action(
                "Remove taglink delimiters",
                CodeActionKind::REFACTOR,
                uri,
                TextEdit {
                    range: span.range,
                    new_text: span.name.clone(),
                },
            ));
            return;
        }
    }

    for span in &line.tag_defs {
        if span.range.start.character <= cursor_char && cursor_char <= span.range.end.character {
            return;
        }
    }

    let byte_pos = utf16_col_to_byte(raw, cursor_char as usize);
    let Some((wb_start, wb_end)) = word_byte_range_at(raw, byte_pos) else {
        return;
    };
    let word = &raw[wb_start..wb_end];
    if !tag_index.contains(word) {
        return;
    }
    let start_char = byte_offset_to_utf16(raw, wb_start);
    let end_char = byte_offset_to_utf16(raw, wb_end);
    actions.push(make_action(
        "Add taglink",
        CodeActionKind::REFACTOR,
        uri,
        TextEdit {
            range: Range {
                start: Position {
                    line: cursor_line as u32,
                    character: start_char,
                },
                end: Position {
                    line: cursor_line as u32,
                    character: end_char,
                },
            },
            new_text: format!("|{word}|"),
        },
    ));
}

struct TocEntry {
    title: String,
    tag: String,
}

fn collect_toc_entries(doc: &Document, raw_lines: &[&str]) -> Vec<TocEntry> {
    let mut entries = Vec::new();
    for (i, pl) in doc.lines.iter().enumerate() {
        if pl.kind != LineKind::Text || pl.tag_defs.is_empty() {
            continue;
        }
        let tag = &pl.tag_defs[0];
        let tag_start_byte = utf16_col_to_byte(raw_lines[i], tag.range.start.character as usize);
        if tag_start_byte == 0 {
            continue;
        }
        let title = raw_lines[i][..tag_start_byte].trim().to_string();
        if title.is_empty() {
            continue;
        }
        entries.push(TocEntry {
            title,
            tag: tag.name.clone(),
        });
    }
    entries
}

fn find_toc_insertion_line(doc: &Document) -> usize {
    for (i, pl) in doc.lines.iter().enumerate() {
        if matches!(pl.kind, LineKind::Separator(SepKind::Major)) {
            return i;
        }
    }
    doc.lines.len()
}

fn derive_contents_tag(uri: &Uri) -> String {
    let path = uri.as_str();
    let filename = path.rsplit('/').next().unwrap_or("doc");
    let stem = filename.rsplit_once('.').map_or(filename, |(s, _)| s);
    format!("{stem}-contents")
}

fn format_toc_header(left: &str, tag: &str, line_width: usize) -> String {
    let right = format!("*{tag}*");
    let used = left.len() + right.len();
    if line_width > used + 1 {
        format!(
            "{left}{}{right}",
            " ".repeat(line_width - left.len() - right.len())
        )
    } else {
        format!("{left} {right}")
    }
}

fn format_toc_entry(title: &str, tag: &str, line_width: usize) -> String {
    let indent = "    ";
    let tag_link = format!("|{tag}|");
    let used = indent.len() + title.len() + tag_link.len();
    if line_width > used + 3 {
        format!("{indent}{title}{}{tag_link}", ".".repeat(line_width - used))
    } else {
        format!("{indent}{title} {tag_link}")
    }
}

#[allow(clippy::cast_possible_truncation)]
fn collect_toc_action(
    actions: &mut Vec<CodeActionOrCommand>,
    doc: &Document,
    raw_lines: &[&str],
    text: &str,
    config: &Config,
    uri: &Uri,
) {
    let already_has_toc = doc.lines.iter().any(|pl| {
        pl.tag_defs
            .iter()
            .any(|t| t.name.ends_with("-contents") || t.name == "contents")
    });
    if already_has_toc {
        return;
    }

    let entries = collect_toc_entries(doc, raw_lines);
    if entries.len() < 2 {
        return;
    }

    let header = format_toc_header("CONTENTS", &derive_contents_tag(uri), config.line_width);
    let toc_text = entries
        .iter()
        .map(|e| format_toc_entry(&e.title, &e.tag, config.line_width))
        .collect::<Vec<_>>()
        .join("\n");

    let insertion_line = find_toc_insertion_line(doc);
    let edit = if insertion_line < raw_lines.len() {
        TextEdit {
            range: Range {
                start: Position {
                    line: insertion_line as u32,
                    character: 0,
                },
                end: Position {
                    line: insertion_line as u32,
                    character: 0,
                },
            },
            new_text: format!("{header}\n\n{toc_text}\n\n"),
        }
    } else {
        let end_pos = text_end_position(text);
        TextEdit {
            range: Range {
                start: end_pos,
                end: end_pos,
            },
            new_text: format!("\n\n{header}\n\n{toc_text}\n"),
        }
    };

    actions.push(make_action(
        "Generate table of contents",
        CodeActionKind::SOURCE,
        uri,
        edit,
    ));
}

#[must_use]
#[allow(clippy::cast_possible_truncation)]
#[allow(clippy::mutable_key_type)]
pub fn handle_code_action(
    req: &lsp_server::Request,
    store: &Store,
    config: &Config,
    tag_index: &TagIndex,
) -> Response {
    let result = (|| -> Result<Option<Vec<CodeActionOrCommand>>> {
        let params: CodeActionParams = serde_json::from_value(req.params.clone())?;
        let uri = params.text_document.uri;
        let Some((text, doc)) = store.get(&uri) else {
            return Ok(Some(vec![]));
        };
        let raw_lines: Vec<&str> = text.lines().collect();
        let cursor_line = params.range.start.line as usize;
        let cursor_char = params.range.start.character;

        let mut actions = Vec::new();
        collect_format_action(&mut actions, doc, &raw_lines, cursor_line, config, &uri);
        collect_separator_convert(&mut actions, doc, &raw_lines, cursor_line, config, &uri);
        collect_taglink_toggle(
            &mut actions,
            doc,
            &raw_lines,
            cursor_line,
            cursor_char,
            &uri,
            tag_index,
        );
        collect_toc_action(&mut actions, doc, &raw_lines, text, config, &uri);

        Ok(Some(actions))
    })();
    make_response(req, result)
}
