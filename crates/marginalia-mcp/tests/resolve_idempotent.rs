mod common;

use std::fs;

use marginalia_core::{Comment, Event, Store};
use serde_json::{Value, json};
use tempfile::TempDir;

use common::StdioServer;

const KNOWN_ID: &str = "c1";

#[test]
fn resolve_comment_records_one_event_however_often_called() {
    let worktree = TempDir::new().expect("temporary worktree");
    Store::open(worktree.path())
        .expect("open store")
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

    let mut server = StdioServer::spawn(worktree.path());
    server.initialize("marginalia-mcp-resolve-idempotent-test");

    let first = resolve(&mut server);
    assert_eq!(
        first["result"]["isError"],
        json!(false),
        "resolving pending id {KNOWN_ID} did not succeed: {first}"
    );

    let second = resolve(&mut server);
    assert_eq!(
        second["result"]["isError"],
        json!(false),
        "resolving {KNOWN_ID} a second time did not succeed: {second}"
    );

    let listed = server.request(
        "tools/call",
        json!({"name": "list_pending_comments", "arguments": {}}),
    );
    let entries = listed["result"]["structuredContent"]["comments"]
        .as_array()
        .unwrap_or_else(|| panic!("tools/call reply lists no comments: {listed}"));
    assert!(
        entries.is_empty(),
        "resolved comment {KNOWN_ID} is still pending: {listed}"
    );

    assert_eq!(
        resolve_events(worktree.path().join(".marginalia").join("comments.jsonl")),
        1,
        "two resolve calls did not append exactly one Resolve event"
    );
}

fn resolve(server: &mut StdioServer) -> Value {
    server.request(
        "tools/call",
        json!({"name": "resolve_comment", "arguments": {"id": KNOWN_ID}}),
    )
}

fn resolve_events(log: std::path::PathBuf) -> usize {
    fs::read_to_string(&log)
        .unwrap_or_else(|e| panic!("reading {}: {e}", log.display()))
        .lines()
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
        .filter(|event| event["type"] == json!("Resolve"))
        .count()
}
