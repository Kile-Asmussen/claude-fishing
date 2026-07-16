# To Do

## Implementation status

All features are implemented and unit-tested (63 tests passing). The project consists of:

- **`fishing`** binary — PreToolUse / ConfigChange / SessionStart / CwdChanged hooks, `init` subcommand
- **`grep-glob-mcp`** binary — MCP server exposing `glob` and `grep` tools with path and exclusion enforcement
- **`claude_fishing`** lib — shared config loading, hook logic, glob/path predicates

---

## Concerns:

Quality of error messages in the MCP needs to match the hooks: the object is to give Claude relevant information for how to immediately self-correct.


## Field testing

Run these in a real Claude Code session in a sterile directory with hooks wired up via `fishing init --inject <cmd>`.

### Hooks (partially verified)

- [x] Bash allowlist: allowed command passes, disallowed is blocked
- [x] Bash allowlist: partial match (e.g. `cargo build && echo foo`) is blocked (anchored regex)
- [ ] Bash allowlist: deny pattern (`!cargo build`) blocks an otherwise-allowed command
- [ ] Bash local variant: patterns in `bash-local` are merged and enforced
- [x] Paths allowlist: `Read` inside project passes, `/etc/passwd` is denied
- [ ] Paths allowlist: deny pattern (`!.env`) blocks a covered path
- [ ] Paths allowlist: `literal_separator` semantics — `./*` does not match `src/main.rs`
- [ ] Paths local variant: patterns in `paths-local` are merged and enforced
- [x] WebFetch allowlist: allowed URL passes, disallowed is blocked
- [ ] WebFetch allowlist: scheme boundary — `https://docs.rs/*` does not match `http://docs.rs/foo`
- [ ] WebFetch local variant: patterns in `webfetch-local` are merged and enforced
- [x] Write/Edit allowlist: file covered by `Edit(...)` glob passes, uncovered file is denied
- [ ] Write/Edit: `.claude/` directory is always denied regardless of globs
- [ ] Write/Edit: tool patterns not shared — `Edit(src/**/*.rs)` does not allow `Write src/main.rs`
- [x] ConfigChange hook: valid settings change produces `CONFIG_ALLOW`
- [ ] ConfigChange hook: invalid JSON produces `CONFIG_BLOCK`
- [ ] ConfigChange hook: schema-invalid value produces `CONFIG_BLOCK` in default mode, `CONFIG_ALLOW` with `--json-only`
- [x] rotate-log: new session renames `.claude/log` to `.claude/log~`
- [x] rotate-log: no-op when no log file exists
- [x] init: creates `.claude/bash`, `paths`, `webfetch`, `glob-exclude` with default contents
- [x] init: updates `.gitignore` with local variant entries
- [x] init: idempotent (no files overwritten, no duplicate `.gitignore` lines)
- [x] init --inject: adds SessionStart, PreToolUse, ConfigChange hook entries to settings.json
- [ ] init --inject: PreToolUse matcher contains `Bash|Read|WebFetch|Edit|Write` and does NOT contain `Glob` or `Grep`
- [ ] init --inject: MCP server registered under `mcpServers.grep-glob`
- [ ] init --inject: idempotent (second run does not duplicate MCP entry or hook entries)
- [x] Unknown tool: unrecognized tool name produces `ALLOW` with "unrecognized tool" reason
- [x] Log format: each entry has stdin JSON, decision line with reason, stdout JSON

### MCP tools (not yet verified)

- [ ] glob: returns files matching pattern, paths relative to search root, sorted lexicographically
- [ ] glob: hidden directories (`.git`, `.claude`) are skipped by default
- [ ] glob: `glob-exclude` config excludes configured dirs (e.g. `target`, `node_modules`)
- [ ] glob: `glob-exclude-local` patterns are merged and enforced
- [ ] glob: paths not permitted by `.claude/paths` are excluded from results
- [ ] glob: hard error (tool error response) when `.claude/paths` is missing
- [ ] glob: `path` parameter restricts search root; `..` traversal outside project root is rejected
- [ ] grep: returns matching file paths by default, one per line, sorted
- [ ] grep: `include_lines=true` produces `path:lineno: content` output for each matched line
- [ ] grep: binary files are skipped
- [ ] grep: hidden directories and `glob-exclude` patterns are respected (same as glob)
- [ ] grep: paths not permitted by `.claude/paths` are excluded
- [ ] grep: hard error when `.claude/paths` is missing
- [ ] grep: `include` glob filter applied before content search
- [ ] grep: `path` parameter restricts search root; `..` escape rejected
