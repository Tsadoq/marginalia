mod range;

use std::env;
use std::io::{self, Write};
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result, bail};
use marginalia_core::{Anchor, Comment, Event, Store, pending};

use range::derive_range;

const USAGE: &str = "usage: marginalia add | marginalia list | marginalia resolve <id>";

fn main() -> Result<()> {
    let mut args = env::args().skip(1);
    let command = args.next();
    match command.as_deref() {
        Some("add") => add(),
        Some("list") => list(),
        Some("resolve") => match args.next() {
            Some(id) => resolve(&id),
            None => bail!("resolve needs the id of a pending comment\n{USAGE}"),
        },
        Some(other) => bail!("unknown command {other:?}\n{USAGE}"),
        None => bail!(USAGE),
    }
}

/// Store one comment on the selection Zed put in the environment.
///
/// The selection arrives through `MARGINALIA_SELECTION` rather than an argument because
/// Zed re-tokenises a task's command string, which a multi-line value does not
/// survive. `MARGINALIA_FILE` must be relative to the worktree root and `MARGINALIA_ROW`
/// 1-based, which is what the `ZED_RELATIVE_FILE` and `ZED_ROW` the task maps
/// them from give: every later read joins the path onto the worktree root, and
/// an absolute path would silently be read from outside the worktree instead.
fn add() -> Result<()> {
    let file = required_var("MARGINALIA_FILE")?;
    let cursor_row: usize = required_var("MARGINALIA_ROW")?
        .trim()
        .parse()
        .context("MARGINALIA_ROW is not a row number")?;
    let selection = env::var("MARGINALIA_SELECTION").unwrap_or_default();
    let (start_row, end_row) = derive_range(cursor_row, &selection);

    let body = read_body()?;
    if body.is_empty() {
        println!("no comment body, nothing written");
        return Ok(());
    }

    let store = Store::open(worktree_root()?)?;
    let comment = Comment {
        id: store.new_id()?,
        file,
        start_row,
        end_row,
        quote: selection,
        body,
        created_at: now()?,
    };
    let line = format!("{} {}:{start_row}-{end_row}", comment.id, comment.file);
    store.append(&Event::Add(comment))?;
    println!("{line}");
    Ok(())
}

fn list() -> Result<()> {
    for pending in pending(worktree_root()?)? {
        let comment = pending.comment;
        let place = match pending.anchor {
            Anchor::Anchored { start_row, end_row } => {
                format!("{}:{start_row}-{end_row} anchored", comment.file)
            }
            Anchor::Drifted {
                start_row_when_written,
            } => format!("{}:{start_row_when_written} drifted", comment.file),
        };
        println!("{} {place} {}", comment.id, comment.body);
    }
    Ok(())
}

fn resolve(id: &str) -> Result<()> {
    let store = Store::open(worktree_root()?)?;
    if !store.fold()?.iter().any(|comment| comment.id == id) {
        bail!("no pending comment {id}");
    }
    store.append(&Event::Resolve {
        id: id.to_owned(),
        resolved_at: now()?,
    })
}

/// The comment body, typed into the task's terminal pane.
fn read_body() -> Result<String> {
    print!("Comment> ");
    io::stdout().flush().context("writing the prompt")?;
    let mut body = String::new();
    io::stdin()
        .read_line(&mut body)
        .context("reading the comment body")?;
    Ok(body.trim().to_owned())
}

/// The worktree the comment belongs to, which is the directory Zed runs the task
/// in.
fn worktree_root() -> Result<PathBuf> {
    env::current_dir().context("reading the working directory")
}

fn required_var(name: &str) -> Result<String> {
    env::var(name).with_context(|| format!("reading {name} from the environment"))
}

fn now() -> Result<u64> {
    Ok(SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .context("reading the clock")?
        .as_secs())
}
