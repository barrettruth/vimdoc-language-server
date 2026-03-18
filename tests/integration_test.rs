use expect_test::expect;
use lsp_server::Request;
use lsp_types::{
    CodeActionOrCommand, CompletionResponse, DocumentHighlight, DocumentHighlightKind,
    DocumentSymbolResponse, FoldingRange, GotoDefinitionResponse, Location, NumberOrString,
    PrepareRenameResponse, TextEdit, Uri, WorkspaceEdit,
};
use serde_json::json;
use vimdoc_language_server::{
    diagnostics, formatter::ReflowMode, handlers, parser::Document, server::Config, store::Store,
    tags::TagIndex,
};

#[test]
fn document_symbol_returns_tag_defs() {
    let mut store = Store::default();
    let uri: Uri = "file:///test.txt".parse().unwrap();
    store.open(uri.clone(), "*foo* heading\n*bar* other\n".into());

    let req = Request {
        id: 1.into(),
        method: "textDocument/documentSymbol".into(),
        params: json!({
            "textDocument": { "uri": uri.as_str() }
        }),
    };

    let resp = handlers::handle_document_symbol(&req, &store);
    let result: DocumentSymbolResponse = serde_json::from_value(resp.result.unwrap()).unwrap();

    let names: Vec<&str> = match &result {
        DocumentSymbolResponse::Nested(symbols) => {
            symbols.iter().map(|s| s.name.as_str()).collect()
        }
        DocumentSymbolResponse::Flat(_) => panic!("expected nested response"),
    };

    let expected = expect![[r#"
        [
          "foo",
          "bar"
        ]"#]];
    expected.assert_eq(&serde_json::to_string_pretty(&names).unwrap());
}

mod completion {
    use super::*;

    #[test]
    fn inside_taglink_returns_items() {
        let mut store = Store::default();
        let uri: Uri = "file:///test.txt".parse().unwrap();
        store.open(uri.clone(), "*foo* heading\n|".into());

        let req = Request {
            id: 1.into(),
            method: "textDocument/completion".into(),
            params: json!({
                "textDocument": { "uri": uri.as_str() },
                "position": { "line": 1, "character": 1 }
            }),
        };

        let tag_index = TagIndex::default();
        let resp = handlers::handle_completion(&req, &store, &tag_index);
        let result: CompletionResponse = serde_json::from_value(resp.result.unwrap()).unwrap();

        let labels: Vec<&str> = match &result {
            CompletionResponse::Array(items) => items.iter().map(|i| i.label.as_str()).collect(),
            CompletionResponse::List(_) => panic!("expected array"),
        };

        let expected = expect![[r#"
            [
              "foo"
            ]"#]];
        expected.assert_eq(&serde_json::to_string_pretty(&labels).unwrap());
    }

    #[test]
    fn multibyte_before_taglink_does_not_panic() {
        let mut store = Store::default();
        let uri: Uri = "file:///test.txt".parse().unwrap();
        store.open(uri.clone(), "*foo* heading\n😀|".into());

        let req = Request {
            id: 1.into(),
            method: "textDocument/completion".into(),
            params: json!({
                "textDocument": { "uri": uri.as_str() },
                "position": { "line": 1, "character": 3 }
            }),
        };

        let tag_index = TagIndex::default();
        let resp = handlers::handle_completion(&req, &store, &tag_index);
        let result: CompletionResponse = serde_json::from_value(resp.result.unwrap()).unwrap();

        let labels: Vec<&str> = match &result {
            CompletionResponse::Array(items) => items.iter().map(|i| i.label.as_str()).collect(),
            CompletionResponse::List(_) => panic!("expected array"),
        };

        let expected = expect![[r#"
            [
              "foo"
            ]"#]];
        expected.assert_eq(&serde_json::to_string_pretty(&labels).unwrap());
    }

    #[test]
    fn outside_taglink_returns_null() {
        let mut store = Store::default();
        let uri: Uri = "file:///test.txt".parse().unwrap();
        store.open(uri.clone(), "*foo* heading\nplain text\n".into());

        let req = Request {
            id: 1.into(),
            method: "textDocument/completion".into(),
            params: json!({
                "textDocument": { "uri": uri.as_str() },
                "position": { "line": 1, "character": 3 }
            }),
        };

        let tag_index = TagIndex::default();
        let resp = handlers::handle_completion(&req, &store, &tag_index);
        assert_eq!(resp.result, Some(serde_json::Value::Null));
    }
}

mod definition {
    use super::*;

    #[test]
    fn cursor_not_on_tag_returns_null() {
        let mut store = Store::default();
        let uri: Uri = "file:///test.txt".parse().unwrap();
        store.open(uri.clone(), "plain text\n".into());

        let req = Request {
            id: 1.into(),
            method: "textDocument/definition".into(),
            params: json!({
                "textDocument": { "uri": uri.as_str() },
                "position": { "line": 0, "character": 3 }
            }),
        };

        let mut tag_index = TagIndex::default();
        let resp = handlers::handle_goto_definition(&req, &store, &mut tag_index);
        assert_eq!(resp.result, Some(serde_json::Value::Null));
    }

    #[test]
    fn resolves_same_file_ref() {
        let mut store = Store::default();
        let uri: Uri = "file:///test.txt".parse().unwrap();
        store.open(uri.clone(), "*foo* heading\nsee |foo| here\n".into());

        let req = Request {
            id: 1.into(),
            method: "textDocument/definition".into(),
            params: json!({
                "textDocument": { "uri": uri.as_str() },
                "position": { "line": 1, "character": 5 }
            }),
        };

        let mut tag_index = TagIndex::default();
        let resp = handlers::handle_goto_definition(&req, &store, &mut tag_index);
        let result: GotoDefinitionResponse = serde_json::from_value(resp.result.unwrap()).unwrap();

        let expected = expect![[r#"
            {
              "uri": "file:///test.txt",
              "range": {
                "start": {
                  "line": 0,
                  "character": 0
                },
                "end": {
                  "line": 0,
                  "character": 5
                }
              }
            }"#]];
        expected.assert_eq(&serde_json::to_string_pretty(&result).unwrap());
    }

    #[test]
    fn resolves_def_under_cursor() {
        let mut store = Store::default();
        let uri: Uri = "file:///test.txt".parse().unwrap();
        store.open(uri.clone(), "*foo* heading\n".into());

        let req = Request {
            id: 1.into(),
            method: "textDocument/definition".into(),
            params: json!({
                "textDocument": { "uri": uri.as_str() },
                "position": { "line": 0, "character": 2 }
            }),
        };

        let mut tag_index = TagIndex::default();
        let resp = handlers::handle_goto_definition(&req, &store, &mut tag_index);
        let result: GotoDefinitionResponse = serde_json::from_value(resp.result.unwrap()).unwrap();

        let expected = expect![[r#"
            {
              "uri": "file:///test.txt",
              "range": {
                "start": {
                  "line": 0,
                  "character": 0
                },
                "end": {
                  "line": 0,
                  "character": 5
                }
              }
            }"#]];
        expected.assert_eq(&serde_json::to_string_pretty(&result).unwrap());
    }

    #[test]
    fn cross_file_via_tag_index() {
        let mut store = Store::default();
        let uri1: Uri = "file:///ref.txt".parse().unwrap();
        let uri2: Uri = "file:///def.txt".parse().unwrap();
        store.open(uri1.clone(), "|foo| ref\n".into());

        let doc2 = Document::parse("*foo* def\n");
        let mut tag_index = TagIndex::default();
        tag_index.update_file(&uri2, &doc2);

        let req = Request {
            id: 1.into(),
            method: "textDocument/definition".into(),
            params: json!({
                "textDocument": { "uri": uri1.as_str() },
                "position": { "line": 0, "character": 2 }
            }),
        };

        let resp = handlers::handle_goto_definition(&req, &store, &mut tag_index);
        let result: GotoDefinitionResponse = serde_json::from_value(resp.result.unwrap()).unwrap();

        let expected = expect![[r#"
            {
              "uri": "file:///def.txt",
              "range": {
                "start": {
                  "line": 0,
                  "character": 0
                },
                "end": {
                  "line": 0,
                  "character": 5
                }
              }
            }"#]];
        expected.assert_eq(&serde_json::to_string_pretty(&result).unwrap());
    }
}

mod highlight {
    use super::*;

    #[test]
    fn cursor_not_on_tag_returns_null() {
        let mut store = Store::default();
        let uri: Uri = "file:///test.txt".parse().unwrap();
        store.open(uri.clone(), "plain text\n".into());

        let req = Request {
            id: 1.into(),
            method: "textDocument/documentHighlight".into(),
            params: json!({
                "textDocument": { "uri": uri.as_str() },
                "position": { "line": 0, "character": 3 }
            }),
        };

        let resp = handlers::handle_document_highlight(&req, &store);
        assert_eq!(resp.result, Some(serde_json::Value::Null));
    }

    #[test]
    fn def_and_refs_highlighted() {
        let mut store = Store::default();
        let uri: Uri = "file:///test.txt".parse().unwrap();
        store.open(uri.clone(), "*foo* heading\nsee |foo| here\n".into());

        let req = Request {
            id: 1.into(),
            method: "textDocument/documentHighlight".into(),
            params: json!({
                "textDocument": { "uri": uri.as_str() },
                "position": { "line": 0, "character": 2 }
            }),
        };

        let resp = handlers::handle_document_highlight(&req, &store);
        let result: Vec<DocumentHighlight> = serde_json::from_value(resp.result.unwrap()).unwrap();

        assert_eq!(result.len(), 2);
        assert_eq!(result[0].kind, Some(DocumentHighlightKind::WRITE));
        assert_eq!(result[1].kind, Some(DocumentHighlightKind::READ));
    }
}

mod folding {
    use super::*;

    #[test]
    fn sections_between_separators() {
        let mut store = Store::default();
        let uri: Uri = "file:///test.txt".parse().unwrap();
        store.open(
            uri.clone(),
            "==========\ncontent\n==========\nmore content\n".into(),
        );

        let req = Request {
            id: 1.into(),
            method: "textDocument/foldingRange".into(),
            params: json!({ "textDocument": { "uri": uri.as_str() } }),
        };

        let resp = handlers::handle_folding_range(&req, &store);
        let result: Vec<FoldingRange> = serde_json::from_value(resp.result.unwrap()).unwrap();
        let ranges: Vec<(u32, u32)> = result.iter().map(|r| (r.start_line, r.end_line)).collect();

        let expected = expect![[r"
            [
              [
                0,
                1
              ],
              [
                2,
                3
              ]
            ]"]];
        expected.assert_eq(&serde_json::to_string_pretty(&ranges).unwrap());
    }

    #[test]
    fn code_block() {
        let mut store = Store::default();
        let uri: Uri = "file:///test.txt".parse().unwrap();
        store.open(
            uri.clone(),
            "example >\n    code here\n    more code\n".into(),
        );

        let req = Request {
            id: 1.into(),
            method: "textDocument/foldingRange".into(),
            params: json!({ "textDocument": { "uri": uri.as_str() } }),
        };

        let resp = handlers::handle_folding_range(&req, &store);
        let result: Vec<FoldingRange> = serde_json::from_value(resp.result.unwrap()).unwrap();
        let ranges: Vec<(u32, u32)> = result.iter().map(|r| (r.start_line, r.end_line)).collect();

        let expected = expect![[r"
            [
              [
                1,
                2
              ]
            ]"]];
        expected.assert_eq(&serde_json::to_string_pretty(&ranges).unwrap());
    }
}

mod document_link {
    use super::*;

    #[test]
    fn resolved_link_included() {
        let mut store = Store::default();
        let uri: Uri = "file:///test.txt".parse().unwrap();
        store.open(uri.clone(), "*foo* def\nsee |foo| here\n".into());

        let req = Request {
            id: 1.into(),
            method: "textDocument/documentLink".into(),
            params: json!({ "textDocument": { "uri": uri.as_str() } }),
        };

        let mut tag_index = TagIndex::default();
        let resp = handlers::handle_document_link(&req, &store, &mut tag_index);
        let result: Vec<lsp_types::DocumentLink> =
            serde_json::from_value(resp.result.unwrap()).unwrap();

        assert_eq!(result.len(), 1);
        assert_eq!(result[0].tooltip.as_deref(), Some("foo"));
    }

    #[test]
    fn unresolved_link_omitted() {
        let mut store = Store::default();
        let uri: Uri = "file:///test.txt".parse().unwrap();
        store.open(uri.clone(), "see |missing| here\n".into());

        let req = Request {
            id: 1.into(),
            method: "textDocument/documentLink".into(),
            params: json!({ "textDocument": { "uri": uri.as_str() } }),
        };

        let mut tag_index = TagIndex::default();
        let resp = handlers::handle_document_link(&req, &store, &mut tag_index);
        let result: Vec<serde_json::Value> = serde_json::from_value(resp.result.unwrap()).unwrap();

        assert!(result.is_empty());
    }
}

mod hover {
    use super::*;

    #[test]
    fn cursor_not_on_tag_returns_null() {
        let mut store = Store::default();
        let uri: Uri = "file:///test.txt".parse().unwrap();
        store.open(uri.clone(), "plain text\n".into());

        let req = Request {
            id: 1.into(),
            method: "textDocument/hover".into(),
            params: json!({
                "textDocument": { "uri": uri.as_str() },
                "position": { "line": 0, "character": 3 }
            }),
        };

        let mut tag_index = TagIndex::default();
        let resp = handlers::handle_hover(&req, &store, &mut tag_index);
        assert_eq!(resp.result, Some(serde_json::Value::Null));
    }

    #[test]
    fn shows_context_for_tag() {
        let mut store = Store::default();
        let uri: Uri = "file:///test.txt".parse().unwrap();
        store.open(
            uri.clone(),
            "*foo* heading\nsome context\n\n|foo| ref\n".into(),
        );

        let req = Request {
            id: 1.into(),
            method: "textDocument/hover".into(),
            params: json!({
                "textDocument": { "uri": uri.as_str() },
                "position": { "line": 3, "character": 2 }
            }),
        };

        let mut tag_index = TagIndex::default();
        let resp = handlers::handle_hover(&req, &store, &mut tag_index);
        let value = resp.result.unwrap();
        let markdown = value["contents"]["value"].as_str().unwrap();

        let expected = expect![[r"
            ```vim
            *foo* heading
            some context
            ```"]];
        expected.assert_eq(markdown);
    }
}

mod references {
    use super::*;

    #[test]
    fn cursor_not_on_tag_returns_null() {
        let mut store = Store::default();
        let uri: Uri = "file:///test.txt".parse().unwrap();
        store.open(uri.clone(), "plain text\n".into());

        let req = Request {
            id: 1.into(),
            method: "textDocument/references".into(),
            params: json!({
                "textDocument": { "uri": uri.as_str() },
                "position": { "line": 0, "character": 3 },
                "context": { "includeDeclaration": false }
            }),
        };

        let tag_index = TagIndex::default();
        let resp = handlers::handle_references(&req, &store, &tag_index);
        assert_eq!(resp.result, Some(serde_json::Value::Null));
    }

    #[test]
    fn finds_refs_in_workspace() {
        let mut store = Store::default();
        let uri1: Uri = "file:///def.txt".parse().unwrap();
        let uri2: Uri = "file:///ref.txt".parse().unwrap();
        store.open(uri1.clone(), "*foo* def\n".into());

        let doc2 = Document::parse("|foo| ref\n");
        let mut tag_index = TagIndex::default();
        tag_index.update_file(&uri2, &doc2);

        let req = Request {
            id: 1.into(),
            method: "textDocument/references".into(),
            params: json!({
                "textDocument": { "uri": uri1.as_str() },
                "position": { "line": 0, "character": 2 },
                "context": { "includeDeclaration": false }
            }),
        };

        let resp = handlers::handle_references(&req, &store, &tag_index);
        let result: Vec<Location> = serde_json::from_value(resp.result.unwrap()).unwrap();

        assert_eq!(result.len(), 1);
        assert_eq!(result[0].uri, uri2);
    }

    #[test]
    fn include_declaration_adds_def() {
        let mut store = Store::default();
        let uri1: Uri = "file:///def.txt".parse().unwrap();
        let uri2: Uri = "file:///ref.txt".parse().unwrap();
        store.open(uri1.clone(), "*foo* def\n".into());

        let doc1 = Document::parse("*foo* def\n");
        let doc2 = Document::parse("|foo| ref\n");
        let mut tag_index = TagIndex::default();
        tag_index.update_file(&uri1, &doc1);
        tag_index.update_file(&uri2, &doc2);

        let req = Request {
            id: 1.into(),
            method: "textDocument/references".into(),
            params: json!({
                "textDocument": { "uri": uri1.as_str() },
                "position": { "line": 0, "character": 2 },
                "context": { "includeDeclaration": true }
            }),
        };

        let resp = handlers::handle_references(&req, &store, &tag_index);
        let result: Vec<Location> = serde_json::from_value(resp.result.unwrap()).unwrap();

        assert_eq!(result.len(), 2);
        assert!(result.iter().any(|l| l.uri == uri1));
        assert!(result.iter().any(|l| l.uri == uri2));
    }
}

mod rename {
    use super::*;

    #[test]
    fn cursor_not_on_tag_returns_null() {
        let mut store = Store::default();
        let uri: Uri = "file:///test.txt".parse().unwrap();
        store.open(uri.clone(), "plain text\n".into());

        let req = Request {
            id: 1.into(),
            method: "textDocument/rename".into(),
            params: json!({
                "textDocument": { "uri": uri.as_str() },
                "position": { "line": 0, "character": 3 },
                "newName": "foo"
            }),
        };

        let tag_index = TagIndex::default();
        let resp = handlers::handle_rename(&req, &store, &tag_index);
        assert_eq!(resp.result, Some(serde_json::Value::Null));
    }

    #[test]
    fn renames_def_and_refs() {
        let mut store = Store::default();
        let uri: Uri = "file:///test.txt".parse().unwrap();
        store.open(uri.clone(), "*foo* heading\nsee |foo| here\n".into());

        let req = Request {
            id: 1.into(),
            method: "textDocument/rename".into(),
            params: json!({
                "textDocument": { "uri": uri.as_str() },
                "position": { "line": 0, "character": 2 },
                "newName": "bar"
            }),
        };

        let tag_index = TagIndex::default();
        let resp = handlers::handle_rename(&req, &store, &tag_index);
        let result: WorkspaceEdit = serde_json::from_value(resp.result.unwrap()).unwrap();
        #[allow(clippy::mutable_key_type)]
        let changes = result.changes.unwrap();
        let edits: &Vec<TextEdit> = changes.get(&uri).unwrap();

        assert_eq!(edits.len(), 2);
        assert_eq!(edits[0].new_text, "*bar*");
        assert_eq!(edits[1].new_text, "|bar|");
    }

    #[test]
    fn rejects_whitespace_name() {
        let mut store = Store::default();
        let uri: Uri = "file:///test.txt".parse().unwrap();
        store.open(uri.clone(), "*foo* heading\n".into());

        let req = Request {
            id: 1.into(),
            method: "textDocument/rename".into(),
            params: json!({
                "textDocument": { "uri": uri.as_str() },
                "position": { "line": 0, "character": 2 },
                "newName": "foo bar"
            }),
        };

        let tag_index = TagIndex::default();
        let resp = handlers::handle_rename(&req, &store, &tag_index);
        assert!(resp.result.is_none());
        assert!(resp.error.is_some());
    }

    #[test]
    fn rejects_conflicting_name() {
        let mut store = Store::default();
        let uri1: Uri = "file:///test.txt".parse().unwrap();
        let uri2: Uri = "file:///other.txt".parse().unwrap();
        store.open(uri1.clone(), "*foo* heading\n".into());

        let doc2 = Document::parse("*baz* other\n");
        let mut tag_index = TagIndex::default();
        tag_index.update_file(&uri2, &doc2);

        let req = Request {
            id: 1.into(),
            method: "textDocument/rename".into(),
            params: json!({
                "textDocument": { "uri": uri1.as_str() },
                "position": { "line": 0, "character": 2 },
                "newName": "baz"
            }),
        };

        let resp = handlers::handle_rename(&req, &store, &tag_index);
        assert!(resp.result.is_none());
        assert!(resp.error.is_some());
    }
}

mod prepare_rename {
    use super::*;

    #[test]
    fn cursor_not_on_tag_returns_null() {
        let mut store = Store::default();
        let uri: Uri = "file:///test.txt".parse().unwrap();
        store.open(uri.clone(), "plain text\n".into());

        let req = Request {
            id: 1.into(),
            method: "textDocument/prepareRename".into(),
            params: json!({
                "textDocument": { "uri": uri.as_str() },
                "position": { "line": 0, "character": 3 }
            }),
        };

        let resp = handlers::handle_prepare_rename(&req, &store);
        assert_eq!(resp.result, Some(serde_json::Value::Null));
    }

    #[test]
    fn returns_tag_range() {
        let mut store = Store::default();
        let uri: Uri = "file:///test.txt".parse().unwrap();
        store.open(uri.clone(), "*foo* heading\n".into());

        let req = Request {
            id: 1.into(),
            method: "textDocument/prepareRename".into(),
            params: json!({
                "textDocument": { "uri": uri.as_str() },
                "position": { "line": 0, "character": 2 }
            }),
        };

        let resp = handlers::handle_prepare_rename(&req, &store);
        let result: PrepareRenameResponse = serde_json::from_value(resp.result.unwrap()).unwrap();

        let expected = expect![[r#"
            {
              "range": {
                "start": {
                  "line": 0,
                  "character": 1
                },
                "end": {
                  "line": 0,
                  "character": 4
                }
              },
              "placeholder": "foo"
            }"#]];
        expected.assert_eq(&serde_json::to_string_pretty(&result).unwrap());
    }
}

mod formatting {
    use super::*;

    #[test]
    fn already_formatted_returns_null() {
        let mut store = Store::default();
        let uri: Uri = "file:///test.txt".parse().unwrap();
        store.open(uri.clone(), format!("{}\n", "=".repeat(78)));

        let req = Request {
            id: 1.into(),
            method: "textDocument/formatting".into(),
            params: json!({
                "textDocument": { "uri": uri.as_str() },
                "options": { "tabSize": 4, "insertSpaces": true }
            }),
        };

        let config = Config {
            line_width: 78,
            formatting: true,
            reflow: ReflowMode::Always,
            normalize_spacing: false,
            diagnostics: false,
            hover: false,
            runtime_tags: false,
            tag_paths: vec![],
        };

        let resp = handlers::handle_formatting(&req, &store, &config);
        assert_eq!(resp.result, Some(serde_json::Value::Null));
    }

    #[test]
    fn normalizes_separator() {
        let mut store = Store::default();
        let uri: Uri = "file:///test.txt".parse().unwrap();
        store.open(uri.clone(), "==========\n".into());

        let req = Request {
            id: 1.into(),
            method: "textDocument/formatting".into(),
            params: json!({
                "textDocument": { "uri": uri.as_str() },
                "options": { "tabSize": 4, "insertSpaces": true }
            }),
        };

        let config = Config {
            line_width: 20,
            formatting: true,
            reflow: ReflowMode::Always,
            normalize_spacing: false,
            diagnostics: false,
            hover: false,
            runtime_tags: false,
            tag_paths: vec![],
        };

        let resp = handlers::handle_formatting(&req, &store, &config);
        let result: Vec<TextEdit> = serde_json::from_value(resp.result.unwrap()).unwrap();

        assert_eq!(result.len(), 1);
        assert_eq!(result[0].new_text, "====================\n");
    }

    #[test]
    fn range_formatting_only_formats_selected_lines() {
        let mut store = Store::default();
        let uri: Uri = "file:///test.txt".parse().unwrap();
        store.open(
            uri.clone(),
            "untouched prose line\n==========\nafter\n".into(),
        );

        let req = Request {
            id: 1.into(),
            method: "textDocument/rangeFormatting".into(),
            params: json!({
                "textDocument": { "uri": uri.as_str() },
                "range": {
                    "start": { "line": 1, "character": 0 },
                    "end":   { "line": 1, "character": 10 }
                },
                "options": { "tabSize": 4, "insertSpaces": true }
            }),
        };

        let config = Config {
            line_width: 78,
            formatting: true,
            reflow: ReflowMode::Always,
            normalize_spacing: false,
            diagnostics: false,
            hover: false,
            runtime_tags: false,
            tag_paths: vec![],
        };

        let resp = handlers::handle_range_formatting(&req, &store, &config);
        let result: Vec<TextEdit> = serde_json::from_value(resp.result.unwrap()).unwrap();

        assert_eq!(result.len(), 1);
        assert_eq!(result[0].range.start.line, 1);
        assert_eq!(result[0].range.end.line, 1);
        assert_eq!(result[0].new_text, "=".repeat(78));
    }

    #[test]
    fn range_formatting_returns_null_when_already_formatted() {
        let mut store = Store::default();
        let uri: Uri = "file:///test.txt".parse().unwrap();
        store.open(uri.clone(), format!("{}\n", "=".repeat(78)));

        let req = Request {
            id: 1.into(),
            method: "textDocument/rangeFormatting".into(),
            params: json!({
                "textDocument": { "uri": uri.as_str() },
                "range": {
                    "start": { "line": 0, "character": 0 },
                    "end":   { "line": 0, "character": 78 }
                },
                "options": { "tabSize": 4, "insertSpaces": true }
            }),
        };

        let config = Config {
            line_width: 78,
            formatting: true,
            reflow: ReflowMode::Always,
            normalize_spacing: false,
            diagnostics: false,
            hover: false,
            runtime_tags: false,
            tag_paths: vec![],
        };

        let resp = handlers::handle_range_formatting(&req, &store, &config);
        assert_eq!(resp.result, Some(serde_json::Value::Null));
    }
}

mod code_action {
    use super::*;

    const FIXTURE: &str = "heading text *heading*\n\n==========\n\nprose line one\nprose line two\n\n>lua\n    some_code()\n<\n";

    fn make_config() -> Config {
        Config {
            line_width: 30,
            formatting: true,
            reflow: ReflowMode::Always,
            normalize_spacing: false,
            diagnostics: false,
            hover: false,
            runtime_tags: false,
            tag_paths: vec![],
        }
    }

    fn make_req(cursor_line: u32, uri: &Uri) -> Request {
        make_req_at(cursor_line, 0, uri)
    }

    fn make_req_at(cursor_line: u32, cursor_char: u32, uri: &Uri) -> Request {
        Request {
            id: 1.into(),
            method: "textDocument/codeAction".into(),
            params: json!({
                "textDocument": { "uri": uri.as_str() },
                "range": {
                    "start": { "line": cursor_line, "character": cursor_char },
                    "end":   { "line": cursor_line, "character": cursor_char }
                },
                "context": { "diagnostics": [] }
            }),
        }
    }

    fn action_edits(result: &[CodeActionOrCommand], idx: usize) -> &[TextEdit] {
        let CodeActionOrCommand::CodeAction(action) = &result[idx] else {
            panic!("expected CodeAction at index {idx}")
        };
        action.edit.as_ref().unwrap().changes.as_ref().unwrap().values().next().unwrap()
    }

    #[test]
    fn prose_block_returns_action_covering_paragraph() {
        let mut store = Store::default();
        let uri: Uri = "file:///test.txt".parse().unwrap();
        store.open(uri.clone(), FIXTURE.into());

        let resp =
            handlers::handle_code_action(&make_req(4, &uri), &store, &make_config(), &TagIndex::default());
        let result: Vec<CodeActionOrCommand> =
            serde_json::from_value(resp.result.unwrap()).unwrap();

        assert_eq!(result.len(), 1);
        let CodeActionOrCommand::CodeAction(action) = &result[0] else {
            panic!("expected CodeAction")
        };
        assert_eq!(action.title, "Format this block");
        let edits = action_edits(&result, 0);
        assert_eq!(edits[0].range.start.line, 4);
        assert_eq!(edits[0].range.end.line, 5);
    }

    #[test]
    fn separator_returns_format_and_convert_actions() {
        let mut store = Store::default();
        let uri: Uri = "file:///test.txt".parse().unwrap();
        store.open(uri.clone(), FIXTURE.into());

        let resp =
            handlers::handle_code_action(&make_req(2, &uri), &store, &make_config(), &TagIndex::default());
        let result: Vec<CodeActionOrCommand> =
            serde_json::from_value(resp.result.unwrap()).unwrap();

        assert_eq!(result.len(), 2);
        let edits = action_edits(&result, 0);
        assert_eq!(edits[0].range.start.line, 2);
        assert_eq!(edits[0].range.end.line, 2);
    }

    #[test]
    fn convert_separator_minor_to_major() {
        let mut store = Store::default();
        let uri: Uri = "file:///test.txt".parse().unwrap();
        store.open(uri.clone(), "----------\n".into());

        let resp =
            handlers::handle_code_action(&make_req(0, &uri), &store, &make_config(), &TagIndex::default());
        let result: Vec<CodeActionOrCommand> =
            serde_json::from_value(resp.result.unwrap()).unwrap();

        let titles: Vec<&str> = result
            .iter()
            .map(|a| {
                let CodeActionOrCommand::CodeAction(ca) = a else { panic!("expected CodeAction") };
                ca.title.as_str()
            })
            .collect();
        assert!(titles.contains(&"Convert to major separator"));
    }

    #[test]
    fn heading_returns_single_line_action() {
        let mut store = Store::default();
        let uri: Uri = "file:///test.txt".parse().unwrap();
        store.open(uri.clone(), FIXTURE.into());

        let resp =
            handlers::handle_code_action(&make_req(0, &uri), &store, &make_config(), &TagIndex::default());
        let result: Vec<CodeActionOrCommand> =
            serde_json::from_value(resp.result.unwrap()).unwrap();

        assert_eq!(result.len(), 1);
        let edits = action_edits(&result, 0);
        assert_eq!(edits[0].range.start.line, 0);
        assert_eq!(edits[0].range.end.line, 0);
    }

    #[test]
    fn blank_line_returns_empty_actions() {
        let mut store = Store::default();
        let uri: Uri = "file:///test.txt".parse().unwrap();
        store.open(uri.clone(), FIXTURE.into());

        let resp =
            handlers::handle_code_action(&make_req(1, &uri), &store, &make_config(), &TagIndex::default());
        let result: Vec<CodeActionOrCommand> =
            serde_json::from_value(resp.result.unwrap()).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn fence_language_offered_on_code_body() {
        let mut store = Store::default();
        let uri: Uri = "file:///test.txt".parse().unwrap();
        store.open(uri.clone(), FIXTURE.into());

        let resp =
            handlers::handle_code_action(&make_req(8, &uri), &store, &make_config(), &TagIndex::default());
        let result: Vec<CodeActionOrCommand> =
            serde_json::from_value(resp.result.unwrap()).unwrap();
        assert_eq!(result.len(), 7);
    }

    #[test]
    fn fence_language_current_not_offered() {
        let mut store = Store::default();
        let uri: Uri = "file:///test.txt".parse().unwrap();
        store.open(uri.clone(), FIXTURE.into());

        let resp =
            handlers::handle_code_action(&make_req(8, &uri), &store, &make_config(), &TagIndex::default());
        let result: Vec<CodeActionOrCommand> =
            serde_json::from_value(resp.result.unwrap()).unwrap();

        let has_lua = result.iter().any(|a| {
            let CodeActionOrCommand::CodeAction(ca) = a else { return false };
            ca.title == "Set code block language: lua"
        });
        assert!(!has_lua);
    }

    #[test]
    fn already_formatted_returns_empty_actions() {
        let mut store = Store::default();
        let uri: Uri = "file:///test.txt".parse().unwrap();
        store.open(uri.clone(), "short prose\n".into());

        let resp =
            handlers::handle_code_action(&make_req(0, &uri), &store, &make_config(), &TagIndex::default());
        let result: Vec<CodeActionOrCommand> =
            serde_json::from_value(resp.result.unwrap()).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn remove_taglink_on_cursor_inside() {
        let mut store = Store::default();
        let uri: Uri = "file:///test.txt".parse().unwrap();
        store.open(uri.clone(), "see |foo| here\n".into());

        let resp = handlers::handle_code_action(
            &make_req_at(0, 5, &uri),
            &store,
            &make_config(),
            &TagIndex::default(),
        );
        let result: Vec<CodeActionOrCommand> =
            serde_json::from_value(resp.result.unwrap()).unwrap();

        assert_eq!(result.len(), 1);
        let CodeActionOrCommand::CodeAction(action) = &result[0] else {
            panic!("expected CodeAction")
        };
        assert_eq!(action.title, "Remove taglink delimiters");
    }

    #[test]
    fn add_taglink_for_known_tag() {
        let mut store = Store::default();
        let uri: Uri = "file:///test.txt".parse().unwrap();
        store.open(uri.clone(), "foo is cool\n".into());

        let mut tag_index = TagIndex::default();
        let tag_uri: Uri = "file:///other.txt".parse().unwrap();
        tag_index.update_file(&tag_uri, &Document::parse("*foo* heading\n"));

        let resp = handlers::handle_code_action(
            &make_req_at(0, 0, &uri),
            &store,
            &make_config(),
            &tag_index,
        );
        let result: Vec<CodeActionOrCommand> =
            serde_json::from_value(resp.result.unwrap()).unwrap();

        assert_eq!(result.len(), 1);
        let CodeActionOrCommand::CodeAction(action) = &result[0] else {
            panic!("expected CodeAction")
        };
        assert_eq!(action.title, "Add taglink");
    }

    #[test]
    fn no_add_taglink_for_unknown_word() {
        let mut store = Store::default();
        let uri: Uri = "file:///test.txt".parse().unwrap();
        store.open(uri.clone(), "unknown word\n".into());

        let resp = handlers::handle_code_action(
            &make_req_at(0, 0, &uri),
            &store,
            &make_config(),
            &TagIndex::default(),
        );
        let result: Vec<CodeActionOrCommand> =
            serde_json::from_value(resp.result.unwrap()).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn toc_generated_before_first_separator() {
        let mut store = Store::default();
        let uri: Uri = "file:///test.txt".parse().unwrap();
        store.open(uri.clone(), "A *a*\n\nB *b*\n\n==============================\n".into());

        let resp =
            handlers::handle_code_action(&make_req(0, &uri), &store, &make_config(), &TagIndex::default());
        let result: Vec<CodeActionOrCommand> =
            serde_json::from_value(resp.result.unwrap()).unwrap();

        let toc_action = result.iter().find(|a| {
            let CodeActionOrCommand::CodeAction(ca) = a else { return false };
            ca.title == "Generate table of contents"
        });
        assert!(toc_action.is_some());
        let CodeActionOrCommand::CodeAction(action) = toc_action.unwrap() else {
            unreachable!()
        };
        let edits =
            action.edit.as_ref().unwrap().changes.as_ref().unwrap().values().next().unwrap();
        assert_eq!(edits[0].range.start.line, 4);
    }

    #[test]
    fn toc_not_offered_single_heading() {
        let mut store = Store::default();
        let uri: Uri = "file:///test.txt".parse().unwrap();
        store.open(uri.clone(), "Only *one-heading*\n".into());

        let resp =
            handlers::handle_code_action(&make_req(0, &uri), &store, &make_config(), &TagIndex::default());
        let result: Vec<CodeActionOrCommand> =
            serde_json::from_value(resp.result.unwrap()).unwrap();

        let has_toc = result.iter().any(|a| {
            let CodeActionOrCommand::CodeAction(ca) = a else { return false };
            ca.title == "Generate table of contents"
        });
        assert!(!has_toc);
    }

    #[test]
    fn toc_not_offered_if_contents_tag_exists() {
        let mut store = Store::default();
        let uri: Uri = "file:///test.txt".parse().unwrap();
        store.open(
            uri.clone(),
            "A *a*\n\nB *b*\n\nContents *test-contents*\n".into(),
        );

        let resp =
            handlers::handle_code_action(&make_req(0, &uri), &store, &make_config(), &TagIndex::default());
        let result: Vec<CodeActionOrCommand> =
            serde_json::from_value(resp.result.unwrap()).unwrap();

        let has_toc = result.iter().any(|a| {
            let CodeActionOrCommand::CodeAction(ca) = a else { return false };
            ca.title == "Generate table of contents"
        });
        assert!(!has_toc);
    }
}

mod diagnostics_tests {
    use super::*;

    #[test]
    fn duplicate_tag_warning() {
        let uri: Uri = "file:///test.txt".parse().unwrap();
        let doc = Document::parse("*foo* first\n*foo* second\n");
        let tag_index = TagIndex::default();
        let diags = diagnostics::compute(&doc, &tag_index, &uri);

        assert_eq!(diags.len(), 2);
        assert!(
            diags
                .iter()
                .all(|d| { d.code == Some(NumberOrString::String("duplicate-tag".into())) })
        );
    }

    #[test]
    fn unresolved_ref_warning() {
        let uri: Uri = "file:///test.txt".parse().unwrap();
        let doc = Document::parse("|missing| ref\n");
        let tag_index = TagIndex::default();
        let diags = diagnostics::compute(&doc, &tag_index, &uri);

        assert_eq!(diags.len(), 1);
        assert_eq!(
            diags[0].code,
            Some(NumberOrString::String("unresolved-tag".into()))
        );
    }

    #[test]
    fn resolved_ref_clean() {
        let uri: Uri = "file:///test.txt".parse().unwrap();
        let doc = Document::parse("*foo* def\nsee |foo| here\n");
        let tag_index = TagIndex::default();
        let diags = diagnostics::compute(&doc, &tag_index, &uri);

        assert!(diags.is_empty());
    }

    #[test]
    fn cross_file_duplicate_tag_warning() {
        let uri1: Uri = "file:///a.txt".parse().unwrap();
        let uri2: Uri = "file:///b.txt".parse().unwrap();
        let doc1 = Document::parse("*foo* heading\n");
        let doc2 = Document::parse("*foo* other\n");
        let mut tag_index = TagIndex::default();
        tag_index.update_file(&uri2, &doc2);
        let diags = diagnostics::compute(&doc1, &tag_index, &uri1);

        assert_eq!(diags.len(), 1);
        assert_eq!(
            diags[0].code,
            Some(NumberOrString::String("duplicate-tag".into()))
        );
    }
}
