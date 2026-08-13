use std::collections::HashSet;
use std::fs;
use std::io::{ErrorKind, Write};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

/// A review comment as it was written.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Comment {
    pub id: String,
    pub file: String,
    pub start_row: usize,
    pub end_row: usize,
    pub quote: String,
    pub body: String,
    pub created_at: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Event {
    Add(Comment),
    Resolve { id: String, resolved_at: u64 },
}

/// What the log says about one id: an id no `Add` ever used is `Unknown`, which
/// is what separates a mistyped id from one whose comment is already closed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommentState {
    Unknown,
    Pending,
    Resolved,
}

/// The comment log of one worktree. Resolving a comment appends a `Resolve`,
/// and nothing here ever rewrites or truncates, which is what lets a writing
/// process and a reading one share the log without locking.
pub struct Store {
    log: PathBuf,
}

impl Store {
    pub fn open(worktree_root: impl AsRef<Path>) -> Result<Self> {
        let dir = worktree_root.as_ref().join(".marginalia");
        fs::create_dir_all(&dir).with_context(|| format!("creating {}", dir.display()))?;
        Ok(Self {
            log: dir.join("comments.jsonl"),
        })
    }

    pub fn append(&self, event: &Event) -> Result<()> {
        let mut line = serde_json::to_string(event).context("serialising event")?;
        line.push('\n');
        let mut log = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.log)
            .with_context(|| format!("opening {} to append", self.log.display()))?;
        log.write_all(line.as_bytes())
            .with_context(|| format!("appending to {}", self.log.display()))
    }

    /// The comments with no later `Resolve`, in the order they were added.
    pub fn fold(&self) -> Result<Vec<Comment>> {
        let mut added: Vec<Comment> = Vec::new();
        let mut resolved: HashSet<String> = HashSet::new();
        for event in self.events()? {
            match event {
                Event::Add(comment) => added.push(comment),
                Event::Resolve { id, .. } => {
                    resolved.insert(id);
                }
            }
        }
        added.retain(|comment| !resolved.contains(&comment.id));
        Ok(added)
    }

    /// Where one id stands, by the same last-event-wins rule as `fold`.
    pub fn state_of(&self, id: &str) -> Result<CommentState> {
        let mut state = CommentState::Unknown;
        for event in self.events()? {
            match event {
                Event::Add(comment) if comment.id == id => state = CommentState::Pending,
                Event::Resolve { id: resolved, .. } if resolved == id => {
                    state = CommentState::Resolved;
                }
                _ => {}
            }
        }
        Ok(state)
    }

    /// An id no earlier comment used, counted over every `Add` ever written so
    /// that resolving a comment cannot free its id for reuse.
    pub fn new_id(&self) -> Result<String> {
        let adds = self
            .events()?
            .iter()
            .filter(|event| matches!(event, Event::Add(_)))
            .count();
        Ok(format!("c{}", adds + 1))
    }

    /// Every event the log parses as. A line that does not parse is skipped, so
    /// the torn tail a killed writer leaves behind cannot fail the read.
    fn events(&self) -> Result<Vec<Event>> {
        let bytes = match fs::read(&self.log) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == ErrorKind::NotFound => return Ok(Vec::new()),
            Err(error) => {
                return Err(error).with_context(|| format!("reading {}", self.log.display()));
            }
        };
        Ok(bytes
            .split(|byte| *byte == b'\n')
            .filter_map(|line| serde_json::from_slice(line).ok())
            .collect())
    }
}
