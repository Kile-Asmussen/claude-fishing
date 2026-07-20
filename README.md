# Claude's Fishing Hooks

This is a hooks suite for [Claude Code](https://claude.ai/code) that lets Claude operate with greater autonomy while keeping sensitive files safe and preventing user decision fatigue.

Any local data that Claude touches is sent to Anthropic's servers. Many realworld code project folders contain files with credentials and other secrets in plain text (sometimes committed to the repository! If that's you, rotate your keys!) Fishing enforces an allowlist on every file read, bash command, and web fetch at the hook layer, before the data is loaded into Claude Code.

It also revives the `Glob` and `Grep` tools that were recently removed from the Claude Code harness, so Claude can search the codebase without needing broad bash access, and preventing grepping operations from reading sensitive files at the same time.

## What is it?

Two binaries:

- `fishing` — the CLI tool containing the hooks and initializer.
- `fishing-grep-glob-mcp` — the MCP server.

## Installation

Binaries:

```sh
cargo install --path .
```

Project configuration:

```sh
# inside your project directory
fishing init
```

This command creates config files under `.claude/`, updates `.gitignore` in relevant ways, injects the hooks into `.claude/settings.json`, and registers the MCP server in `.mcp.json`. The operation is idempotent.

If `fishing` or `fishing-grep-glob-mcp` are not on your `$PATH` at their default names, pass the invocation paths explicitly:

```sh
fishing init --inject ~/somewhere/alt-fishing --mcp ~/somewhere/else/alt-grep-glob
```

## Configuration

All config lives under `.claude/`. Every file has a `.local.txt` variant (`.gitignore`'d by default) that is merged in at runtime, so you can layer project-private additions without committing them.

This is especially useful since the `fishing.paths.*` configuration files can allow Claude to read files outside the project directory by using absolute paths, such as referencing data elsewhere on the local system without having the user having to move files or symlink directories to the current project directory.

Each configuration file takes the form of line-by-line patterns that can either allow or deny an action, to allow an action it must match at least one 'allow' pattern and no 'deny' patterns.

### File read allowlist: `fishing.paths.txt`

Per-line glob patterns controlling which files Claude may read. Lines starting with `!` are deny rules that take priority, overall very similar to `.gitignore`.

```
# allow everything in the project
**
# but never read secrets
!.env
!.env.*
!**/*.pem
!**/*.key
```

Patterns starting with `/` are absolute and can reach outside the project directory. Patterns without `/` or starting with `./` are relative to the project root.

### Bash command allowlist: `fishing.bash.txt`

Regular expressions matched against the full command string. Same format as the `fishing.paths.*` files with `!`-prefixed lines being 'deny' patterns.

```
# allow common dev commands
cargo (build|test|check|clippy|fmt).*
npm (install|run|test).*
# deny command chaining
!.*&&.*
!.*;.*
!.*\|.*
# deny destructive commands even if something above would match
!.*rm -rf.*
!.*git push --force.*
```

The regex is anchored at both ends (`\A...\z`), so `cargo build` will not match `cargo build && rm -rf /`. Take care when using trailing wildcards as they **will** match `&& | ;` and other ways to tack on extra unauthorized commands.

### Web access allowlist: `fishing.webfetch.txt`

DNS-server and firewall style wildcard patterns matched against the full URL. `*` matches any substring including `/`.

```
https://docs.rs*
!https://docs.rs/crate/prompt_injection_trap*
```

### Skipped directories: `fishing.glob-exclude.txt`

Minor configuration file of project-relative paths to skip during MCP traversal, to prevent enormous printouts (a quick `ls ./target/**` of this project after a clean+build prints a cool 2000+ lines!)

Hidden directories (`.git/` etc.) are always skipped unless explicitly un-hidden with a `!` prefix. Hidden files are always shown (`.gitignore` etc.)

```
target
node_modules
# un-hide a specific hidden directory
!.githooks
```

### Write permissions: derived from `settings.json`

Edit and Write operations are controlled by Claude Code's built-in `permissions.allow` entries in `.claude/settings.json`, using the `Edit(...)` / `Write(...)` syntax. Fishing enforces two additional hard rules on top of that:

- Writes/Edits must stay within the project directory.
- Writes/Edits to `.claude/` are always blocked as a security measure.

This is more of a defense in depth measure.

## Hooks

The following types of hooks are injected upon running `init`:

- `PreToolUse` via `fishing pre-tool-use` : enforces the allow lists for tool invocations

- `SessionStart` via `fishing rotate-log` : does exactly what you think (moves one (1) file)

- `CwdChanged` via `fishing cwd-changed` : blocks working directory changes (just to be safe)

- `ConfigChange` --- Validates the JSON of Claude Code's `settings.json` and `settings.local.json` files to prevent a change in the files breaking the integrity of the hooks.

**NOTICE:** As of version 2.1.196, the `ConfigChange` hooks cannot actually block a configuration change. [Bug report here.](https://github.com/anthropics/claude-code/issues/79547)

## MCP tools

The `fishing-grep-glob-mcp` server provides two tools that replace `ls`/`find` and `grep`/`rg` bash commands:

- `mcp__grep-glob__glob` : lists files matching a glob pattern under a directory
- `mcp__grep-glob__grep` : searches file contents with a regex and optionally includes matched lines

Both tools respect the `fishing.glob-exclude.txt` exclusion list and the `fishing.paths.txt` read allowlist, so they cannot reveal the contents of files Claude isn't permitted to read directly.

## Logging

`.claude/fishing.log` records every hook decision for the current session (allow, deny, and the matching pattern). It is rotated to `.claude/fishing.log~` at the start of each new session.

## Self-correction through error messages

When a hook denies an action, Claude will receive a detailed error message which lists the allowed patterns and names the relevant config file containing the rules. This either lets Claude immediately self-correct, or stop and ask the user to be granted new access rights.
