# To Do

## MCP server sub-crate: `fishing-mcp`

**Background**: As of a recent Claude Code CLI update, the built-in `Grep` and `Glob` tools have been
removed. Anthropic's intent is that Claude uses shell commands (`rg`, `ls`, `find`) instead. This
conflicts with the security model of this project: writing a safe Bash allowlist regex that correctly
constrains `rg` or `find` to permitted paths is far harder than gating a dedicated built-in tool.

**Goal**: Implement a Rust-based MCP server as a sub-crate (`fishing-mcp`) that re-exposes the
equivalent of the removed first-party `Grep` and `Glob` tools. The tools should mirror the original
first-party interfaces as closely as possible so that existing path-allowlist hook logic in
`src/hooks/paths.rs` continues to gate them without changes.

---

### A. Define the MCP tool schemas

Mirror the original first-party tool signatures:

- **`Grep`** — search file contents for a pattern
  - `pattern` (string, required): the regex or literal to search for
  - `path` (string, optional): directory or file to search within; defaults to `.`
  - `include` (string, optional): glob filter applied to filenames (e.g. `*.rs`)
  - Returns: matching lines as `file:line: content` text, same format as the old built-in
- **`Glob`** — list files matching a glob pattern
  - `pattern` (string, required): the glob pattern to expand
  - `path` (string, optional): directory to search within; defaults to `.`
  - Returns: newline-separated list of matching paths, same format as the old built-in

Document each parameter with JSON Schema `description` fields copied from (or consistent with) the
original Anthropic tool documentation so Claude's prompting behaviour is unchanged.

---

### B. Implement `Grep` tool logic

- Use the `regex` crate for pattern matching (same engine Claude Code used internally).
- Walk the target path recursively with `walkdir`, respecting the `include` glob filter via `globset`.
- Skip binary files (check for null bytes in the first 8 KB).
- Output format: `path/to/file:line_number: matched line content` — one match per line.
- Honour the `path` parameter: resolve it relative to the working directory; reject `..` traversal
  that would escape the working directory (return a tool error, not a panic).

---

### C. Implement `Glob` tool logic

- Use `globset` with `literal_separator(true)` so the semantics match the hook's path-check logic.
- Resolve `path` the same way as `Grep` (relative to working directory, no `..` escape).
- Walk with `walkdir`; emit matching paths relative to the working directory, one per line.
- Preserve the original sort order (lexicographic by path).

---

### D. Wire up the MCP server

- Use the `rmcp` (or equivalent minimal MCP) crate to expose the two tools over `stdio` transport.
- The binary entry point should be `fishing-mcp` (set via `[[bin]]` in the sub-crate's `Cargo.toml`).
- On startup, emit the standard MCP `initialize` / `tools/list` handshake.
- Map tool errors (bad regex, path-not-found, permission denied) to MCP `isError: true` responses
  rather than crashing, so Claude Code surfaces them gracefully.

---

### E. Hook integration

- The existing `PreToolUse` hook in `src/hooks/paths.rs` already gates `Read`, `Grep`, and `Glob`
  by tool name. Verify (or extend) `src/hook_input.rs` so the `ToolInput` enum recognises the MCP
  tool names `fishing-mcp/Grep` and `fishing-mcp/Glob` and routes them through the same path-check
  logic.
- No changes to `.claude/paths` config format should be needed.

---

### F. Tests

- Unit-test `Grep` output format against known fixture files.
- Unit-test `Glob` expansion for `**/*`, `*`, and literal patterns with `literal_separator`.
- Integration-test the MCP stdio handshake (spawn the binary, send JSON-RPC, assert responses).
- Confirm that a path outside the working directory returns a tool error rather than content.

---

### G. Build & distribution

- Add `fishing-mcp` as a workspace member in the root `Cargo.toml` once the user sets up the
  sub-crate folder.
- Ensure `cargo build` at the workspace root builds both `fishing` and `fishing-mcp`.
- Update `fishing init` to emit an MCP config stub pointing to `fishing-mcp` so users can wire it
  into Claude Code with minimal manual steps.

---

## Field testing

Run these tests in a real Claude Code session with the hooks wired up (`fishing init --inject <cmd>`).
Each test gives an observable outcome (allow/deny in the log, or a blocked response in the UI).

---

### 2. Bash allowlist enforcement

**Goal**: confirm that allowed commands pass and disallowed ones are blocked before execution.

- [ ] Ask Claude to run an allowed command (e.g. `cargo build`) — expect `ALLOW` in the log.
- [ ] Ask Claude to run a disallowed command (e.g. `echo hello world`) — expect `DENY` in the log and a blocked
  response in the UI.
- [ ] Ask Claude to run a command that only partially matches a pattern (e.g. `cargo build && echo hello world`) —
  expect `DENY` (tests anchored-regex prevention of shell-injection bypass).
- [ ] Add a deny pattern (`!cargo build`) and verify the explicitly allowed command now gets blocked.

---

### 3. Paths allowlist enforcement (Read / Grep / Glob)

**Goal**: confirm path-based tools are gated by `.claude/paths`.

- [ ] Ask Claude to `Read` a file inside the project (covered by `./**/*`) — expect `ALLOW`.
- [ ] Ask Claude to `Read` a file outside the project (e.g. `/etc/passwd`) — expect `DENY`.
- [ ] Ask Claude to `Grep` with no `path` argument (defaults to `.`) — expect `ALLOW` for `.`.
- [ ] Add a deny pattern `!.env` and then ask Claude to `Read .env` — expect `DENY`.
- [ ] Verify `literal_separator` semantics: add `./*` only and ask for `src/main.rs` (no `./` prefix) —
  expect `DENY`; this also validates the path-normalization question from item 1.

---

### 4. WebFetch allowlist enforcement

**Goal**: confirm URL matching via wildcard patterns.

- [ ] Ask Claude to fetch an allowed URL (e.g. `https://docs.rs/regex/latest/regex/`) — expect `ALLOW`.
- [ ] Ask Claude to fetch a disallowed URL — expect `DENY`.
- [ ] Verify scheme boundary: add `https://docs.rs/*` and ask for `http://docs.rs/foo` — expect `DENY`
  (wildcard must not cross the scheme separator).
- [ ] Add a deny pattern `!https://docs.rs/secret/*` and ask for a URL under that prefix — expect `DENY`.

---

### 5. Write / Edit allowlist enforcement

**Goal**: confirm that writes are gated by `settings.json` `permissions.allow` globs.

- [ ] Ask Claude to `Edit` a file covered by an `Edit(...)` glob — expect `ALLOW`.
- [ ] Ask Claude to `Edit` a file not covered by any glob — expect `DENY`.
- [ ] Ask Claude to `Edit` a file inside `.claude/` — expect `DENY` regardless of globs.
- [ ] Ask Claude to `Write` a file outside the project root — expect `DENY`.
- [ ] Confirm tool patterns are not shared: add only `Edit(src/**/*.rs)` and ask Claude to `Write
  src/main.rs` — expect `DENY`.

---

### 6. Settings (ConfigChange) hook

**Goal**: confirm that changes to `settings.json` are validated before they take effect.

- [ ] Make a valid settings change (e.g. add an innocuous `permissions.allow` entry) — expect
  `CONFIG_ALLOW` in the log.
- [ ] Manually break `settings.json` syntax (introduce invalid JSON) and trigger a ConfigChange event —
  expect `CONFIG_BLOCK` and the change rejected.
- [ ] Use `--json-only` mode: introduce a schema-invalid but syntactically valid value — expect
  `CONFIG_ALLOW` (json-only skips schema check).
- [ ] Use default schema mode with a schema-invalid value — expect `CONFIG_BLOCK`.

---

### 7. rotate-log (SessionStart hook)

**Goal**: confirm that starting a new session renames the previous log.

- [x] Accumulate some log content by running several hook-triggering commands.
- [x] Start a new Claude Code session (triggers `SessionStart` → `rotate-log`).
- [x] Verify `.claude/log~` now contains the old content and `.claude/log` is absent or fresh.
- [x] Run `rotate-log` when no log file exists — confirm it is a no-op (no error).

---

### 8. `init` subcommand

**Goal**: confirm that `init` creates config stubs and optionally injects hooks.

- [x] Run `fishing init` in a fresh directory — verify `.claude/bash`, `.claude/paths`, and
  `.claude/webfetch` are created with default contents.
- [x] Verify `.gitignore` is updated to include `bash-local`, `paths-local`, and `webfetch-local`.
- [x] Run `fishing init` again — verify it is idempotent (no files overwritten, no duplicate
  `.gitignore` lines).
- [x] Run `fishing init --inject <cmd>` — verify `settings.json` gains `SessionStart`, `PreToolUse`,
  and `ConfigChange` hook entries pointing to `<cmd>`.
- [x] Run `fishing init --inject <cmd>` a second time — verify hooks are appended again which is expected behavior.

---

### 9. Unknown / unrecognized tool pass-through

**Goal**: confirm that tools not listed in `ToolInput` variants are allowed by default.

- [x] Trigger a hook event for a tool name that has no variant (e.g. a hypothetical future tool) —
  expect `ALLOW` with the "unrecognized tool" reason in the log.

---

### 10. Log output format

**Goal**: confirm that log entries are human-readable and contain enough context for post-hoc auditing.

- [x] After running a mix of allowed and denied calls, open `.claude/log` and verify each entry has:
  - the raw `stdin:` JSON line,
  - a `[ALLOW]` / `[DENY]` decision line with reason,
  - the final `verdict:` JSON line.
- [x] Verify the log rotates correctly across sessions (see item 7).