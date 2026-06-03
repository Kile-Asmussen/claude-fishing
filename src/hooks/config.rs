use super::env::HookEnv;

pub struct Patterns<'a> {
    pub allow: Vec<&'a str>,
    pub deny: Vec<&'a str>,
}

pub fn partition(contents: &str) -> Patterns<'_> {
    let mut allow = Vec::new();
    let mut deny = Vec::new();

    for line in contents.lines().map(str::trim).filter(|l| !l.is_empty() && !l.starts_with('#')) {
        if let Some(rest) = line.strip_prefix('!') {
            deny.push(rest);
        } else {
            allow.push(line);
        }
    }

    Patterns { allow, deny }
}

/// Run the standard allow/deny check, calling `matches(pattern)` for each pattern.
/// The matcher returns `Err` to short-circuit with a deny, or `Ok(bool)` for the match result.
/// Emits env.allow or env.deny and returns.
pub fn check<F>(env: &mut HookEnv, patterns: &Patterns<'_>, subject: &str, empty_msg: &str, matches: F)
where
    F: Fn(&str) -> Result<bool, String>,
{
    if patterns.allow.is_empty() {
        return env.deny(format!(
            "{empty_msg}; add allowed patterns to the config file and retry"
        ));
    }

    let mut allowed_by = None;
    for &p in &patterns.allow {
        match matches(p) {
            Err(e) => return env.deny(e),
            Ok(true) => { allowed_by = Some(p); break; }
            Ok(false) => {}
        }
    }

    if allowed_by.is_none() {
        return env.deny(format!(
            "{subject:?} not matched by any allowed pattern; \
             allowed: [{}]; \
             ask the user to add a matching pattern to the config file",
            patterns.allow.join(", ")
        ));
    }

    for &p in &patterns.deny {
        match matches(p) {
            Err(e) => return env.deny(e),
            Ok(true) => return env.deny(format!(
                "{subject:?} is explicitly blocked by deny pattern {p:?}; \
                 ask the user to remove or narrow the deny rule if this was unintended"
            )),
            Ok(false) => {}
        }
    }

    env.allow(format!("{subject:?} permitted by pattern {:?}", allowed_by.unwrap()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hooks::env::{HookEnv, PreToolDecision};

    #[test]
    fn partition_basic() {
        let p = partition("allow_this\n!deny_this\n# comment\n\nallow_also");
        assert_eq!(p.allow, vec!["allow_this", "allow_also"]);
        assert_eq!(p.deny, vec!["deny_this"]);
    }

    #[test]
    fn partition_empty() {
        let p = partition("# just a comment\n\n");
        assert!(p.allow.is_empty());
        assert!(p.deny.is_empty());
    }

    #[test]
    fn check_empty_allowlist_denies() {
        let mut env = HookEnv::test("", "", "", "");
        let p = partition("");
        check(&mut env, &p, "anything", "no patterns", |_| Ok(true));
        assert_eq!(env.decision(), Some(&PreToolDecision::Deny));
    }

    #[test]
    fn check_explicit_deny_wins() {
        let mut env = HookEnv::test("", "", "", "");
        let p = partition("foo\n!foo");
        check(&mut env, &p, "foo", "no patterns", |pat| Ok(pat == "foo"));
        assert_eq!(env.decision(), Some(&PreToolDecision::Deny));
    }
}
