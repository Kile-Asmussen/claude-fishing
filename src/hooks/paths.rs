use globset::GlobBuilder;
use std::{fs, path::Path};

use super::config;
use super::env::HookEnv;

pub fn load_config(claude_dir: &Path) -> Result<String, String> {
    let path = claude_dir.join("paths");
    let mut out = fs::read_to_string(&path).map_err(|e| format!("could not read {path:?}: {e}"))?;
    if let Ok(local) = fs::read_to_string(claude_dir.join("paths-local")) {
        if !local.is_empty() {
            out.push('\n');
            out.push_str(&local);
        }
    }
    Ok(out)
}

pub fn check(project_dir: &Path, env: &mut HookEnv, path: &Path) {
    let contents = match env.paths_config() {
        Ok(s) => s,
        Err(reason) => return env.deny(reason),
    };

    match is_path_allowed(&contents, project_dir, path) {
        Ok(true) => env.allow(format!("{path:?} permitted by paths config")),
        Ok(false) => env.deny(format!(
            "{path:?} not permitted by paths config\n\
             (prefix / match absolute paths, ./ or no prefix match relative to project directory)\n\
             ask the user to add a matching pattern to .claude/paths"
        )),
        Err(e) => env.deny(e),
    }
}

/// Pure predicate: returns `Ok(true)` if `path` is allowed by `paths_config`,
/// `Ok(false)` if denied, `Err` if a pattern fails to compile.
pub fn is_path_allowed(
    paths_config: &str,
    project_dir: &Path,
    path: &Path,
) -> Result<bool, String> {
    let patterns = config::partition(paths_config);

    for p in &patterns.deny {
        if match_pattern(p, project_dir, path)? {
            return Ok(false);
        }
    }

    if patterns.allow.is_empty() {
        return Ok(false);
    }

    for p in &patterns.allow {
        if match_pattern(p, project_dir, path)? {
            return Ok(true);
        }
    }

    Ok(false)
}

fn match_pattern(pattern: &str, project_dir: &Path, path: &Path) -> Result<bool, String> {
    let normalized = if pattern.starts_with('/') {
        path.to_path_buf()
    } else if pattern.starts_with("./") {
        match path.strip_prefix(project_dir) {
            Ok(rel) => Path::new(".").join(rel),
            Err(_) => return Ok(false),
        }
    } else {
        match path.strip_prefix(project_dir) {
            Ok(rel) => rel.to_path_buf(),
            Err(_) => return Ok(false),
        }
    };
    GlobBuilder::new(pattern)
        .literal_separator(true)
        .build()
        .map(|g| g.compile_matcher().is_match(&normalized))
        .map_err(|e| format!("pattern {pattern:?} failed to compile: {e}"))
}

#[cfg(test)]
mod tests {
    use crate::hooks::env::{HookEnv, PreToolDecision};
    use std::path::Path;

    fn env(paths: &str) -> HookEnv {
        HookEnv::test("", paths, "", "")
    }

    #[test]
    fn allows_matching_path() {
        let mut env = env("./**/*");
        super::check(Path::new("."), &mut env, Path::new("./src/main.rs"));
        assert_eq!(env.decision(), Some(&PreToolDecision::Allow));
    }

    #[test]
    fn denies_unmatched_path() {
        let mut env = env("src/**/*");
        super::check(Path::new("."), &mut env, Path::new("other/file.rs"));
        assert_eq!(env.decision(), Some(&PreToolDecision::Deny));
    }

    #[test]
    fn deny_pattern_blocks_allowed_path() {
        let mut env = env("./**/*\n!.env");
        super::check(Path::new("."), &mut env, Path::new(".env"));
        assert_eq!(env.decision(), Some(&PreToolDecision::Deny));
    }

    #[test]
    fn literal_separator_prevents_glob_crossing() {
        // ./* should match top-level files but not subdirectories
        let mut env = env("./*");
        super::check(Path::new("."), &mut env, Path::new("src/main.rs"));
        assert_eq!(env.decision(), Some(&PreToolDecision::Deny));
    }

    #[test]
    fn dotslash_pattern_matches_absolute_path() {
        let mut env = env("./**/*");
        super::check(
            Path::new("/project"),
            &mut env,
            Path::new("/project/src/main.rs"),
        );
        assert_eq!(env.decision(), Some(&PreToolDecision::Allow));
    }

    #[test]
    fn bare_pattern_matches_absolute_path() {
        let mut env = env("**/*");
        super::check(
            Path::new("/project"),
            &mut env,
            Path::new("/project/src/main.rs"),
        );
        assert_eq!(env.decision(), Some(&PreToolDecision::Allow));
    }

    #[test]
    fn absolute_pattern_matches_absolute_path() {
        let mut env = env("/project/**/*");
        super::check(
            Path::new("/project"),
            &mut env,
            Path::new("/project/src/main.rs"),
        );
        assert_eq!(env.decision(), Some(&PreToolDecision::Allow));
    }

    #[test]
    fn absolute_path_outside_project_denied() {
        let mut env = env("./**/*");
        super::check(Path::new("/project"), &mut env, Path::new("/etc/passwd"));
        assert_eq!(env.decision(), Some(&PreToolDecision::Deny));
    }

    #[test]
    fn bare_glob_star_does_not_match_outside_project() {
        let mut env = env("**");
        super::check(Path::new("/project"), &mut env, Path::new("/etc/passwd"));
        assert_eq!(env.decision(), Some(&PreToolDecision::Deny));
    }

    #[test]
    fn dotenv_and_dotslash_dotenv_deny_patterns_are_equivalent() {
        let path = Path::new("/project/.env");
        let project = Path::new("/project");

        let mut env1 = env("./**/*\n!.env");
        super::check(project, &mut env1, path);
        let d1 = env1.decision().cloned();

        let mut env2 = env("./**/*\n!./.env");
        super::check(project, &mut env2, path);
        let d2 = env2.decision().cloned();

        assert_eq!(d1, Some(PreToolDecision::Deny));
        assert_eq!(d1, d2);
    }

    #[test]
    fn load_config_merges_local_file() {
        use tempfile::TempDir;
        let dir = TempDir::new().unwrap();
        let claude = dir.path().join(".claude");
        std::fs::create_dir_all(&claude).unwrap();
        std::fs::write(claude.join("paths"), "**\n").unwrap();
        std::fs::write(claude.join("paths-local"), "!.env\n").unwrap();
        let combined = super::load_config(&claude).unwrap();
        assert!(combined.contains("**"));
        assert!(combined.contains("!.env"));
    }

    #[test]
    fn load_config_missing_local_is_ok() {
        use tempfile::TempDir;
        let dir = TempDir::new().unwrap();
        let claude = dir.path().join(".claude");
        std::fs::create_dir_all(&claude).unwrap();
        std::fs::write(claude.join("paths"), "**\n").unwrap();
        let combined = super::load_config(&claude).unwrap();
        assert!(combined.contains("**"));
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
