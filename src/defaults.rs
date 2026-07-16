use std::path::Path;

pub struct ConfigFile {
    pub rel_path: &'static str,
    pub local_rel_path: &'static str,
    pub default_content: &'static str,
}

pub const CONFIG_FILES: &[ConfigFile] = &[
    ConfigFile {
        rel_path: ".claude/bash",
        local_rel_path: ".claude/bash-local",
        default_content: r"
# Each line is a regex that matches against bash commands which Claude wishes to run.
#
# Lines prefixed with ! are negative patterns, other lines are positive patterns.
# Contents of .claude/bash-local (.gitignore'd by default) is added to this list if it exists.
# If any positive pattern and no negative patterns match, the command is allowed.
#
# To have a pattern match a literal ! as the first character, use a character class: [!]
#
# examples:
echo 'Hello \\w+!'
!\s+rm.*
",
    },
    ConfigFile {
        rel_path: ".claude/paths",
        local_rel_path: ".claude/paths-local",
        default_content: r"
# Each line is a glob pattern, similar to .gitignore syntax, that matches against file names Claude wishes to read.
#
# Lines prefixed with ! are negative patterns, other lines are positive patterns.
# Contents of .claude/paths-local (.gitignore'd by default) is added to this list if it exists.
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
        rel_path: ".claude/webfetch",
        local_rel_path: ".claude/webfetch-local",
        default_content: r"
# Each line is a wildcard pattern that matches against URLs Claude wishes to fetch from the web.
#
# Lines prefixed with ! are negative patterns, other lines are positive patterns.
# Contents of .claude/webfetch-local (.gitignore'd by default) is added to this list if it exists.
# If any positive pattern and no negative patterns match, the file read is allowed.
#
# Full URLs are matched, and the * pattern matches any substring, including those containing / .
#
# examples:
https://code.claude.com/docs*
",
    },
    ConfigFile {
        rel_path: ".claude/glob-exclude",
        local_rel_path: ".claude/glob-exclude-local",
        default_content: r"
# Each line is a glob pattern matched against directory names or project-relative paths.
# Matching directories are skipped entirely during Glob and Grep traversal.
#
# Lines prefixed with ! are unhide overrides: they allow a hidden directory (one whose
# name starts with .) through even though hidden directories are excluded by default.
# Contents of .claude/glob-exclude-local (.gitignore'd by default) is added to this list.
#
# examples:
target
node_modules
",
    },
];

/// Config files that do not yet exist under `project_dir`.
pub fn missing_files(project_dir: &Path) -> Vec<&'static ConfigFile> {
    CONFIG_FILES
        .iter()
        .filter(|f| !project_dir.join(f.rel_path).exists())
        .collect()
}

/// Local variant filenames that are missing from `.gitignore` content.
pub fn missing_gitignore_entries(gitignore_content: &str) -> Vec<&'static str> {
    CONFIG_FILES
        .iter()
        .map(|f| f.local_rel_path)
        .filter(|p| !gitignore_content.lines().any(|line| line.trim() == *p))
        .collect()
}
