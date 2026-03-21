use std::collections::HashMap;

use lsp_types::{Diagnostic, DiagnosticSeverity, NumberOrString, Position, Range, Uri};
use serde::Deserialize;

use crate::parser::Document;
use crate::tags::TagIndex;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DiagnosticLevel {
    Error,
    Warning,
    Information,
    Hint,
    Off,
}

fn default_level(code: &str) -> DiagnosticLevel {
    match code {
        "duplicate-tag" => DiagnosticLevel::Error,
        "missing-modeline" => DiagnosticLevel::Hint,
        _ => DiagnosticLevel::Warning,
    }
}

fn apply_level(
    mut diag: Diagnostic,
    levels: &HashMap<String, DiagnosticLevel>,
) -> Option<Diagnostic> {
    let code = match &diag.code {
        Some(NumberOrString::String(s)) => s.clone(),
        _ => return Some(diag),
    };
    let level = levels
        .get(&code)
        .copied()
        .unwrap_or_else(|| default_level(&code));
    diag.severity = Some(match level {
        DiagnosticLevel::Off => return None,
        DiagnosticLevel::Error => DiagnosticSeverity::ERROR,
        DiagnosticLevel::Warning => DiagnosticSeverity::WARNING,
        DiagnosticLevel::Information => DiagnosticSeverity::INFORMATION,
        DiagnosticLevel::Hint => DiagnosticSeverity::HINT,
    });
    Some(diag)
}

#[must_use]
#[allow(clippy::cast_possible_truncation, clippy::implicit_hasher)]
pub fn compute(
    doc: &Document,
    tag_index: &TagIndex,
    uri: &Uri,
    levels: &HashMap<String, DiagnosticLevel>,
) -> Vec<Diagnostic> {
    let mut raw_diags: Vec<Diagnostic> = Vec::new();
    let mut defined: HashMap<&str, Vec<Range>> = HashMap::new();

    for span in doc.tag_defs() {
        defined
            .entry(span.name.as_str())
            .or_default()
            .push(span.range);
    }

    for (name, ranges) in &defined {
        if ranges.len() > 1 {
            for &range in ranges {
                raw_diags.push(Diagnostic {
                    range,
                    severity: Some(DiagnosticSeverity::ERROR),
                    code: Some(NumberOrString::String("duplicate-tag".into())),
                    message: format!("duplicate tag definition: *{name}*"),
                    source: Some("vimdoc".into()),
                    ..Default::default()
                });
            }
        }
        if tag_index.has_definition_in_other_file(name, uri) {
            for &range in ranges {
                raw_diags.push(Diagnostic {
                    range,
                    severity: Some(DiagnosticSeverity::ERROR),
                    code: Some(NumberOrString::String("duplicate-tag".into())),
                    message: format!("tag `*{name}*` is also defined in another file"),
                    source: Some("vimdoc".into()),
                    ..Default::default()
                });
            }
        }
    }

    for span in doc.tag_refs() {
        if !defined.contains_key(span.name.as_str()) && !tag_index.contains(&span.name) {
            raw_diags.push(Diagnostic {
                range: span.range,
                severity: Some(DiagnosticSeverity::WARNING),
                code: Some(NumberOrString::String("unresolved-tag".into())),
                message: format!("unresolved tag reference: |{}|", span.name),
                source: Some("vimdoc".into()),
                ..Default::default()
            });
        }
    }

    if !doc.lines.is_empty() && !doc.has_modeline {
        let last_line = doc.lines.len().saturating_sub(1) as u32;
        raw_diags.push(Diagnostic {
            range: Range {
                start: Position {
                    line: last_line,
                    character: 0,
                },
                end: Position {
                    line: last_line,
                    character: 0,
                },
            },
            severity: Some(DiagnosticSeverity::HINT),
            code: Some(NumberOrString::String("missing-modeline".into())),
            message: "missing modeline; add ' vim:tw=78:ts=8:ft=help:norl:' to the last line"
                .into(),
            source: Some("vimdoc".into()),
            ..Default::default()
        });
    }

    raw_diags
        .into_iter()
        .filter_map(|d| apply_level(d, levels))
        .collect()
}
