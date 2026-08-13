mod server;

use anyhow::{Context, Result};
use rmcp::{ServiceExt, service::QuitReason, transport::stdio};

use crate::server::MarginaliaServer;

#[tokio::main]
async fn main() -> Result<()> {
    let service = MarginaliaServer::new().serve(stdio()).await?;
    match service.waiting().await? {
        QuitReason::JoinError(e) => Err(e).context("mcp service task failed"),
        _ => Ok(()),
    }
}
