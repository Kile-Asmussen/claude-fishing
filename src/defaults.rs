use std::path::Path;

pub struct ConfigFile {
    pub rel_path: &'static str,
    pub local_rel_path: &'static str,
    pub default_content: &'static str,
}

pub const CONFIG_FILES: &[ConfigFile] = &[
    ConfigFile {
        rel_path: ".claude/fishing.bash.txt",
        local_rel_path: ".claude/fishing.bash.local.txt",
        default_content: r"
# Each line is a regex that matches against bash commands which Claude wishes to run.
#
# Lines prefixed with ! are negative patterns, other lines are positive patterns.
# Contents of .claude/fishing.bash.local.txt (.gitignore'd by default) is added to this list if it exists.
# If any positive pattern and no negative patterns match, the command is allowed.
#
# To have a pattern match a literal ! as the first character, use a character class: [!]
#
# examples:
echo 'Hello \w+!'
!\s*rm.*
",
    },
    ConfigFile {
        rel_path: ".claude/fishing.paths.txt",
        local_rel_path: ".claude/fishing.paths.local.txt",
        default_content: r"
# Each line is a glob pattern, similar to .gitignore syntax, that matches against file names Claude wishes to read.
#
# Lines prefixed with ! are negative patterns, other lines are positive patterns.
# Contents of .claude/fishing.paths.local.txt (.gitignore'd by default) is added to this list if it exists.
# If any positive pattern and no negative patterns match, the file read is allowed.
#
# Patterns starting with / are absolute paths, allowing Claude to read directories outside
# the project directory for reference, though it is recommended to put those in the local file.
#
# examples:
**
!.env
",
    },
    ConfigFile {
        rel_path: ".claude/fishing.webfetch.txt",
        local_rel_path: ".claude/fishing.webfetch.local.txt",
        default_content: r"
# Each line is a wildcard pattern that matches against URLs Claude wishes to fetch from the web.
#
# Lines prefixed with ! are negative patterns, other lines are positive patterns.
# Contents of .claude/fishing.webfetch.local.txt (.gitignore'd by default) is added to this list if it exists.
# If any positive pattern and no negative patterns match, the file read is allowed.
#
# Full URLs are matched, and the * pattern matches any substring, including those containing / .
#
# examples:
https://code.claude.com/docs*
",
    },
    ConfigFile {
        rel_path: ".claude/fishing.glob-exclude.txt",
        local_rel_path: ".claude/fishing.glob-exclude.local.txt",
        default_content: r"
# Each line is a glob pattern matched against directory names or project-relative paths.
# Matching directories are skipped entirely during Glob and Grep traversal.
#
# Lines prefixed with ! are unhide overrides: they allow a hidden directory (one whose
# name starts with .) through even though hidden directories are excluded by default.
# Contents of .claude/fishing.glob-exclude.local.txt (.gitignore'd by default) is added to this list.
#
# examples:
target
node_modules
",
    },
];

pub const FISHING_README_PATH: &str = ".claude/fishing.txt";

pub const FISHING_README_CONTENT: &str = r"# Fishing — Claude Code hooks suite

This project uses the 'fishing' hooks suite to enforce safety policies on your actions.

## Active hooks

- **SessionStart / rotate-log**: rotates `.claude/fishing.log` to `.claude/fishing.log~` at the start of each session.
- **PreToolUse / pre-tool-use**: enforces allowlists for Bash commands, file reads (Read), web fetches (WebFetch), and file writes (Edit/Write).
- **ConfigChange / config-change**: validates any change to settings.json against the official Claude Code schema before allowing it.
- **CwdChanged**: blocks all working directory changes.

## MCP tools

The `fishing-grep-glob-mcp` MCP server provides two tools:

- **glob**: recursively lists files matching a glob pattern, respecting `.claude/fishing.glob-exclude.txt` and `.claude/fishing.paths.txt`.
- **grep**: searches file contents with a regex, respecting the same exclusion and path rules.

Hidden directories (`.git`, `.claude`, etc.) are skipped by default.

These tools replace the use of `ls`/`find` and `grep`/`rg` bash commands. Load this mcp instead for gaining an overview of this project.

## Configuration files

All config files live in `.claude/` and follow the `fishing.<purpose>.txt` / `fishing.<purpose>.local.txt` naming scheme.
The `.local` variants are merged in at runtime and are excluded from version control (listed in `.gitignore`).

| File | Purpose |
|------|---------|
| `fishing.bash.txt` | Regexes for allowed/denied Bash commands |
| `fishing.paths.txt` | Glob patterns for allowed/denied file reads |
| `fishing.webfetch.txt` | Wildcard patterns for allowed/denied URLs |
| `fishing.glob-exclude.txt` | Directory names/paths excluded from glob and grep traversal |

## Log

`.claude/fishing.log` accumulates hook decisions for the current session. It is rotated at the start of each new session.

## Self-correction

When a hook denies an action, the denial reason explains which config file to update and the required pattern syntax.
When an MCP tool call fails due to a policy violation, the error message similarly names the relevant config file.
";

/// Config files that do not yet exist under `project_dir`.
pub fn missing_files(project_dir: &Path) -> Vec<&'static ConfigFile> {
    CONFIG_FILES
        .iter()
        .filter(|f| !project_dir.join(f.rel_path).exists())
        .collect()
}

const EXTRA_GITIGNORE_ENTRIES: &[&str] = &[".claude/fishing.log", ".claude/fishing.log~"];

/// Local variant filenames and log files that are missing from `.gitignore` content.
pub fn missing_gitignore_entries(gitignore_content: &str) -> Vec<&'static str> {
    let from_configs = CONFIG_FILES.iter().map(|f| f.local_rel_path);
    let extra = EXTRA_GITIGNORE_ENTRIES.iter().copied();
    from_configs
        .chain(extra)
        .filter(|p| !gitignore_content.lines().any(|line| line.trim() == *p))
        .collect()
}
