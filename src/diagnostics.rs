use std::collections::HashMap;

use lsp_types::{Diagnostic, DiagnosticSeverity, NumberOrString, Range};

use crate::parser::Document;

#[must_use]
pub fn compute(doc: &Document) -> Vec<Diagnostic> {
    let mut diags = Vec::new();
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
                diags.push(Diagnostic {
                    range,
                    severity: Some(DiagnosticSeverity::WARNING),
                    code: Some(NumberOrString::String("duplicate-tag".into())),
                    message: format!("duplicate tag definition: *{name}*"),
                    source: Some("vimdoc".into()),
                    ..Default::default()
                });
            }
        }
    }

    diags
}
