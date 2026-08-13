# marginalia

Leave review comments on code in Zed the way you would in GitLab or Gerrit, anchored to a file
and a line range, and have a coding agent read them back when you ask it to.

Select some lines, press a key, type the remark. The remark is stored with the selected text
quoted verbatim. Later you say "check my review comments" in the agent panel and the agent
lists them, each one carrying the code it was written against, fixes them, and closes them.

Three pieces do that:

- `marginalia`, a CLI a Zed task spawns with the editor's file, row and selection in its environment.
  This is how a comment gets written, because a Zed extension cannot read a buffer or a
  selection.
- `marginalia-mcp`, an MCP server exposing `list_pending_comments` and `resolve_comment`. Zed forwards
  its configured MCP servers to an external agent such as `claude-acp`, and this is the only
  channel from this repository to the agent.
- `.marginalia/comments.jsonl` at the worktree root, an append-only log of add and resolve events. It
  is git-ignored, so your half-formed remarks about your own code never reach a commit.

Comments are pulled, never pushed. Nothing reaches the agent until you ask for it, which costs
you one sentence per turn and in exchange has no failure mode you cannot see.

## Install the two binaries

```
cargo install --path crates/marginalia-cli
cargo install --path crates/marginalia-mcp
```

`marginalia-mcp` must end up on the `PATH` a login shell sees, because that is how the extension finds
it (see below). Check with `sh -lc 'command -v marginalia-mcp'`; if that prints nothing, add
`~/.cargo/bin` to the `PATH` your shell's login profile sets.

## Bind the capture task to a key

`.zed/tasks.json` in this repository already defines the task:

```json
{
  "label": "marginalia: comment on selection",
  "command": "marginalia",
  "args": ["add"],
  "env": {
    "MARGINALIA_FILE": "$ZED_RELATIVE_FILE",
    "MARGINALIA_ROW": "$ZED_ROW",
    "MARGINALIA_SELECTION": "$ZED_SELECTED_TEXT"
  },
  "use_new_terminal": true,
  "reveal": "always",
  "hide": "on_success"
}
```

The editor state travels through `env` rather than `args` because Zed re-tokenises a task's
command string, which a multi-line selection does not survive. `reveal: always` is what gives
the new terminal pane focus so it can take the keystrokes of your comment body.

A key binding cannot be shipped from a repository, so add one yourself. Run `zed: open keymap`
from the command palette and add:

```json
[
  {
    "context": "Editor",
    "bindings": {
      "ctrl-alt-c": ["task::Spawn", { "task_name": "marginalia: comment on selection" }]
    }
  }
]
```

`"context": "Editor"` rather than `"Workspace"`, so the binding fires only where a selection
exists.

Selecting nothing is legal and gives a single-row comment on the cursor's row, because no
key-binding predicate can require a selection to exist first. Zed reports only one row for a
selection, so the range is derived by spanning the selected text's line count upwards from that
row.

Press the key, and a terminal pane opens with `Comment> `. Type the remark, press enter, and the
pane closes. An empty body writes nothing.

## Serve the comments to the agent

Two routes, and either one is enough. The extension is the intended route. The direct entry
needs no extension install and is the one to fall back to.

### Route 1: the Zed extension

1. Build and install the dev extension. Run `zed: install dev extension` from the command
   palette and pick **`crates/marginalia-zed-ext/`**, not the repository root. Zed wants
   `extension.toml`, `Cargo.toml` and `src/lib.rs` in one directory; pointing it at the
   repository root makes it build the whole workspace for wasm and die compiling tokio.

2. Leave the `[[capabilities]]` block in `crates/marginalia-zed-ext/extension.toml` alone:

   ```toml
   [[capabilities]]
   kind = "process:exec"
   command = "/bin/sh"
   args = ["**"]
   ```

   Zed's host refuses an extension's process operations unless the manifest asks for them.
   Without this entry the server never starts and Zed reports a 60 second
   `Context server request timeout` with nothing logged about the spawn.

3. Have `marginalia-mcp` on the login shell's `PATH`. The extension resolves the binary by running
   `/bin/sh -lc 'command -v marginalia-mcp'`, and takes the server's environment from
   `/bin/sh -lc env`, because the wasm guest sees neither your `PATH` nor your environment. If
   the lookup finds nothing the extension says so in the Zed log and names the setting that
   overrides it, `context_servers.marginalia.command.path`.

4. **Enable the server in settings.** This is the step nothing warns you about:

   ```json
   {
     "context_servers": {
       "marginalia": {
         "enabled": true,
         "settings": {}
       }
     }
   }
   ```

   Without it Zed silently never attempts to start the server. There is no error, no timeout and
   no log line, so the symptom is indistinguishable from the extension not existing. This
   repository's `.zed/settings.json` already carries the entry; you need it in your own settings
   for any other project.

### Route 2: a direct settings entry

Name the binary yourself and skip the extension entirely. Put this in the project's
`.zed/settings.json` or in your user settings, substituting the output of
`command -v marginalia-mcp` for the path, since Zed expands neither `~` nor `PATH` here:

```json
{
  "context_servers": {
    "marginalia-direct": {
      "command": "/home/you/.cargo/bin/marginalia-mcp",
      "args": [],
      "env": {}
    }
  }
}
```

An entry that names its own `command` needs no `enabled` key. The server takes the project from
its working directory, which Zed sets to the project root.

## Ask the agent

Open the agent panel on an external agent such as `claude-acp` and say:

```
check my review comments
```

The agent calls `list_pending_comments` and gets back each open comment with its id (`c1`,
`c2`, ...), its file, its current line range, the quoted code and your remark. When it has made
the change it calls `resolve_comment` with the id, and the comment stops appearing. Resolving
appends an event rather than rewriting anything, and resolving twice is harmless.

The quote is authoritative and the line numbers are advisory. Every read searches the current
file for the quoted text, comparing lines with whitespace trimmed at both ends, so reindenting
or inserting code above a comment does not move it. A comment whose quote is no longer anywhere
in its file comes back with `status: drifted` and `lines_when_written`, which is enough for the
agent to find the code from the quote instead of trusting a stale row.

You can also read and close comments from the terminal, with `marginalia list` and `marginalia resolve <id>`.

## Troubleshooting

**Zed keeps running an old server.** After changing anything under `crates/marginalia-mcp` or
`crates/marginalia-core`, run `cargo install --path crates/marginalia-mcp --force` and restart Zed. Zed holds
onto the binary it spawned, so a finished tool can look like an unimplemented stub.

**`cargo build` fails on symbols that plainly exist.** Zed's extension build writes into
`crates/marginalia-zed-ext/target/` while cargo may be building the workspace, which can leave cargo's
fingerprints inconsistent: `marginalia-mcp` failing with `no 'Anchor' in the root` against a `marginalia-core`
that exports it is the shape of this. `cargo clean -p marginalia-core` fixes it.

**The extension will not build at the repository root.** It is not meant to. Install it from
`crates/marginalia-zed-ext/`, and build it by hand from inside that directory with
`cargo build --release --target wasm32-wasip2`.

**The context server does nothing at all.** Re-read step 4 above. A missing `"enabled": true`
produces no output of any kind.
