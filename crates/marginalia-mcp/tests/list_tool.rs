mod common;

use std::fs;

use marginalia_core::{Comment, Event, Store};
use serde_json::{Value, json};
use tempfile::TempDir;

use common::StdioServer;

const SOURCE_FILE: &str = "src/lib.rs";

const SOURCE: &str = "fn one() {\n    let kept = 1;\n    let also_kept = 2;\n}\n";

const PRESENT_QUOTE: &str = "    let kept = 1;\n    let also_kept = 2;\n";

const ABSENT_QUOTE: &str = "    let deleted = 3;\n";

#[test]
fn list_pending_comments_maps_anchor_state_to_field_set() {
    let worktree = TempDir::new().expect("temporary worktree");
    let source = worktree.path().join(SOURCE_FILE);
    fs::create_dir_all(source.parent().expect("source file has a parent"))
        .expect("create source directory");
    fs::write(&source, SOURCE).expect("write source file");
    let store = Store::open(worktree.path()).expect("open store");
    add(&store, "c1", PRESENT_QUOTE);
    add(&store, "c2", ABSENT_QUOTE);

    let mut server = StdioServer::spawn(worktree.path());
    server.initialize("marginalia-mcp-list-tool-test");

    let called = server.request(
        "tools/call",
        json!({"name": "list_pending_comments", "arguments": {}}),
    );
    let entries = called["result"]["structuredContent"]["comments"]
        .as_array()
        .unwrap_or_else(|| panic!("tools/call reply lists no comments: {called}"))
        .clone();
    assert_eq!(entries.len(), 2, "expected two entries in {called}");

    let anchored = entry(&entries, "c1");
    assert_eq!(anchored["status"], "anchored", "c1 in {called}");
    assert!(anchored.get("lines").is_some(), "c1 in {called}");
    assert!(
        anchored.get("lines_when_written").is_none(),
        "c1 in {called}"
    );

    let drifted = entry(&entries, "c2");
    assert_eq!(drifted["status"], "drifted", "c2 in {called}");
    assert!(
        drifted.get("lines_when_written").is_some(),
        "c2 in {called}"
    );
    assert!(drifted.get("lines").is_none(), "c2 in {called}");
}

fn add(store: &Store, id: &str, quote: &str) {
    store
        .append(&Event::Add(Comment {
            id: id.to_owned(),
            file: SOURCE_FILE.to_owned(),
            start_row: 2,
            end_row: 3,
            quote: quote.to_owned(),
            body: format!("body of {id}"),
            created_at: 0,
        }))
        .unwrap_or_else(|e| panic!("append {id}: {e:#}"));
}

fn entry<'a>(entries: &'a [Value], id: &str) -> &'a Value {
    entries
        .iter()
        .find(|entry| entry["id"] == id)
        .unwrap_or_else(|| panic!("no entry with id {id} among {entries:?}"))
}
