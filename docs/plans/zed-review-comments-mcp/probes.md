## Verification probes (appendix)

[probe 1]: `rustup target list | grep wasm`
Why: the extension crate cannot build unless the stable toolchain offers `wasm32-wasip2`.
Observed:
```
wasm32-unknown-emscripten
wasm32-unknown-unknown
wasm32-wasip1
wasm32-wasip1-threads
wasm32-wasip2
wasm32v1-none
```
If it had failed: decision 6 would have been unbuildable on this machine and the plan would have needed a toolchain upgrade task before anything else.

[probe 2]: `curl -s https://crates.io/api/v1/crates/rmcp | python3 -c "..."`
Why: check that the chosen MCP crate builds on the installed rustc 1.88.0.
Observed:
```
rmcp max_version: 3.1.2 | max_stable: 3.1.2
newest: 3.1.2 rust_version(MSRV): 1.88 edition? 2024
```
If it had failed: an MSRV above 1.88 would have forced either a rustup upgrade task or decision 5 falling back to TypeScript.

[probe 3]: `curl -s https://crates.io/api/v1/crates/zed_extension_api | python3 -c "..."`
Why: confirm the extension API version to pin, since the Zed docs sample still shows a stale `0.1.0`.
Observed:
```
max_version: 0.7.0
  0.7.0 msrv None yanked False 2025-09-12
  0.6.0 msrv None yanked False 2025-06-18
  0.5.0 msrv None yanked False 2025-05-02
  0.4.0 msrv None yanked False 2025-04-22
```
If it had failed: a yanked or absent 0.7.0 would have changed the trait signature task 4 implements.

[probe 4]: `python3 append_race.py` (8 processes, 500 appends each, 200-byte lines, `O_APPEND`)
Why: decision 3 claims `O_APPEND` alone makes the concurrent writer and reader safe without a lock file.
Observed:
```
lines: 4000 expected: 4000 malformed: 0
```
If it had failed: interleaved or lost lines would have forced a lock file or pushed decision 3 to SQLite.

[probe 5]: `grep -rn "agent_servers\|debug_adapters" --include=extension.toml ~/.local/share/zed/extensions/installed`
Why: check whether the deprecated ACP provider hook is still present in manifests, since a live hook would have reopened the push-injection option.
Observed:
```
catppuccin-icons/extension.toml:21:[agent_servers]
make/extension.toml:23:[agent_servers]
catppuccin/extension.toml:21:[agent_servers]
dockerfile/extension.toml:39:[debug_adapters.buildx-dockerfile]
html/extension.toml:31:[agent_servers]
log/extension.toml:23:[agent_servers]
toml/extension.toml:23:[agent_servers]
```
If it had failed: an absent table would have confirmed removal outright; the tables are present but empty, and the Zed docs still call ACP extensions deprecated since v1.5.0, so the option stays rejected.
