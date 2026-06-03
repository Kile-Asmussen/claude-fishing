use std::path::Path;
use regex::Regex;

use super::config;
use super::env::HookEnv;

pub fn check(_project_dir: &Path, env: &mut HookEnv, command: &str) {
    let contents = match env.bash_config() {
        Ok(s) => s,
        Err(reason) => return env.deny(reason),
    };

    let patterns = config::partition(&contents);
    config::check(env, &patterns, command, "no allowed Bash patterns in .claude/bash", |p| {
        let anchored = format!(r"\A(?:{p})\z");
        Regex::new(&anchored)
            .map(|re| re.is_match(command))
            .map_err(|e| format!("pattern {p:?} failed to compile: {e}"))
    });
}

#[cfg(test)]
mod tests {
    use std::path::Path;
    use crate::hooks::env::{HookEnv, PreToolDecision};

    fn env(bash: &str) -> HookEnv { HookEnv::test(bash, "", "", "") }

    #[test]
    fn allows_matching_command() {
        let mut env = env("cargo build");
        super::check(Path::new("."), &mut env, "cargo build");
        assert_eq!(env.decision(), Some(&PreToolDecision::Allow));
    }

    #[test]
    fn denies_non_matching_command() {
        let mut env = env("cargo build");
        super::check(Path::new("."), &mut env, "rm -rf /");
        assert_eq!(env.decision(), Some(&PreToolDecision::Deny));
    }

    #[test]
    fn regex_anchoring_prevents_partial_match() {
        let mut env = env("cargo build");
        super::check(Path::new("."), &mut env, "cargo build && rm -rf /");
        assert_eq!(env.decision(), Some(&PreToolDecision::Deny));
    }

    #[test]
    fn deny_pattern_blocks_allowed_command() {
        let mut env = env("cargo.*\n!cargo build");
        super::check(Path::new("."), &mut env, "cargo build");
        assert_eq!(env.decision(), Some(&PreToolDecision::Deny));
    }

    #[test]
    fn regex_allows_wildcard_pattern() {
        let mut env = env("cargo .*");
        super::check(Path::new("."), &mut env, "cargo test");
        assert_eq!(env.decision(), Some(&PreToolDecision::Allow));
    }
}

