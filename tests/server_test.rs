use lsp_server::{Connection, Message, Notification, Request};
use serde_json::json;
use std::thread::{self, JoinHandle};
use vimdoc_language_server::{
    server::{Config, main_loop},
    tags::TagIndex,
};

fn default_config() -> Config {
    Config {
        line_width: 78,
        formatting: true,
        diagnostics: false,
        hover: true,
        runtime_tags: false,
        tag_paths: vec![],
    }
}

#[allow(clippy::missing_panics_doc)]
fn spawn_server(config: Config) -> (Connection, JoinHandle<()>) {
    let (server_conn, client_conn) = Connection::memory();
    let handle = thread::spawn(move || {
        server_conn
            .initialize(json!({}))
            .expect("initialize failed");
        let mut tag_index = TagIndex::default();
        main_loop(&server_conn, &config, &mut tag_index).expect("main_loop failed");
    });
    client_conn
        .sender
        .send(Message::Request(Request {
            id: 1.into(),
            method: "initialize".into(),
            params: json!({"capabilities": {}, "rootUri": null}),
        }))
        .expect("send initialize");
    match client_conn.receiver.recv().expect("recv initializeResult") {
        Message::Response(_) => {}
        other => panic!("expected initializeResult response, got {other:?}"),
    }
    client_conn
        .sender
        .send(Message::Notification(Notification {
            method: "initialized".into(),
            params: json!({}),
        }))
        .expect("send initialized");
    (client_conn, handle)
}

#[allow(clippy::missing_panics_doc, clippy::needless_pass_by_value)]
fn shutdown(conn: Connection, handle: JoinHandle<()>) {
    conn.sender
        .send(Message::Request(Request {
            id: 0.into(),
            method: "shutdown".into(),
            params: json!(null),
        }))
        .expect("send shutdown");
    match conn.receiver.recv().expect("recv shutdown response") {
        Message::Response(_) => {}
        other => panic!("expected shutdown response, got {other:?}"),
    }
    conn.sender
        .send(Message::Notification(Notification {
            method: "exit".into(),
            params: json!({}),
        }))
        .expect("send exit");
    handle.join().expect("server thread panicked");
}

#[test]
fn initialization_succeeds() {
    let (conn, handle) = spawn_server(default_config());
    shutdown(conn, handle);
}

#[test]
fn did_open_and_document_symbol() {
    let (conn, handle) = spawn_server(default_config());
    let uri = "file:///test.txt";

    conn.sender
        .send(Message::Notification(Notification {
            method: "textDocument/didOpen".into(),
            params: json!({
                "textDocument": {
                    "uri": uri,
                    "languageId": "vimdoc",
                    "version": 1,
                    "text": "*foo* heading\n*bar* other\n"
                }
            }),
        }))
        .expect("send didOpen");

    conn.sender
        .send(Message::Request(Request {
            id: 2.into(),
            method: "textDocument/documentSymbol".into(),
            params: json!({"textDocument": {"uri": uri}}),
        }))
        .expect("send documentSymbol");

    let resp = match conn.receiver.recv().expect("recv documentSymbol response") {
        Message::Response(r) => r,
        other => panic!("expected response, got {other:?}"),
    };
    let symbols: Vec<serde_json::Value> = serde_json::from_value(resp.result.unwrap()).unwrap();
    let names: Vec<&str> = symbols
        .iter()
        .map(|s| s["name"].as_str().unwrap())
        .collect();
    assert_eq!(names, ["foo", "bar"]);

    shutdown(conn, handle);
}

#[test]
fn did_change_updates_content() {
    let (conn, handle) = spawn_server(default_config());
    let uri = "file:///test.txt";

    conn.sender
        .send(Message::Notification(Notification {
            method: "textDocument/didOpen".into(),
            params: json!({
                "textDocument": {
                    "uri": uri,
                    "languageId": "vimdoc",
                    "version": 1,
                    "text": "*foo*\n"
                }
            }),
        }))
        .expect("send didOpen");

    conn.sender
        .send(Message::Notification(Notification {
            method: "textDocument/didChange".into(),
            params: json!({
                "textDocument": {"uri": uri, "version": 2},
                "contentChanges": [{"text": "*bar*\n*baz*\n"}]
            }),
        }))
        .expect("send didChange");

    conn.sender
        .send(Message::Request(Request {
            id: 2.into(),
            method: "textDocument/documentSymbol".into(),
            params: json!({"textDocument": {"uri": uri}}),
        }))
        .expect("send documentSymbol");

    let resp = match conn.receiver.recv().expect("recv documentSymbol response") {
        Message::Response(r) => r,
        other => panic!("expected response, got {other:?}"),
    };
    let symbols: Vec<serde_json::Value> = serde_json::from_value(resp.result.unwrap()).unwrap();
    let names: Vec<&str> = symbols
        .iter()
        .map(|s| s["name"].as_str().unwrap())
        .collect();
    assert_eq!(names, ["bar", "baz"]);

    shutdown(conn, handle);
}

#[test]
fn did_close_removes_from_store() {
    let (conn, handle) = spawn_server(default_config());
    let uri = "file:///test.txt";

    conn.sender
        .send(Message::Notification(Notification {
            method: "textDocument/didOpen".into(),
            params: json!({
                "textDocument": {
                    "uri": uri,
                    "languageId": "vimdoc",
                    "version": 1,
                    "text": "*foo*\n"
                }
            }),
        }))
        .expect("send didOpen");

    conn.sender
        .send(Message::Notification(Notification {
            method: "textDocument/didClose".into(),
            params: json!({"textDocument": {"uri": uri}}),
        }))
        .expect("send didClose");

    conn.sender
        .send(Message::Request(Request {
            id: 2.into(),
            method: "textDocument/documentSymbol".into(),
            params: json!({"textDocument": {"uri": uri}}),
        }))
        .expect("send documentSymbol");

    let resp = match conn.receiver.recv().expect("recv documentSymbol response") {
        Message::Response(r) => r,
        other => panic!("expected response, got {other:?}"),
    };
    assert!(resp.error.is_some());

    shutdown(conn, handle);
}

#[test]
fn did_open_pushes_diagnostics() {
    let config = Config {
        diagnostics: true,
        ..default_config()
    };
    let (conn, handle) = spawn_server(config);
    let uri = "file:///test.txt";

    conn.sender
        .send(Message::Notification(Notification {
            method: "textDocument/didOpen".into(),
            params: json!({
                "textDocument": {
                    "uri": uri,
                    "languageId": "vimdoc",
                    "version": 1,
                    "text": "*dup* a\n*dup* b\n"
                }
            }),
        }))
        .expect("send didOpen");

    let notif = match conn.receiver.recv().expect("recv diagnostics") {
        Message::Notification(n) => n,
        other => panic!("expected notification, got {other:?}"),
    };
    assert_eq!(notif.method, "textDocument/publishDiagnostics");
    let diags = notif.params["diagnostics"].as_array().unwrap();
    assert_eq!(diags.len(), 2);
    assert!(
        diags
            .iter()
            .all(|d| d["code"].as_str() == Some("duplicate-tag"))
    );

    shutdown(conn, handle);
}

#[test]
fn hover_disabled_returns_error() {
    let config = Config {
        hover: false,
        ..default_config()
    };
    let (conn, handle) = spawn_server(config);
    let uri = "file:///test.txt";

    conn.sender
        .send(Message::Notification(Notification {
            method: "textDocument/didOpen".into(),
            params: json!({
                "textDocument": {
                    "uri": uri,
                    "languageId": "vimdoc",
                    "version": 1,
                    "text": "*foo*\n"
                }
            }),
        }))
        .expect("send didOpen");

    conn.sender
        .send(Message::Request(Request {
            id: 2.into(),
            method: "textDocument/hover".into(),
            params: json!({
                "textDocument": {"uri": uri},
                "position": {"line": 0, "character": 2}
            }),
        }))
        .expect("send hover");

    let resp = match conn.receiver.recv().expect("recv hover response") {
        Message::Response(r) => r,
        other => panic!("expected response, got {other:?}"),
    };
    assert!(resp.error.is_some());

    shutdown(conn, handle);
}

#[test]
fn formatting_disabled_returns_error() {
    let config = Config {
        formatting: false,
        ..default_config()
    };
    let (conn, handle) = spawn_server(config);
    let uri = "file:///test.txt";

    conn.sender
        .send(Message::Notification(Notification {
            method: "textDocument/didOpen".into(),
            params: json!({
                "textDocument": {
                    "uri": uri,
                    "languageId": "vimdoc",
                    "version": 1,
                    "text": "*foo*\n"
                }
            }),
        }))
        .expect("send didOpen");

    conn.sender
        .send(Message::Request(Request {
            id: 2.into(),
            method: "textDocument/formatting".into(),
            params: json!({
                "textDocument": {"uri": uri},
                "options": {"tabSize": 4, "insertSpaces": true}
            }),
        }))
        .expect("send formatting");

    let resp = match conn.receiver.recv().expect("recv formatting response") {
        Message::Response(r) => r,
        other => panic!("expected response, got {other:?}"),
    };
    assert!(resp.error.is_some());

    shutdown(conn, handle);
}
