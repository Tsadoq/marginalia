## Research dossiers (appendix)

### Coverage

| # | Decision | Dossier | Not researched because |
|---|----------|---------|------------------------|
| 1 | Delivery channel | [Delivery channel](#delivery-channel) | |
| 2 | Comment capture | [Comment capture](#comment-capture) | |
| 3 | Comment store | | Answered by probe 4 on this machine rather than by documentation |
| 4 | Anchor durability | | A self-contained algorithm with no external dependency to validate |
| 5 | Implementation language | [Implementation language](#implementation-language) | |
| 6 | Extension crate language | | Forced by the platform, so there was no option set to research |

### Delivery channel

**The question**: does a Zed extension that declares an MCP context server actually reach an external ACP agent such as `claude-acp`, and is the manifest and trait shape what the plan assumes?

**The answer**: the manifest shape and the trait signature are confirmed. The load-bearing forwarding claim is not: Zed's docs say only that "Zed-configured MCP servers may be forwarded to External Agents over ACP" and never distinguish an extension-contributed server from a settings.json one. Task 5 exists to settle it by experiment.

**What we found**:
- A real published extension gives the minimal manifest shape, a `[context_servers.<id>]` table whose only field is `name`, with no `capabilities` entry at all, so a capability stanza is not a required companion of a context server declaration.
- `zed_extension_api` 0.7.0 declares `fn context_server_command(&mut self, _context_server_id: &ContextServerId, _project: &Project) -> Result<Command>`, returning `Result<Command>` rather than `Result<Option<Command>>`, with a companion `context_server_configuration()` returning `Result<Option<ContextServerConfiguration>>` for user-supplied settings.
- Zed's MCP page states it supports MCP Tools and Prompts, but whether prompts cross the ACP boundary to an external agent is unconfirmed, so the plan uses tools only.
- `agent.tool_permissions.default` accepts `confirm` (the default), `allow`, and `deny`, with per-tool overrides keyed `mcp:<server>:<tool_name>`. The user's settings already carry `allow`, so per-call approval should not appear.

**Sources**:
- https://zed.dev/docs/extensions/mcp-extensions
- https://zed.dev/docs/ai/mcp
- https://zed.dev/docs/ai/external-agents
- https://docs.rs/zed_extension_api/0.7.0/zed_extension_api/trait.Extension.html
- https://github.com/LoamStudios/zed-mcp-server-github/blob/main/extension.toml

### Comment capture

**The question**: can a Zed task read interactive input, and do the task variables carry enough information to recover a multi-line selection range?

**The answer**: the range can be recovered only by derivation, and interactivity is undocumented in either direction. Task 1 settles interactivity by experiment before task 8 depends on it.

**What we found**:
- Zed ships no selection start and end row variables. Issue 13478, open since June 2024, requests exactly `ZED_START_ROW` and `ZED_END_ROW`. `$ZED_ROW` is documented only as the current line row, so the implementation must treat it as one endpoint and span the selection's line count.
- The task docs warn that variables embedded in a raw `command` string need manual escaping and steer callers to `args`. `env` is a separate substitution path described as appended to the terminal's environment, which is why the plan passes the multi-line selection through `env`.
- Global tasks live at `~/.config/zed/tasks.json` and project tasks at `.zed/tasks.json`. The documented precedence order covers runnable tags, not `task_name` collisions, so the plan defines its task only in the project file.
- The documented binding form is `"alt-g": ["task::Spawn", { "task_name": "..." }]`. The docs example uses `"context": "Workspace"`; the plan uses `"Editor"` so the binding fires where the selection lives. No `has_selection` context predicate is documented, so the CLI must tolerate an empty selection.
- The full task schema includes `label`, `command`, `args`, `env`, `cwd`, `use_new_terminal`, `allow_concurrent_runs`, `reveal`, `reveal_target`, `hide`, `shell`, `show_summary`, `show_command`, `save`, `tags`, `hooks`, and `reevaluate_context`. `reveal` must stay `always`, because a pane without focus cannot receive keystrokes.

**Sources**:
- https://zed.dev/docs/tasks
- https://raw.githubusercontent.com/zed-industries/zed/main/docs/src/tasks.md
- https://raw.githubusercontent.com/zed-industries/zed/main/docs/src/key-bindings.md
- https://github.com/zed-industries/zed/issues/13478

### Implementation language

**The question**: is Rust a workable single language for the extension, the MCP server, and the CLI, and does a WASM crate coexist with native crates in one workspace?

**The answer**: yes, with one known pain point that has a documented workaround the plan adopts in task 2.

**What we found**:
- `rmcp` is the official Rust MCP SDK under the `modelcontextprotocol` organisation, at 3.1.2, with roughly 4.1 million downloads a month and 2267 dependent crates. A second crate, `rust-mcp-sdk`, exists but is far less adopted.
- The current MCP specification revision is `2026-07-28`, which `rmcp` 3.1.2 implements.
- Zed extensions require `crate-type = ["cdylib"]`, target `wasm32-wasip2`, and call `zed::register_extension!`. Zed compiles the Rust to WASM itself during `zed: install dev extension`, so no prebuilt artefact is committed. `extension.toml` sits at the repository root.
- Mixing a `wasm32-wasip2` crate with native crates in one workspace can make a workspace-wide build pick the host linker, which cannot resolve WASI imports. The workaround is to keep the WASM crate out of the default members and build it explicitly with `--target`. The MCP Rust SDK's own repository handles its WASI example this way.
- The docs sample still pins `zed_extension_api = "0.1.0"`, which is stale. Probe 3 establishes 0.7.0 as current.

**Sources**:
- https://lib.rs/crates/rmcp
- https://crates.io/crates/rmcp
- https://github.com/modelcontextprotocol/rust-sdk
- https://github.com/modelcontextprotocol/rust-sdk/pull/541
- https://modelcontextprotocol.io/specification
- https://zed.dev/docs/extensions/developing-extensions
