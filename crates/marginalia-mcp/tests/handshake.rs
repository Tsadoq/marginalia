mod common;

use serde_json::json;
use tempfile::TempDir;

use common::StdioServer;

const SERVER_DECLARED_PROTOCOL: &str = "2026-07-28";

/// Not an MCP revision any SDK knows, so negotiation cannot echo it back and the
/// `initialize` reply has to carry the revision the server itself declares.
const UNKNOWN_CLIENT_PROTOCOL: &str = "1970-01-01";

#[test]
fn initialize_replies_with_the_protocol_the_server_declares() {
    let worktree = TempDir::new().expect("temporary worktree");
    let mut server = StdioServer::spawn(worktree.path());

    let initialized = server.request(
        "initialize",
        json!({
            "protocolVersion": UNKNOWN_CLIENT_PROTOCOL,
            "capabilities": {},
            "clientInfo": {"name": "marginalia-mcp-handshake-test", "version": "0.1.0"},
        }),
    );
    assert_eq!(
        initialized["result"]["protocolVersion"], SERVER_DECLARED_PROTOCOL,
        "initialize reply did not declare {SERVER_DECLARED_PROTOCOL}: {initialized}"
    );
}
