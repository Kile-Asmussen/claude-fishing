use globset::GlobBuilder;
use serde::Deserialize;
use std::path::Path;

use super::env::HookEnv;

#[derive(Deserialize)]
struct Settings {
    permissions: Option<Permissions>,
}

#[derive(Deserialize)]
struct Permissions {
    #[serde(default)]
    allow: Vec<String>,
}

pub fn check(project_dir: &Path, env: &mut HookEnv, tool: &str, file_path: &Path) {
    // Path must be within project directory.
    if file_path.strip_prefix(project_dir).is_err() {
        return env.deny(format!(
            "{tool}({file_path:?}) is outside the project directory ({project_dir:?}); \
             only files within the project may be written"
        ));
    }

    // Path must not be within .claude/.
    if file_path.strip_prefix(project_dir.join(".claude")).is_ok() {
        return env.deny(format!(
            "{tool}({file_path:?}) is within the protected .claude/ directory; \
             ask the user to make this change manually"
        ));
    }

    let globs = match load_globs(env, tool) {
        Ok(g) => g,
        Err(e) => return env.deny(e),
    };

    if globs.is_empty() {
        return env.deny(format!(
            "no {tool}(...) patterns found in .claude/settings.json permissions.allow; \
             ask the user to add a {tool}(glob) entry covering this path"
        ));
    }

    let rel = match file_path.strip_prefix(project_dir) {
        Ok(r) => r,
        Err(_) => return env.deny("could not relativize path".to_string()),
    };

    for glob in &globs {
        match GlobBuilder::new(glob).literal_separator(true).build() {
            Ok(g) => {
                if g.compile_matcher().is_match(rel) {
                    return env.allow(format!("{tool}({file_path:?}) permitted by {tool}({glob})"));
                }
            }
            Err(e) => return env.deny(format!("pattern {glob:?} failed to compile: {e}")),
        }
    }

    env.deny(format!(
        "{tool}({rel:?}) not matched by any allowed pattern; \
         allowed: [{}]; \
         ask the user to add a {tool}(glob) entry to .claude/settings.json covering this path",
        globs
            .iter()
            .map(|g| format!("{tool}({g})"))
            .collect::<Vec<_>>()
            .join(", ")
    ))
}

#[cfg(test)]
mod tests {
    use crate::hooks::env::{HookEnv, PreToolDecision};
    use std::path::Path;

    fn settings(json: &str) -> HookEnv {
        HookEnv::test("", "", "", json)
    }

    const SETTINGS_WITH_EDIT: &str = r#"{"permissions":{"allow":["Edit(src/**/*.rs)"]}}"#;

    #[test]
    fn allows_matching_path() {
        let mut env = settings(SETTINGS_WITH_EDIT);
        super::check(
            Path::new("/proj"),
            &mut env,
            "Edit",
            Path::new("/proj/src/main.rs"),
        );
        assert_eq!(env.decision(), Some(&PreToolDecision::Allow));
    }

    #[test]
    fn denies_unmatched_path() {
        let mut env = settings(SETTINGS_WITH_EDIT);
        super::check(
            Path::new("/proj"),
            &mut env,
            "Edit",
            Path::new("/proj/other/file.txt"),
        );
        assert_eq!(env.decision(), Some(&PreToolDecision::Deny));
    }

    #[test]
    fn denies_outside_project() {
        let mut env = settings(SETTINGS_WITH_EDIT);
        super::check(
            Path::new("/proj"),
            &mut env,
            "Edit",
            Path::new("/etc/passwd"),
        );
        assert_eq!(env.decision(), Some(&PreToolDecision::Deny));
    }

    #[test]
    fn denies_inside_claude_dir() {
        let mut env = settings(SETTINGS_WITH_EDIT);
        super::check(
            Path::new("/proj"),
            &mut env,
            "Edit",
            Path::new("/proj/.claude/settings.json"),
        );
        assert_eq!(env.decision(), Some(&PreToolDecision::Deny));
    }

    #[test]
    fn denies_when_no_tool_patterns() {
        let mut env = settings(r#"{"permissions":{"allow":["Write(src/**/*.rs)"]}}"#);
        super::check(
            Path::new("/proj"),
            &mut env,
            "Edit",
            Path::new("/proj/src/main.rs"),
        );
        assert_eq!(env.decision(), Some(&PreToolDecision::Deny));
    }

    #[test]
    fn denies_on_invalid_settings_json() {
        let mut env = settings("not json");
        super::check(
            Path::new("/proj"),
            &mut env,
            "Edit",
            Path::new("/proj/src/main.rs"),
        );
        assert_eq!(env.decision(), Some(&PreToolDecision::Deny));
    }

    #[test]
    fn tool_patterns_are_not_shared_across_tools() {
        let mut env = settings(r#"{"permissions":{"allow":["Edit(**/*)", "Write(other/**)"]}}"#);
        super::check(
            Path::new("/proj"),
            &mut env,
            "Write",
            Path::new("/proj/src/main.rs"),
        );
        assert_eq!(env.decision(), Some(&PreToolDecision::Deny));
    }
}

fn load_globs(env: &mut HookEnv, tool: &str) -> Result<Vec<String>, String> {
    let text = env.settings_json()?;
    let settings: Settings =
        serde_json::from_str(&text).map_err(|e| format!("failed to parse settings.json: {e}"))?;

    let allow = settings.permissions.map(|p| p.allow).unwrap_or_default();

    let prefix = format!("{tool}(");
    Ok(allow
        .into_iter()
        .filter_map(|entry| {
            entry
                .strip_prefix(&prefix)?
                .strip_suffix(')')?
                .to_string()
                .into()
        })
        .collect())
}
