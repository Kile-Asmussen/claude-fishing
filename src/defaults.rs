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
        default_content: "# examples:\necho 'Hello \\w+!'\n!\\s+rm.*",
    },
    ConfigFile {
        rel_path: ".claude/paths",
        local_rel_path: ".claude/paths-local",
        default_content: "# default:\n./**/*\n**/*\n./*\n*\n!.env",
    },
    ConfigFile {
        rel_path: ".claude/webfetch",
        local_rel_path: ".claude/webfetch-local",
        default_content: "# for claude:\nhttps://code.claude.com/docs/*",
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
