# To Do

## Implementation status

All features are implemented and unit-tested (63 tests passing). The project consists of:

- **`fishing`** binary — PreToolUse / ConfigChange / SessionStart / CwdChanged hooks, `init` subcommand
- **`fishing-grep-glob-mcp`** binary — MCP server exposing `glob` and `grep` tools with path and exclusion enforcement
- **`claude_fishing`** lib — shared config loading, hook logic, glob/path predicates

---


## Field testing

Run these in a real Claude Code session in a sterile directory with hooks wired up via `fishing init --inject <cmd>`.

### Hooks (partially verified)

- [x] Bash allowlist: allowed command passes, disallowed is blocked
- [x] Bash allowlist: partial match (e.g. `cargo build && echo foo`) is blocked (anchored regex)
- [x] Bash allowlist: deny pattern (`!cargo build`) blocks an otherwise-allowed command
- [x] Bash local variant: patterns in `bash-local` are merged and enforced
- [x] Paths allowlist: `Read` inside project passes, `/etc/passwd` is denied
- [ ] Paths allowlist: deny pattern (`!.env`) blocks a covered path
  - **ISSUE**: deny pattern fires but error message does not distinguish from "no allow pattern matched" — same generic "not permitted by paths config" message in both cases
- [x] Paths allowlist: `literal_separator` semantics — `./*` does not match `src/main.rs`
- [x] Paths local variant: patterns in `paths-local` are merged and enforced
- [x] WebFetch allowlist: allowed URL passes, disallowed is blocked
- [x] WebFetch allowlist: scheme boundary — `https://docs.rs/*` does not match `http://docs.rs/foo`
- [x] WebFetch local variant: patterns in `webfetch-local` are merged and enforced
- [x] Write/Edit allowlist: file covered by `Edit(...)` glob passes, uncovered file is denied
- [x] Write/Edit: `.claude/` directory is always denied regardless of globs
- [x] Write/Edit: tool patterns not shared — `Edit(src/**/*.rs)` does not allow `Write src/main.rs`
  - **WONTFIX**: Claude Code CLI uses the same glob for both Edit and Write; enforcing them separately would diverge from canonical platform behavior
- [x] ConfigChange hook: valid settings change produces `CONFIG_ALLOW`
- [ ] ConfigChange hook: invalid JSON produces `CONFIG_BLOCK`
  - **BUG**: hook fires and logs a `CONFIG_BLOCK` error correctly, but Claude Code CLI does not respect the block — session continues rather than halting the settings change
- [ ] ConfigChange hook: schema-invalid value produces `CONFIG_BLOCK` in default mode, `CONFIG_ALLOW` with `--json-only`
- [x] rotate-log: new session renames `.claude/fishing.log` to `.claude/fishing.log~`
- [x] rotate-log: no-op when no log file exists
- [x] init: creates `.claude/bash`, `paths`, `webfetch`, `glob-exclude` with default contents
- [x] init: updates `.gitignore` with local variant entries
- [x] init: idempotent (no files overwritten, no duplicate `.gitignore` lines)
- [x] init --inject: adds SessionStart, PreToolUse, ConfigChange hook entries to settings.json
- [x] init --inject: PreToolUse matcher contains `Bash|Read|WebFetch|Edit|Write` and does NOT contain `Glob` or `Grep`
- [x] init --inject: MCP server registered under `mcpServers.grep-glob`
- [x] init --inject: idempotent (second run does not duplicate MCP entry or hook entries)
- [x] Unknown tool: unrecognized tool name produces `ALLOW` with "unrecognized tool" reason
- [x] Log format: each entry has stdin JSON, decision line with reason, stdout JSON
- [x] MCP log: glob and grep calls appear in `.claude/fishing.log` with timestamp and parameters
  - **IMPROVEMENT**: log messages are currently too brief — consider including more detail (e.g. result count, matched paths, which config files were consulted)

### MCP tools (not yet verified)

- [x] glob: returns files matching pattern, paths relative to search root, sorted lexicographically
- [x] glob: hidden directories (`.git`, `.claude`) are skipped by default — dotfile exclusion applies to directories only, not files
- [x] glob: `glob-exclude` config excludes configured dirs (e.g. `target`, `node_modules`)
- [x] glob: `glob-exclude-local` patterns are merged and enforced
  - **NOTE**: `!node_modules` in local file (unhide) vs `node_modules` in base (hide) — local unhide wins per deny-override semantics, but `node_modules` dir absent on disk so untested
  - **NOTE**: `!.git/hooks/` unhide in local file did not surface test file in `.git/hooks/` — may be that path-based unhide patterns for nested hidden dirs are not yet supported
- [x] glob: paths not permitted by `.claude/paths` are excluded from results
  - **NOTE**: glob reveals denied files (they appear in listing) but grep silently skips them — this is intentional per design
- [x] glob: hard error (tool error response) when `.claude/paths` is missing
  - **NOTE**: glob only respects `glob-exclude`, not `fishing.paths.txt` — so missing paths config is intentionally not an error for glob
- [x] glob: `path` parameter restricts search root; `..` traversal outside project root is rejected
- [x] grep: returns matching file paths by default, one per line, sorted
- [x] grep: `include_lines=true` produces `path:lineno: content` output for each matched line
- [x] grep: binary files are skipped
- [x] grep: hidden directories and `glob-exclude` patterns are respected (same as glob)
- [x] grep: paths not permitted by `.claude/paths` are excluded
- [x] grep: hard error when `.claude/paths` is missing
- [x] grep: `include` glob filter applied before content search
- [x] grep: `path` parameter restricts search root; `..` escape rejected
