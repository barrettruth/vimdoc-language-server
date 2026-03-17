use expect_test::expect;
use lsp_server::Request;
use lsp_types::{DocumentSymbolResponse, Uri};
use serde_json::json;
use vimdoc_language_server::{handlers, store::Store};

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
