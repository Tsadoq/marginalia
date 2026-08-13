#![allow(dead_code)]

use std::{
    io::{BufRead, BufReader, Write},
    path::Path,
    process::{Child, ChildStdin, Command, Stdio},
    sync::mpsc::{Receiver, RecvTimeoutError, channel},
    thread,
    time::Duration,
};

use serde_json::{Value, json};

const REPLY_TIMEOUT: Duration = Duration::from_secs(10);

/// The revision a client offers when it is not the revision itself that is
/// under test.
const CLIENT_PROTOCOL: &str = "2026-07-28";

/// The `marginalia-mcp` binary running as a child process, driven over its real stdio
/// pipes with its working directory set to a worktree root.
pub struct StdioServer {
    child: Child,
    stdin: ChildStdin,
    replies: Receiver<String>,
    next_id: u32,
}

impl StdioServer {
    pub fn spawn(worktree_root: &Path) -> Self {
        let mut child = Command::new(env!("CARGO_BIN_EXE_marginalia-mcp"))
            .current_dir(worktree_root)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .expect("spawn marginalia-mcp");
        let stdin = child.stdin.take().expect("piped stdin");
        let stdout = child.stdout.take().expect("piped stdout");

        let (tx, replies) = channel();
        thread::spawn(move || {
            for line in BufReader::new(stdout).lines().map_while(Result::ok) {
                if tx.send(line).is_err() {
                    break;
                }
            }
        });

        Self {
            child,
            stdin,
            replies,
            next_id: 1,
        }
    }

    /// Complete the handshake, after which the server answers tool calls. Tests
    /// that drive the handshake itself send `initialize` through `request`
    /// instead.
    pub fn initialize(&mut self, client_name: &str) {
        self.request(
            "initialize",
            json!({
                "protocolVersion": CLIENT_PROTOCOL,
                "capabilities": {},
                "clientInfo": {"name": client_name, "version": "0.1.0"},
            }),
        );
        self.notify("notifications/initialized", json!({}));
    }

    /// The reply to one request, ignoring anything the server sends in the
    /// meantime that is not addressed to it.
    pub fn request(&mut self, method: &str, params: Value) -> Value {
        let id = self.next_id;
        self.next_id += 1;
        self.send(json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params}));
        loop {
            let reply: Value = serde_json::from_str(&self.next_line(method))
                .unwrap_or_else(|e| panic!("reply to {method} is not JSON: {e}"));
            if reply["id"] == json!(id) {
                return reply;
            }
        }
    }

    pub fn notify(&mut self, method: &str, params: Value) {
        self.send(json!({"jsonrpc": "2.0", "method": method, "params": params}));
    }

    fn send(&mut self, message: Value) {
        writeln!(self.stdin, "{message}").expect("write to server stdin");
        self.stdin.flush().expect("flush server stdin");
    }

    fn next_line(&mut self, method: &str) -> String {
        match self.replies.recv_timeout(REPLY_TIMEOUT) {
            Ok(line) => line,
            Err(RecvTimeoutError::Timeout) => {
                panic!("no reply to {method} within {REPLY_TIMEOUT:?}")
            }
            Err(RecvTimeoutError::Disconnected) => {
                panic!("server closed stdout before replying to {method}")
            }
        }
    }
}

impl Drop for StdioServer {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
