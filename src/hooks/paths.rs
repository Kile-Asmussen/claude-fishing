use std::path::Path;
use globset::GlobBuilder;

use super::config;
use super::env::HookEnv;

pub fn check(_project_dir: &Path, env: &mut HookEnv, path: &Path) {
    let contents = match env.paths_config() {
        Ok(s) => s,
        Err(reason) => return env.deny(reason),
    };

    let patterns = config::partition(&contents);
    let path_str = path.to_string_lossy();

    config::check(
        env,
        &patterns,
        &path_str,
        "no allowed paths in .claude/paths",
        |p| match_pattern(p, path),
    );
}

fn match_pattern(pattern: &str, path: &Path) -> Result<bool, String> {
    GlobBuilder::new(pattern)
        .literal_separator(true)
        .build()
        .map(|g| g.compile_matcher().is_match(path))
        .map_err(|e| format!("pattern {pattern:?} failed to compile: {e}"))
}

#[cfg(test)]
mod tests {
    use std::path::Path;
    use crate::hooks::env::{HookEnv, PreToolDecision};

    fn env(paths: &str) -> HookEnv { HookEnv::test("", paths, "", "") }

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
}

