use std::fs;

use marginalia_core::{Comment, CommentState, Event, Store};

#[test]
fn fold_drops_resolved_and_skips_torn_lines() {
    let worktree = tempfile::TempDir::new().expect("temp worktree");
    let log = worktree.path().join(".marginalia").join("comments.jsonl");
    fs::create_dir_all(log.parent().expect("log has a parent")).expect("create .marginalia");
    fs::write(
        &log,
        concat!(
            r#"{"type":"Add","id":"c1","file":"src/a.rs","start_row":10,"end_row":12,"quote":"let x = 1;","body":"rename this","created_at":1000}"#,
            "\n",
            r#"{"type":"Add","id":"c2","file":"src/b.rs","start_row":3,"end_row":3,"quote":"fn main() {}","body":"needs a doc","created_at":1001}"#,
            "\n",
            r#"{"type":"Resolve","id":"c1","resolved_at":1002}"#,
            "\n",
            r#"{"type":"Add","id":"c3","file":"src/c."#,
        ),
    )
    .expect("write log");

    let comments = Store::open(worktree.path())
        .expect("open store")
        .fold()
        .expect("fold tolerates the torn line");

    assert_eq!(
        comments.iter().map(|c| c.id.as_str()).collect::<Vec<_>>(),
        vec!["c2"]
    );
}

#[test]
fn state_of_separates_an_unknown_id_from_a_resolved_one() {
    let worktree = tempfile::TempDir::new().expect("temp worktree");
    let store = Store::open(worktree.path()).expect("open store");
    add(&store, "c1");
    add(&store, "c2");
    store
        .append(&Event::Resolve {
            id: "c1".to_owned(),
            resolved_at: 1002,
        })
        .expect("append resolve");

    assert_eq!(
        store.state_of("c1").expect("state of c1"),
        CommentState::Resolved,
        "c1 has a Resolve event"
    );
    assert_eq!(
        store.state_of("c2").expect("state of c2"),
        CommentState::Pending,
        "c2 was added and never resolved"
    );
    assert_eq!(
        store.state_of("c3").expect("state of c3"),
        CommentState::Unknown,
        "no Add event ever used c3"
    );
}

#[test]
fn new_id_does_not_reuse_the_id_of_a_resolved_comment() {
    let worktree = tempfile::TempDir::new().expect("temp worktree");
    let store = Store::open(worktree.path()).expect("open store");
    store
        .append(&Event::Add(Comment {
            id: store.new_id().expect("first id"),
            file: "src/a.rs".to_owned(),
            start_row: 1,
            end_row: 1,
            quote: "let x = 1;".to_owned(),
            body: "rename this".to_owned(),
            created_at: 1000,
        }))
        .expect("append add");
    store
        .append(&Event::Resolve {
            id: "c1".to_owned(),
            resolved_at: 1001,
        })
        .expect("append resolve");

    assert_eq!(store.new_id().expect("second id"), "c2");
}

fn add(store: &Store, id: &str) {
    store
        .append(&Event::Add(Comment {
            id: id.to_owned(),
            file: "src/a.rs".to_owned(),
            start_row: 1,
            end_row: 1,
            quote: "let x = 1;".to_owned(),
            body: format!("body of {id}"),
            created_at: 1000,
        }))
        .unwrap_or_else(|e| panic!("append {id}: {e:#}"));
}
