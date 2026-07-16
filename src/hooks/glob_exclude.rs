use globset::GlobBuilder;
use std::{fs, path::Path};

use super::config;

pub fn load_config(claude_dir: &Path) -> Result<String, String> {
    let path = claude_dir.join("glob-exclude");
    let mut out = fs::read_to_string(&path).map_err(|e| format!("could not read {path:?}: {e}"))?;
    if let Ok(local) = fs::read_to_string(claude_dir.join("glob-exclude-local")) {
        if !local.is_empty() {
            out.push('\n');
            out.push_str(&local);
        }
    }
    Ok(out)
}

/// Returns `true` if `path` should be excluded from glob/grep traversal.
///
/// Config file syntax (`.claude/glob-exclude`):
/// - Plain lines are exclusion patterns (exclude matching paths beyond hidden dirs).
/// - `!`-prefixed lines are unhide overrides (allow a hidden path through).
///
/// After `config::partition`, plain lines land in `patterns.allow` and
/// `!`-prefixed lines land in `patterns.deny` — so in this module the bucket
/// names are intentionally read in reverse of their names.
///
/// A path is excluded when:
/// - Its file name starts with `.` (hidden), unless a `!`-prefixed pattern matches it, OR
/// - A plain pattern in `glob_exclude_config` matches its name or project-relative path.
pub fn is_excluded(glob_exclude_config: &str, project_dir: &Path, path: &Path) -> bool {
    let patterns = config::partition(glob_exclude_config);
    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
    let rel = path.strip_prefix(project_dir).unwrap_or(path);

    // Hidden by default, but !-patterns (in patterns.deny) can override.
    if name.starts_with('.') {
        let unhidden = patterns.deny.iter().any(|p| matches_either(p, name, rel));
        return !unhidden;
    }

    // Plain patterns (in patterns.allow) add extra exclusions for non-hidden paths.
    patterns.allow.iter().any(|p| matches_either(p, name, rel))
}

fn matches_either(pattern: &str, name: &str, rel: &Path) -> bool {
    let name_match = GlobBuilder::new(pattern)
        .literal_separator(false)
        .build()
        .map(|g| g.compile_matcher().is_match(name))
        .unwrap_or(false);

    let rel_match = GlobBuilder::new(pattern)
        .literal_separator(true)
        .build()
        .map(|g| g.compile_matcher().is_match(rel))
        .unwrap_or(false);

    name_match || rel_match
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    fn excluded(config: &str, path: &str) -> bool {
        super::is_excluded(config, Path::new("/project"), Path::new(path))
    }

    #[test]
    fn hidden_dir_excluded_by_default() {
        assert!(excluded("", "/project/.git"));
        assert!(excluded("", "/project/.direnv"));
    }

    #[test]
    fn normal_dir_not_excluded() {
        assert!(!excluded("", "/project/src"));
        assert!(!excluded("", "/project/target"));
    }

    #[test]
    fn deny_pattern_excludes_normal_dir() {
        assert!(excluded("target\n", "/project/target"));
        assert!(!excluded("target\n", "/project/src"));
    }

    #[test]
    fn allow_pattern_unhides_hidden_dir() {
        assert!(!excluded("!.git\n", "/project/.git"));
        assert!(excluded("!.git\n", "/project/.direnv"));
    }

    #[test]
    fn deny_pattern_excludes_by_relative_path() {
        assert!(excluded("vendor/assets\n", "/project/vendor/assets"));
        assert!(!excluded("vendor/assets\n", "/project/vendor/code"));
    }

    #[test]
    fn comments_are_ignored() {
        // A line starting with # should not be treated as a pattern.
        assert!(!excluded("# target\n", "/project/target"));
        assert!(excluded("", "/project/.hidden"));
    }

    #[test]
    fn load_config_merges_local_file() {
        use tempfile::TempDir;
        let dir = TempDir::new().unwrap();
        let claude = dir.path().join(".claude");
        std::fs::create_dir_all(&claude).unwrap();
        std::fs::write(claude.join("glob-exclude"), "target\n").unwrap();
        std::fs::write(claude.join("glob-exclude-local"), "node_modules\n").unwrap();
        let combined = super::load_config(&claude).unwrap();
        assert!(combined.contains("target"));
        assert!(combined.contains("node_modules"));
    }

    #[test]
    fn load_config_missing_local_is_ok() {
        use tempfile::TempDir;
        let dir = TempDir::new().unwrap();
        let claude = dir.path().join(".claude");
        std::fs::create_dir_all(&claude).unwrap();
        std::fs::write(claude.join("glob-exclude"), "target\n").unwrap();
        assert!(super::load_config(&claude).is_ok());
    }

    #[test]
    fn load_config_missing_base_is_error() {
        use tempfile::TempDir;
        let dir = TempDir::new().unwrap();
        let claude = dir.path().join(".claude");
        std::fs::create_dir_all(&claude).unwrap();
        assert!(super::load_config(&claude).is_err());
    }
}
