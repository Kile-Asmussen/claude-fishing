use std::path::Path;
use wildcard::Wildcard;

use super::config;
use super::env::HookEnv;

pub fn check(_project_dir: &Path, env: &mut HookEnv, url: &str) {
    let contents = match env.webfetch_config() {
        Ok(s) => s,
        Err(reason) => return env.deny(reason),
    };

    let patterns = config::partition(&contents);
    config::check(
        env,
        &patterns,
        url,
        "no allowed WebFetch URLs in .claude/webfetch",
        "",
        |p| match_pattern(p, url),
    );
}

fn match_pattern(pattern: &str, url: &str) -> Result<bool, String> {
    Wildcard::new(pattern.as_bytes())
        .map(|wc| wc.is_match(url.as_bytes()))
        .map_err(|e| format!("pattern {pattern:?} failed to compile: {e}"))
}

#[cfg(test)]
mod tests {
    use crate::hooks::env::{HookEnv, PreToolDecision};
    use std::path::Path;

    fn env(webfetch: &str) -> HookEnv {
        HookEnv::test("", "", webfetch, "")
    }

    #[test]
    fn allows_matching_url() {
        let mut env = env("https://docs.rs/*");
        super::check(
            Path::new("."),
            &mut env,
            "https://docs.rs/regex/latest/regex/",
        );
        assert_eq!(env.decision(), Some(&PreToolDecision::Allow));
    }

    #[test]
    fn denies_unmatched_url() {
        let mut env = env("https://docs.rs/*");
        super::check(Path::new("."), &mut env, "https://evil.com/steal");
        assert_eq!(env.decision(), Some(&PreToolDecision::Deny));
    }

    #[test]
    fn deny_pattern_blocks_allowed_url() {
        let mut env = env("https://docs.rs/*\n!https://docs.rs/secret/*");
        super::check(Path::new("."), &mut env, "https://docs.rs/secret/thing");
        assert_eq!(env.decision(), Some(&PreToolDecision::Deny));
    }

    #[test]
    fn wildcard_does_not_cross_scheme() {
        let mut env = env("https://docs.rs/*");
        super::check(Path::new("."), &mut env, "http://docs.rs/crate");
        assert_eq!(env.decision(), Some(&PreToolDecision::Deny));
    }
}
