# Fishing hooks

A hooks suite for Claude, written in rust.

## User Profile

- Name: Kashmira Qeel
- Pronouns: she/her
- Background: CS master's, 5 years professional software development, fluent in Lua/Python/Rust/Bash

## Permissions

### What I can do without asking

- **Read**, **Grep**, and **Glob** anything in this project directory.
- **Edit and write/create** any and all files under the following folders:
  - `slop/`: notes and progress tracking, temporary files
  - `src/`: Rust code
- **Edit** certain other select files:
  - `Cargo.toml`
  - `TODO.md`: the to-do list
  - `CLAUDE.md`: this file
- **Run** the specific Bash commands listed in `.claude/bash-commands.json` (see below)
- **WebFetch** from `docs.rs`

### What I must ask the user to do

- **Modify hook or settings configuration** — `.claude/` is fully off-limits for edits and writes.
- **Add new allowed Bash commands** — user must update `.claude/bash-commands.json`.
- **Read files outside the project** that aren't in the allowlist (e.g. new system paths).
- **Fetch from new web domains** — user must update `.claude/webfetch-urls.json`.

### Allowed Bash commands

cargo build
cargo test

## Safety Harness Notes

- `defaultMode: dontAsk` means any tool not explicitly allowed is auto-denied without prompting.
- Hooks are a defense-in-depth layer; they fire even if permissions are misconfigured.
- When a hook denies a tool call, the error message includes the full list of allowed patterns. Use that list to self-correct — no need to read config files.
- No edit access to `.claude/`, ask user for help if Claude's configration files or any of the hook scripts cause problems.
