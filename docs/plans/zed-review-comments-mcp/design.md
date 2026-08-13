# Design: Zed review comments for ACP agents

## Background

Code review tools let a reviewer attach a remark to a file and a line range. Talking to a coding agent in Zed's agent panel offers nothing equivalent: the observation has to be retyped as prose, and the anchor, the thing that made the remark cheap to write, is lost. This project restores the anchor. The reviewer selects lines, records a remark, and the agent later reads back a list of remarks each carrying the code it was about.

Three constraints shape everything that follows. First, Zed 1.14.2 talks to `claude-acp` over the Agent Client Protocol (ACP), a JSON-RPC protocol carried on the agent process's stdin and stdout, and Zed owns both ends of that pipe. Second, a Zed extension is a WebAssembly module whose host functions cover downloads, npm, GitHub releases, Node, and platform queries, and nothing else. It cannot read a buffer, a selection, or a cursor, cannot register a command or a key binding, and cannot draw anything. Third, extension-provided ACP agents were deprecated in Zed v1.5.0 in favour of a central registry, which removes the one hook that would have let an extension sit in the prompt path.

What survives those constraints is narrow. An extension may declare a Model Context Protocol (MCP) context server, a subprocess speaking a tool-calling protocol, and Zed forwards its configured MCP servers to external agents over ACP. That is the only channel from an extension to the agent, and it is a pull channel: the agent calls a tool when it decides to. Capture has to happen entirely outside the extension, which leaves Zed's task system, a separate feature that does have access to the editor's file and selection through `ZED_*` environment variables and can be bound to a key.

## How do pending comments reach the model?

The agent calls a `list_pending_comments` tool on an MCP server, and only when the user asks it to in plain language. Four routes were weighed. A push into the next prompt, which is what the original request described, would need a process sitting between Zed and the agent rewriting `session/prompt`; that is buildable as an ACP middleware, but it cannot ship as an extension since the v1.5.0 deprecation, and it would mean abandoning the registry-based `claude-acp` entry for a hand-rolled command entry. An MCP resource that the user @-mentions would put the comments into the prompt as literal text, but Zed's documentation does not confirm that resources from an external agent's forwarded servers appear in the mention picker at all. A tool plus an AGENTS.md instruction telling the agent to check comments every turn would approximate a push, at the cost of making correctness depend on the agent obeying a written instruction on every single turn.

The explicit pull won because it has no failure mode that is hard to see. If the user does not ask, nothing happens, which is obvious; whereas a nudge that the agent quietly ignores looks identical to having no pending comments. The cost is one sentence per turn, something like "check my review comments", and that cost is paid knowingly.

One thing is not yet established. Zed's own wording is that "Zed-configured MCP servers may be forwarded to External Agents over ACP", and neither that page nor the external-agents page says whether a server contributed by an extension counts, as opposed to one written by hand into `settings.json`. Task 5 tests it directly with a one-tool server. If extension-contributed servers turn out not to be forwarded, the fallback is a `settings.json` entry pointing at the same binary, which costs the user one manual configuration step and makes the extension crate redundant.

For the implementer this means the MCP server is the product surface and must be self-describing. The `list_pending_comments` tool description is read by the model with no other context, so it states that the quoted code is authoritative and the line numbers are advisory.

## How does a comment get authored?

A Zed task, bound to a key, spawns the `cfc` CLI with the editor's state in its environment, and the CLI prompts for the comment body in the task's terminal pane. The alternatives all avoided the task system and paid for it. In-file `// @review:` markers need no tooling at all and put the comment exactly where the code is, but they dirty the buffer, they have to be removed afterwards, and a marker inside a string literal or a language without line comments becomes a puzzle. A hand-edited review buffer keeps the repository clean but makes the reviewer type file paths and line numbers, which is precisely the work this project exists to remove. A bare CLI has the same problem with none of the editor integration.

The task route is the only one where selecting lines and pressing a key is the whole interaction, because `ZED_FILE`, `ZED_ROW`, and `ZED_SELECTED_TEXT` arrive automatically. It carries two known sharp edges. Zed exposes no start and end row for a selection, only a single row, a gap tracked as an open issue since June 2024, so the range is derived by spanning the selected text's line count from that row and clamping at the first row. And the selection text is multi-line, so it travels through `env` rather than `args`: the task documentation warns that variables embedded in a command string need manual escaping, and an environment value is never re-tokenised.

One thing is not yet established here either. No Zed documentation states whether a spawned task's terminal accepts typed input. The surrounding evidence leans yes, since tasks run in the ordinary integrated terminal widget and the `reveal` option exists to give that pane focus, but leaning is not knowing. Task 1 settles it with a throwaway `read` task before task 8 is written. If input does not round-trip, the body has to come from a scratch buffer opened with `zed --wait` instead of a prompt, which changes the CLI's input path but nothing else in this design.

For the implementer this means `cfc add` never trusts its inputs to be well-formed: an empty selection is a valid single-row comment, because no documented key-binding predicate can require a selection to exist before the task fires.

## Where do comments live?

Comments live in `.cfc/comments.jsonl` at the worktree root, as an append-only log of `Add` and `Resolve` events that is folded into current state on every read. The competing options each traded away something this one keeps. A single rewritten JSON file is the simplest thing that works until the CLI writes while the server reads, at which point a truncated read is possible. SQLite makes concurrency somebody else's problem but adds a dependency and a store that cannot be inspected with `cat`. A tracked markdown file in the repository would make review threads diffable and shareable, which is genuinely attractive, but it puts half-formed remarks about one's own code into the commit history and turns concurrent edits into merge conflicts.

Append-only won on a measured result rather than an argument. Eight processes appending two hundred byte lines concurrently through `O_APPEND` produced four thousand lines with none interleaved or lost, which is the entire concurrency story for this workload: writers only ever append, and the reader tolerates a torn final line by skipping unparseable lines instead of failing the read. That last rule matters more than it looks, because it is what makes a killed `cfc add` a non-event rather than a corrupted store.

For the implementer this means resolving a comment appends an event and never rewrites the file, and no code path may open the log for writing in truncate mode.

## How does a comment survive the agent editing the file?

The quoted text is stored verbatim and searched for in the current file every time comments are read, so the reported line range is always the range as of that moment, and a comment whose quote has vanished is reported as `drifted` rather than pointed somewhere wrong. Storing line numbers alone was never viable, since the agent editing anything above a comment silently invalidates it. Storing the quote and reporting it as a hint, leaving the model to find the code, would work well in practice and needs no matching code at all, but it pushes work onto every reader and gives the reviewer no way to see that a comment has gone stale. Recording the file's git blob SHA and shifting the range through a diff is the most precise option and survives reformatting that a text search would miss, but it needs the file to have been committed, which is exactly what a file the agent just wrote has not been.

Searching for the quote also gives a place to be deliberately forgiving: lines are compared with whitespace trimmed at both ends, so reindentation alone does not count as drift, and when a quote occurs more than once the occurrence nearest the original row wins. When the quote is genuinely gone the result is `Drifted`, never a guess, because a comment silently attached to the wrong code is worse than one that admits it is lost.

Trimming the leading whitespace is what makes an agent's reindentation survive, and it has a cost we accept: two lines that differ only in indentation now compare equal, so in an indentation-significant language such as Python a quote can match a block that is structurally different from the one it was written against. The nearest-hint-row tiebreak is the only mitigation, and it is enough here, because reindentation is a routine outcome of an agent editing code while a same-text-different-block collision needs the file to repeat the quote verbatim at another nesting depth. No further matching rules are added to defeat it.

For the implementer this means the drift state is part of the tool's public contract, not an internal detail, and callers of `cfc_core::pending` never see raw events, folding, or file reads.

## What is this written in?

Rust, in one cargo workspace holding four crates. The extension crate has no choice: Zed extensions compile to `wasm32-wasip2` and Rust is the only supported source language, so that part of the decision was made by the platform. What was open is the server and the CLI. TypeScript would have made distribution smoothest, because `npm_install_package` is a host function Zed offers extensions specifically so they can fetch an MCP server, and the TypeScript MCP SDK is the most exercised. Python is the most familiar language here, but Zed has no pip or uv host function, so the extension would have to shell out to `uvx`, the least supported route of the three.

Rust won because the extension crate is Rust regardless, and a second toolchain buys nothing once the whole thing ships as one static binary that `context_server_command()` simply names. The official Rust MCP SDK, `rmcp`, is at 3.1.2 and implements the current `2026-07-28` protocol revision, so nothing is being built on a fringe library.

Two facts constrain the build. `rmcp` 3.1.2 declares a minimum supported Rust version of 1.88 and edition 2024, and the toolchain here is exactly 1.88.0, so there is no headroom: an `rmcp` update that raises the floor forces a toolchain upgrade. And a `wasm32-wasip2` crate inside a workspace of native crates can make a workspace-wide build reach for the host linker, which cannot resolve WASI imports.

For the implementer this means the extension crate is listed as a workspace member but kept out of `default-members`, so `cargo build` skips it and it is built explicitly with `--target wasm32-wasip2`. The MCP Rust SDK's own repository handles its WASI example the same way.

## Implementation notes

stdin spike outcome: interactive. A typed line round-tripped through `read` in the spawned task terminal. Zed wrapped the task as `/usr/bin/zsh -i -c 'sh'` and the inner command string was re-tokenised, which raised a spurious `printf: usage:` before the prompt without stopping the read. That re-tokenisation is the reason the selection must travel through `env`.

### Task 2: Scaffold the cargo workspace with the WASM crate excluded

Cargo rejects a manifest with no target, so each crate also got a placeholder root (`src/lib.rs` empty, `src/main.rs` as `fn main() {}`) that tasks 3, 4, 6 and 8 overwrite. Dependencies those tasks need but cannot add — they own only source files — were declared here: `anyhow`, `serde` with `derive`, and `serde_json` in `[workspace.dependencies]`, plus `serde_json` as a `cfc-mcp` dev-dependency for the task 3 handshake test.

`rmcp` default features are `base64`, `macros`, `server`, none of which include a transport, so stdio requires `transport-io` explicitly. `default-members` makes plain `cargo build` skip `cfc-zed-ext` as intended, but `cargo build --workspace` still builds it for the host; that only links today because `lib.rs` is empty, and task 4 should expect the workspace-wide build to stop working once `register_extension!` lands.

The machine had no `1.88.0` rustup toolchain (only `stable`, which is 1.88.0) and no `wasm32-wasip2`; both were installed, with `rustfmt` and `clippy` added since the install used the minimal profile.

### Task 3: Serve one MCP tool over stdio

`rmcp`'s `ProtocolVersion::LATEST` is `2025-11-25`, not `2026-07-28`, so `get_info` must call `with_protocol_version` explicitly. `#[tool_handler]` defaults its router to `Self::tool_router()`, which rebuilds the router on every `call_tool` and leaves the cached field dead; `#[tool_handler(router = self.tool_router)]` uses the field instead.

The handshake test sends `1970-01-01` as the client's requested revision. rmcp echoes any revision the server supports, and its default `supported_protocol_versions()` already contains `2026-07-28`, so a realistic client offer made the assertion pass even with `with_protocol_version` deleted. Only an unknown offer forces negotiation to fall back to the revision the server actually declares. Verified by mutation both ways.

Not addressed here: `serverInfo` still reports `rmcp`/`3.1.2` rather than the binary's own name and version, because task 3 owns only the protocol revision.

### Task 4: Expose the server to Zed as a context server

`extension.toml` needs a `repository` field but the checkout has no git remote, so `https://github.com/matteovillosio/comment-for-claude-zed` is a placeholder to correct before publishing. `authors` came from git config.

Task 2 predicted `cargo build --workspace` would stop linking once `register_extension!` landed; it still passes, because the macro puts its wasi-libc `chdir` shim behind `#[cfg(target_os = "wasi")]`, leaving nothing WASI-specific in the host build.

`std::env::var("CFC_MCP_BINARY")` is read inside the WASM guest, so the value has to reach the extension host's environment, not just the terminal that launched Zed. Superseded: task 5 showed neither half of this worked, and task 15 removed the variable and replaced the bare-name fallback with settings-then-login-shell resolution.

### Task 6: Fold an append-only event log into comment state

`Event::Add` is a newtype over `Comment` with `#[serde(tag = "type")]`, so one field list describes both the stored line and the folded value; the wire form stays `{"type":"Add","id":...}`. `Comment::new_id()` became `Store::new_id()` because the counter has to run over every `Add` ever written, which only the store can read, and counting survivors instead would hand a resolved comment's id to the next one.

The log is read with `fs::read` and split on `b'\n'` rather than through `BufReader::lines()`: a torn tail can split a multi-byte character, and `lines()` returns `Err` on invalid UTF-8, which would have failed exactly the read the skip rule exists to protect. `append` glues the newline onto the serialised line so one event is one `write_all`; two writes would let a concurrent appender land between the JSON and its newline.

`tempfile` was added to `[workspace.dependencies]` and as a `cfc-core` dev-dependency. Nothing tests `Store::append` under real concurrency; the eight-writer result in the probes is the only evidence for that path.

### Task 7: Re-locate each comment against the current file

`pending` lives in `lib.rs` rather than `anchor.rs` so `anchor.rs` stays a pure function over strings, which is what the plan's test level assumes; `locate` has to be `pub` because the mandated test lives in `tests/`, compiled as its own crate.

`Comment::file` is read as relative to the worktree root, and that is now stated on `pending` because `Path::join` silently discards the root when handed an absolute path, which would read a file outside the worktree and report a confident anchor into it. Rows are 1-based, stated on `Anchor` itself since that is the type a caller lands on.

A second test beyond the plan's covers `pending`: task 9's planned test gives both its comments existing files, so "a comment whose file no longer exists is `Drifted`" would otherwise be tested nowhere. It could not be proven red, the code being already green, so it was proven to bite by mutation instead.

### Task 8: Capture a comment from the editor selection

A binary crate exposes nothing to an integration test, so `tests/range.rs` pulls the module in with `#[path = "../src/range.rs"] mod range;`; the alternative, splitting a `lib.rs` out of `cfc-cli`, was outside the task's target files.

`derive_range` reads the cursor as the *last* row of the selection, which the plan leaves open. That choice is what makes the clamp reachable at all: with the cursor as the start row the start can never fall below 1 and the clamping rule would be dead code. Clamping preserves the span rather than truncating it, so row 2 with a four-line selection gives `1..=4` — the only four-row range containing row 2 that starts at row 1.

`cfc resolve` rejects an id that `Store::fold` does not report as pending, so a mistyped id is not a silent no-op. The message cannot say whether the id is unknown or already resolved, because `fold` only returns survivors; task 11's `Store::is_pending` is what would let it distinguish the two, and the CLI is deliberately not idempotent the way task 11 makes the MCP tool.

### Task 9: Replace the ping tool with the comment listing tool

`lines` is a display string (`"2-3"`, or `"2"` when the range is one row) rather than a pair of numbers, because a drifted entry would otherwise have to report a one-field range next to the anchored two-field one. The drifted range comes from `Comment::start_row`/`end_row`, not from `Anchor::Drifted::start_row_when_written`, which only knows the start. Nothing pins that format; the plan's dedup rule forbids this test asserting row values.

The result is `Json<Listing>`, so rmcp fills both `structuredContent` and a JSON text block. The wrapping `Listing { comments }` exists because MCP requires `structuredContent` to be an object, not an array. `JsonSchema` derives expand to `schemars::` paths, so `use rmcp::schemars::{self, JsonSchema}` is needed rather than the plain trait import; `serde` became a `cfc-mcp` dependency and `tempfile` a dev-dependency.

`tests/handshake.rs` kept its mutation-verified protocol-fallback assertion and lost the `ping` half, along with its now-unused `notify` helper. Its `StdioServer` harness is duplicated verbatim in `tests/list_tool.rs` (plus `current_dir`); tasks 10 and 11 need it too, so it wants a shared `tests/` module, which no task's file list allows.

### Task 10: Reject a resolve naming an unknown comment

The rejection is `Ok(CallToolResult::error(..))`, not `Err(ErrorData)`: rmcp turns an `Err` from a tool into a JSON-RPC error, which clients render opaquely, so the agent would never read which id was refused. The test therefore asserts `result.isError`, and the known-id branch is an `ErrorData::internal_error` saying the recording is not implemented, which task 11 replaces.

The lookup runs through `cfc_core::pending` rather than `Store::fold` so both tools agree on what is open, at the cost of re-anchoring comments an id check does not need; task 11's `Store::is_pending` is meant to take over here. The test pins only the rejection, so an unconditional rejection would pass it too — task 11's test covers the other side.

The `StdioServer` harness task 9 flagged is now `tests/common/mod.rs`, which also owns the handshake (`initialize`) and allocates request ids itself, so callers no longer hardcode them. `tests/handshake.rs` keeps sending `initialize` directly, since the revision it offers is the thing under test, and now spawns against a `TempDir` instead of the repository root.

### Task 11: Record a resolve exactly once however often it is called

The plan mandates `Store::is_pending(id)`, but a boolean cannot carry the three-way distinction the same task demands — never added, added and open, already resolved — so `cfc_core` gained `Store::state_of(id) -> Result<CommentState>` with `Unknown`/`Pending`/`Resolved` instead. It is a per-id restatement of `fold`'s last-event-wins rule sharing `fold`'s private `events()`, so the duplication task 10's critic objected to stays inside `Store` rather than in the server. `resolve_comment` now opens the `Store` directly and no longer goes through `cfc_core::pending`, which drops the re-anchoring an id check never needed; `list_pending_comments` still goes through `pending`.

Only the `Unknown` arm is `CallToolResult::error`. An already-resolved id succeeds without appending, so a retrying agent cannot grow the log, and the tool description now says so. `resolved_at` comes from a private `now()` in `server.rs` mirroring `cfc-cli`'s, because the two return different error types (`ErrorData` against `anyhow::Result`); nothing asserts the timestamp, so the real clock runs unfaked.

`Store::state_of` would let `cfc resolve` say whether a refused id is unknown or already resolved, which task 8 recorded as impossible with `fold` alone. `cfc-cli` was out of scope here, so it still rejects both alike.

### Task 13: Match anchor lines with both ends trimmed

A plan amendment closing the gap task 7 recorded: `trim_end` became `trim` in `locate`, which is the whole implementation change. The accepted cost now lives in the prose above rather than in a code comment, because it is a decision about the matching rule, not something a reader of `locate` needs in order to use it.

The `cfc-mcp` `list_tool` fixture's absent quote (`    let deleted = 3;`) does not match its source under both-ends trimming either, so nothing outside `cfc-core` had to move.

### Task 5: Spike whether an extension MCP server reaches claude-acp

forwarding spike outcome: forwarded. The `settings.json` route worked on the day of the spike and the extension route worked once four defects had been cleared, so both decision 1's channel and decision 6's extension crate are proven.

A `context_servers` entry in the project's `.zed/settings.json` naming the binary by absolute path reached `claude-acp`: the agent listed `mcp__cfc-direct__list_pending_comments` and `mcp__cfc-direct__resolve_comment`, called the first, and got back the pending comment with its quote. That settles the question the plan called unestablished — Zed does forward a configured MCP server to an external agent, and the tool description carried enough for the agent to act on the comment without further prompting.

The extension-contributed server did not get that far on the day. Zed compiled and installed the dev extension (`extension::extension_builder`, 58.21s), found `[context_servers.comment-for-claude]`, and tried to start it, but the `initialize` request went unanswered for 60s and Zed gave up: `comment-for-claude context server failed to start: Context server request timeout`. The same binary answers `initialize` in milliseconds when spawned directly, and nothing at all was logged about the spawn, which is what made the timeout so hard to read.

Four independent defects stood between that timeout and a working extension route, each hidden behind the one before it, and clearing all four is what proves an extension-contributed server *is* forwarded to `claude-acp`:

1. `extension.toml` declared no `[[capabilities]]`, so Zed's host refused the extension's process operations. This is what produced the 60s timeout, and at this point the extension had no error path of its own to report it with. Task 16 granted `process:exec` for `/bin/sh`.
2. The command named the bare program `cfc-mcp`, which the guest cannot resolve because it sees none of the user's `PATH`. Task 15 replaced it with `/bin/sh -lc 'command -v cfc-mcp'`, so the binary has to be on the `PATH` a login shell sees.
3. The spawned server got an empty environment for the same reason. Task 15 takes it from `/bin/sh -lc env`.
4. The `context_servers.comment-for-claude.enabled` setting was absent, and Zed then never attempts the server at all: no error, no timeout, no log line, indistinguishable from the extension not existing. This is the one no diagnostic reveals and the one that cost the most time; `"context_servers": {"comment-for-claude": {"enabled": true, "settings": {}}}` is what fixes it, and the README makes it a numbered step for that reason.

Task 15's named resolution error is what unstacked the pile: once the extension said out loud what it was trying to run, Zed answered with `capability for process:exec ... was not listed in the extension manifest` instead of timing out silently.

Verified end to end afterwards in a `claude-acp` thread over the extension route: the agent called `list_pending_comments`, received a pending comment (`c1`) with its quote, acted on it, and `resolve_comment` closed it. Drift was seen live too, a comment whose quoted line had been reformatted coming back as `status: drifted` with `lines_when_written`, and the agent locating the code from the quote.

The approval half of the spike is unanswered: the thread ran with `mode: bypassPermissions`, under which nothing prompts regardless.

Both routes work, so the extension is a convenience rather than a necessity. Zed's own context-servers page says MCP server extensions are facing deprecation in favour of the official MCP registry, and describes the extension route as intended for servers published as binaries or via NPM, which keeps `settings.json` the better-supported channel of the two even now that the extension is proven.

### Task 14: Relocate extension.toml next to the extension crate

The Zed extension root is `crates/cfc-zed-ext/`, not the repository root: Zed wants `extension.toml`, `Cargo.toml` and `src/lib.rs` in one directory, and `cargo build --release --target wasm32-wasip2` launched at the repository root resolves the workspace `default-members` and dies compiling `tokio` for wasm (`Only features sync,macros,io-util,rt,time are supported on wasm`). Run that build from inside the crate directory. `docs/plans/zed-review-comments-mcp/research.md` still says the manifest sits at the repository root and is now wrong.

### Task 15: Resolve the server binary the extension names

A plan amendment for the 60s `Context server request timeout` task 5 hit. That timeout was a defect in the command the extension described, not in forwarding or in the protocol: the same binary answers `initialize` in milliseconds when spawned directly and works forwarded through `.zed/settings.json`, so only `context_server_command`'s return value was wrong. `CFC_MCP_BINARY` was never functional — `std::env::var` in the WASM guest reads the guest's WASI environment, which is the extension host's, so the variable could not be set from the user's shell at all; it is gone. The bare name `cfc-mcp` was the other half: the guest sees neither the user's `PATH` nor their environment.

Precedence is now three tiers. `ContextServerSettings::for_project(id, project)` with `command.path` set wins on every platform, honouring `arguments` and `env`. Otherwise, on non-Windows, `/bin/sh -lc 'command -v cfc-mcp'` gives the absolute path and `/bin/sh -lc env` gives the environment, both through the `process` host API. Windows cannot self-resolve because that probe is a POSIX shell invocation and no portable equivalent is reachable from the guest, so Windows, a failed probe, and a probe finding nothing all return `Err` naming what was looked for. The `Err` is the point as much as the fix: Zed logs it immediately, whereas an unspawnable command is a silent 60s timeout.

Dead end, do not retry: `Worktree::which` and `Worktree::shell_env` would do this properly but are unreachable from `context_server_command`, which receives a `Project` whose only wit method is `worktree_ids() -> list<u64>`; no API hands a `Worktree` to the guest, and 0.7.0 is the latest crate. A settings read that fails is deliberately not fatal — `for_project` errs on a host or deserialize failure, and it is not established that an absent `context_servers` entry does not produce one, so an `Err` falls through to discovery and is appended to the resolution error only if discovery also fails. `parse_env` keeps the first line of a multi-line environment value and drops its continuation lines; `env` output cannot be parsed unambiguously without `env -0`, and no variable this server needs is multi-line. Nothing here is unit-tested: every input (`current_platform`, `Command::output`, `ContextServerSettings::for_project`) is a wit host import, and linking a host test binary against them would fail, so `cargo build --target wasm32-wasip2` plus driving Zed by hand is the whole verification.

### Task 16: Declare the process:exec capability the extension needs

Zed governs extension operations through capabilities declared in `extension.toml`, enforced by the host and absent from the `zed_extension_api` crate, which is why reading that crate turned up no trace of them. Task 4 deliberately omitted the entry, citing the published `zed-mcp-server-github` extension as spawning without one; that is wrong for Zed 1.14.2, and the omission is what produced task 5's silent 60s timeout. Granted here: one `[[capabilities]]` entry, `kind = "process:exec"`, `command = "/bin/sh"`, `args = ["**"]`, which covers both probes the extension runs (`/bin/sh -lc 'command -v cfc-mcp'` and `/bin/sh -lc env`). The glob is deliberate: Zed's documentation demonstrates only `"**"` for `args` and does not confirm that exact-literal arg matching works. The three documented kinds are `process:exec`, `download_file` and `npm:install`; only the first is needed.

Worth attempting later: narrow this to one entry per literal arg list. Granting `/bin/sh` arbitrary arguments is effectively arbitrary local execution, and a published extension should ask for less than that.

### Task 12: Wire the key binding and document the setup

The plan's `CFC_MCP_BINARY` and repository-root dev extension are both gone (tasks 15 and 14), so the README documents `context_servers.comment-for-claude.command.path` and `crates/cfc-zed-ext/` instead, plus both delivery routes with the extension first.

`.zed/settings.json` existed from the task 5 spike carrying a `cfc-direct` entry with a hardcoded `/home/matteovillosio/.cargo/bin/cfc-mcp`. Only the portable `comment-for-claude` enablement is committed; the machine-specific direct entry moved into the README as route 2, since a committed absolute path would be wrong on every other machine.

`.gitignore`'s `/target` is root-anchored and so covers neither `crates/cfc-zed-ext/target/` nor the `extension.wasm` Zed's extension build drops beside the manifest; both are now listed.

Not verifiable from here: the keymap entry and the `zed: install dev extension` step live outside the repository, so the README is the only artefact for them and the verification command only pins the task definition.

### Rename to marginalia

Everything above this line names the project as it was built, and deliberately keeps those
names: `cfc-*` for the crates, `cfc` for the CLI, `comment-for-claude` for the extension. Log
excerpts and error messages are quoted verbatim from runs that really said `comment-for-claude`,
so rewriting them would falsify the record.

After the plan was executed the project was renamed for publication as
[Tsadoq/marginalia](https://github.com/Tsadoq/marginalia). `cfc-core`, `cfc-mcp`, `cfc-cli` and
`cfc-zed-ext` became `marginalia-core`, `marginalia-mcp`, `marginalia-cli` and
`marginalia-zed-ext`; the CLI command became `marginalia`; the `CFC_*` task environment
variables became `MARGINALIA_*`; and the store moved from `.cfc/` to `.marginalia/`. The old
name tied the project to one agent vendor, which nothing in the implementation is specific to.

Two consequences for anyone who used the earlier name. The extension id changed, so the
agent-visible tools are now `mcp__marginalia__list_pending_comments` and
`mcp__marginalia__resolve_comment`, and the settings key to enable is `marginalia` rather than
`comment-for-claude` — an entry still naming the old id simply stops starting, silently, because
an unenabled context server logs nothing. And a previously installed `cfc-mcp`/`cfc` pair is
orphaned in `~/.cargo/bin` and should be removed.

### Environment passed to the server

Found in live use, after the rename. Zed puts a context server's environment into the external
agent's command line: `pgrep -af` on the running `claude-acp` process printed the whole
`--mcp-config` blob, environment included. The extension was supplying that environment by
running `/bin/sh -lc env` and returning everything it printed, so every secret in the developer's
login shell — API tokens among them — became world-readable through `/proc` to any local process.

The fix is to ask the shell only for what the server could want, rather than capturing everything
and filtering afterwards: filtering would still have carried the secrets through the wasm guest
and Zed's logs before dropping them. The probe is now
`printf '%s\n%s\n' "$PATH" "$HOME"`, and the two values are zipped against a named list.

Even `PATH` and `HOME` are generous. The server reads one file under the working directory Zed
gives it and spawns nothing, so it needs no environment at all; the two are kept only so a future
version that shells out is not surprised. A pinned `command.env` from settings is passed through
untouched, since that is the user naming values deliberately.

The earlier claim that an empty `env` was one of the four causes of the startup failure was wrong.
It was never demonstrated: the capability grant and the unresolvable bare program name account for
the timeout on their own, and a server needing no environment would not have noticed an empty one.
