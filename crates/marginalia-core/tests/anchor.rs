use std::fs;

use marginalia_core::{Anchor, locate, pending};

const QUOTE: &str = "    let total = items.len();\n    total\n";

const ROW_WHEN_WRITTEN: usize = 2;

const SHIFTED_DOWN_BY_TWO: &str = "\
use std::fmt;

fn count(items: &[u8]) -> usize {
    let total = items.len();
    total
}
";

const QUOTE_REWRITTEN_AWAY: &str = "\
fn count(items: &[u8]) -> usize {
    items.len()
}
";

const REINDENTED_ONE_LEVEL_DEEPER: &str = "\
fn wrap() {
    fn count(items: &[u8]) -> usize {
        let total = items.len();
        total
    }
}
";

#[test]
fn locate_follows_shifted_quote_and_reports_drift_when_gone() {
    assert_eq!(
        locate(QUOTE, SHIFTED_DOWN_BY_TWO, ROW_WHEN_WRITTEN),
        Anchor::Anchored {
            start_row: 4,
            end_row: 5
        },
        "a quote that moved down the file must be reported at its new rows"
    );

    assert_eq!(
        locate(QUOTE, QUOTE_REWRITTEN_AWAY, ROW_WHEN_WRITTEN),
        Anchor::Drifted {
            start_row_when_written: ROW_WHEN_WRITTEN
        },
        "a quote that no longer occurs must drift, carrying the original row"
    );
}

#[test]
fn locate_anchors_a_quote_whose_indentation_changed() {
    assert_eq!(
        locate(QUOTE, REINDENTED_ONE_LEVEL_DEEPER, ROW_WHEN_WRITTEN),
        Anchor::Anchored {
            start_row: 3,
            end_row: 4
        },
        "a quote the agent only reindented must anchor at its new rows, not drift"
    );
}

#[test]
fn pending_drifts_a_comment_whose_file_is_gone() {
    const ROW_IN_THE_DELETED_FILE: usize = 7;

    let worktree = tempfile::TempDir::new().expect("temp worktree");
    let source = worktree.path().join("src").join("count.rs");
    fs::create_dir_all(source.parent().expect("source has a parent")).expect("create src");
    fs::write(&source, SHIFTED_DOWN_BY_TWO).expect("write source");
    let log = worktree.path().join(".marginalia").join("comments.jsonl");
    fs::create_dir_all(log.parent().expect("log has a parent")).expect("create .marginalia");
    fs::write(
        &log,
        concat!(
            r#"{"type":"Add","id":"c1","file":"src/count.rs","start_row":2,"end_row":3,"quote":"    let total = items.len();\n    total\n","body":"inline it","created_at":1000}"#,
            "\n",
            r#"{"type":"Add","id":"c2","file":"src/gone.rs","start_row":7,"end_row":7,"quote":"fn removed() {}","body":"drop this","created_at":1001}"#,
            "\n",
        ),
    )
    .expect("write log");

    let anchors: Vec<(String, Anchor)> = pending(worktree.path())
        .expect("pending reads the worktree")
        .into_iter()
        .map(|entry| (entry.comment.id, entry.anchor))
        .collect();

    assert_eq!(
        anchors,
        vec![
            (
                "c1".to_owned(),
                Anchor::Anchored {
                    start_row: 4,
                    end_row: 5
                }
            ),
            (
                "c2".to_owned(),
                Anchor::Drifted {
                    start_row_when_written: ROW_IN_THE_DELETED_FILE
                }
            ),
        ],
        "a comment must anchor against its own file, and one whose file is gone must drift"
    );
}
