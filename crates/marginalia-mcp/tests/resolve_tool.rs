mod common;

use std::fs;

use marginalia_core::{Comment, Event, Store};
use serde_json::json;
use tempfile::TempDir;

use common::StdioServer;

const KNOWN_ID: &str = "c1";

/// No `Add` event ever used this id, so the server cannot resolve it.
const UNKNOWN_ID: &str = "c7";

#[test]
fn resolve_comment_errors_on_unknown_id() {
    let worktree = TempDir::new().expect("temporary worktree");
    let store = Store::open(worktree.path()).expect("open store");
    store
        .append(&Event::Add(Comment {
            id: KNOWN_ID.to_owned(),
            file: "src/lib.rs".to_owned(),
            start_row: 2,
            end_row: 2,
            quote: "    let kept = 1;\n".to_owned(),
            body: format!("body of {KNOWN_ID}"),
            created_at: 0,
        }))
        .unwrap_or_else(|e| panic!("append {KNOWN_ID}: {e:#}"));
    let log = worktree.path().join(".marginalia").join("comments.jsonl");
    let logged_before = fs::metadata(&log).expect("log of the added comment").len();

    let mut server = StdioServer::spawn(worktree.path());
    server.initialize("marginalia-mcp-resolve-tool-test");

    let called = server.request(
        "tools/call",
        json!({"name": "resolve_comment", "arguments": {"id": UNKNOWN_ID}}),
    );

    assert_eq!(
        called["result"]["isError"],
        json!(true),
        "resolving unknown id {UNKNOWN_ID} was not reported as an error: {called}"
    );
    assert_eq!(
        fs::metadata(&log).expect("log after the call").len(),
        logged_before,
        "resolving unknown id {UNKNOWN_ID} wrote to the log: {called}"
    );
}
