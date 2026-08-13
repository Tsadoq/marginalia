mod anchor;
mod store;

use std::fs;
use std::path::Path;

use anyhow::Result;

pub use anchor::{Anchor, AnchoredComment, locate};
pub use store::{Comment, CommentState, Event, Store};

/// Every unresolved comment of a worktree, each re-located against the file as
/// it reads now, reading `Comment::file` as a path relative to `worktree_root`.
///
/// A comment whose file cannot be read is reported as drifted, so one deleted
/// or unreadable file cannot cost the caller every other comment.
pub fn pending(worktree_root: impl AsRef<Path>) -> Result<Vec<AnchoredComment>> {
    let root = worktree_root.as_ref();
    let anchored = Store::open(root)?
        .fold()?
        .into_iter()
        .map(|comment| {
            let anchor = match fs::read_to_string(root.join(&comment.file)) {
                Ok(file_text) => locate(&comment.quote, &file_text, comment.start_row),
                Err(_) => Anchor::Drifted {
                    start_row_when_written: comment.start_row,
                },
            };
            AnchoredComment { comment, anchor }
        })
        .collect();
    Ok(anchored)
}
