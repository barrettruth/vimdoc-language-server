use expect_test::expect;
use lsp_server::Request;
use lsp_types::{
    CompletionResponse, DocumentHighlight, DocumentHighlightKind, DocumentSymbolResponse,
    FoldingRange, GotoDefinitionResponse, Location, NumberOrString, PrepareRenameResponse,
    TextEdit, Uri, WorkspaceEdit,
};
use serde_json::json;
use vimdoc_language_server::{
    diagnostics, handlers, parser::Document, server::Config, store::Store, tags::TagIndex,
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
              "start": {
                "line": 0,
                "character": 0
              },
              "end": {
                "line": 0,
                "character": 5
              }
            }"#]];
        expected.assert_eq(&serde_json::to_string_pretty(&result).unwrap());
    }
}

mod formatting {
    use super::*;

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
}

mod diagnostics_tests {
    use super::*;

    #[test]
    fn duplicate_tag_warning() {
        let doc = Document::parse("*foo* first\n*foo* second\n");
        let tag_index = TagIndex::default();
        let diags = diagnostics::compute(&doc, &tag_index);

        assert_eq!(diags.len(), 2);
        assert!(
            diags
                .iter()
                .all(|d| { d.code == Some(NumberOrString::String("duplicate-tag".into())) })
        );
    }

    #[test]
    fn unresolved_ref_warning() {
        let doc = Document::parse("|missing| ref\n");
        let tag_index = TagIndex::default();
        let diags = diagnostics::compute(&doc, &tag_index);

        assert_eq!(diags.len(), 1);
        assert_eq!(
            diags[0].code,
            Some(NumberOrString::String("unresolved-tag".into()))
        );
    }

    #[test]
    fn resolved_ref_clean() {
        let doc = Document::parse("*foo* def\nsee |foo| here\n");
        let tag_index = TagIndex::default();
        let diags = diagnostics::compute(&doc, &tag_index);

        assert!(diags.is_empty());
    }
}
