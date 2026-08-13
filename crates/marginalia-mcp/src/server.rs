use std::{
    env,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

use marginalia_core::{Anchor, AnchoredComment, CommentState, Event, Store};
use rmcp::{
    ErrorData, Json, ServerHandler,
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    model::{CallToolResult, ContentBlock, ProtocolVersion, ServerCapabilities, ServerInfo},
    schemars::{self, JsonSchema},
    tool, tool_handler, tool_router,
};
use serde::{Deserialize, Serialize};

/// The MCP server exposing Zed review comments as tools.
#[derive(Clone)]
pub struct MarginaliaServer {
    tool_router: ToolRouter<Self>,
}

impl MarginaliaServer {
    pub fn new() -> Self {
        Self {
            tool_router: Self::tool_router(),
        }
    }
}

#[tool_router]
impl MarginaliaServer {
    #[tool(
        description = "List the review comments the reviewer has left open in this project. \
                       Each entry quotes the code it was written against; that quote is \
                       authoritative and the line numbers are advisory, because the file may \
                       have changed since. An entry with status `drifted` no longer matches \
                       anywhere in its file, so find the code by its quote rather than by \
                       `lines_when_written`."
    )]
    fn list_pending_comments(&self) -> Result<Json<Listing>, ErrorData> {
        let pending = marginalia_core::pending(worktree_root()?).map_err(|e| {
            ErrorData::internal_error(format!("reading pending comments: {e:#}"), None)
        })?;
        Ok(Json(Listing {
            comments: pending.into_iter().map(PendingComment::from).collect(),
        }))
    }

    /// A refused resolve is a tool-level error, which reaches the agent as
    /// content it can act on; `ErrorData` is for a fault it cannot, because
    /// clients render those opaquely. Repeating the call on a comment that is
    /// already closed succeeds without appending, so an agent that retries
    /// cannot grow the log.
    #[tool(
        description = "Close one review comment, so it stops appearing in the pending list. \
                       Resolve a comment only once the change it asks for has been made, \
                       because resolving it is what tells the reviewer it is done. \
                       Resolving a comment that is already closed is harmless."
    )]
    fn resolve_comment(
        &self,
        Parameters(ResolveRequest { id }): Parameters<ResolveRequest>,
    ) -> Result<CallToolResult, ErrorData> {
        let store = Store::open(worktree_root()?).map_err(|e| {
            ErrorData::internal_error(format!("opening the comment log: {e:#}"), None)
        })?;
        match store
            .state_of(&id)
            .map_err(|e| ErrorData::internal_error(format!("reading comment {id}: {e:#}"), None))?
        {
            CommentState::Unknown => Ok(CallToolResult::error(vec![ContentBlock::text(format!(
                "No comment has id {id}, so nothing was resolved. \
                 Call list_pending_comments for the ids that are pending."
            ))])),
            CommentState::Resolved => Ok(CallToolResult::success(vec![ContentBlock::text(
                format!("Comment {id} was already resolved, so nothing changed."),
            )])),
            CommentState::Pending => {
                store
                    .append(&Event::Resolve {
                        id: id.clone(),
                        resolved_at: now()?,
                    })
                    .map_err(|e| {
                        ErrorData::internal_error(format!("resolving comment {id}: {e:#}"), None)
                    })?;
                Ok(CallToolResult::success(vec![ContentBlock::text(format!(
                    "Resolved comment {id}."
                ))]))
            }
        }
    }
}

#[derive(Deserialize, JsonSchema)]
struct ResolveRequest {
    /// The id of the comment to close, as `list_pending_comments` reported it.
    id: String,
}

/// The project the server was spawned for. Zed sets the working directory of a
/// server it spawns to the project root, and passes nothing else that would
/// identify it.
fn worktree_root() -> Result<PathBuf, ErrorData> {
    env::current_dir()
        .map_err(|e| ErrorData::internal_error(format!("reading working directory: {e}"), None))
}

fn now() -> Result<u64, ErrorData> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|since_epoch| since_epoch.as_secs())
        .map_err(|e| ErrorData::internal_error(format!("reading the clock: {e}"), None))
}

#[derive(Serialize, JsonSchema)]
struct Listing {
    comments: Vec<PendingComment>,
}

#[derive(Serialize, JsonSchema)]
struct PendingComment {
    id: String,
    file: String,
    #[serde(flatten)]
    position: Position,
    quote: String,
    body: String,
}

/// Where a pending comment's code sits. A drifted comment reports the rows it
/// was written against, taken from the comment rather than from the anchor,
/// which carries only their start.
#[derive(Serialize, JsonSchema)]
#[serde(tag = "status", rename_all = "snake_case")]
enum Position {
    Anchored { lines: String },
    Drifted { lines_when_written: String },
}

impl From<AnchoredComment> for PendingComment {
    fn from(AnchoredComment { comment, anchor }: AnchoredComment) -> Self {
        let position = match anchor {
            Anchor::Anchored { start_row, end_row } => Position::Anchored {
                lines: row_range(start_row, end_row),
            },
            Anchor::Drifted { .. } => Position::Drifted {
                lines_when_written: row_range(comment.start_row, comment.end_row),
            },
        };
        Self {
            id: comment.id,
            file: comment.file,
            position,
            quote: comment.quote,
            body: comment.body,
        }
    }
}

fn row_range(start_row: usize, end_row: usize) -> String {
    if start_row == end_row {
        start_row.to_string()
    } else {
        format!("{start_row}-{end_row}")
    }
}

#[tool_handler(router = self.tool_router)]
impl ServerHandler for MarginaliaServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build())
            .with_protocol_version(ProtocolVersion::V_2026_07_28)
    }
}
