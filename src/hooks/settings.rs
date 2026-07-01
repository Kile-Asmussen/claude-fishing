use serde_json::Value;
use std::path::Path;

use super::env::HookEnv;

const DEFAULT_SCHEMA_URL: &str = "https://json.schemastore.org/claude-code-settings.json";

#[derive(Default, Debug, Clone, PartialEq, Eq)]
pub enum Mode {
    JsonOnly,
    Schema(String),
    #[default]
    Default,
}

pub fn check(_project_dir: &Path, env: &mut HookEnv, mode: Mode, local: bool) {
    let text = if local {
        env.settings_local_json()
    } else {
        env.settings_json()
    };

    let text = match text {
        Ok(t) => t,
        Err(e) => return env.config_block(format!("could not read settings.json: {e}")),
    };

    let value: Value = match serde_json::from_str(&text) {
        Ok(v) => v,
        Err(e) => return env.config_block(format!("settings.json is not valid JSON: {e}")),
    };

    if mode == Mode::JsonOnly {
        return env.config_allow("settings.json is valid JSON");
    }

    let schema_text = match load_schema(mode) {
        Ok(s) => s,
        Err(e) => return env.config_block(format!("could not load schema: {e}")),
    };

    let schema: Value = match serde_json::from_str(&schema_text) {
        Ok(v) => v,
        Err(e) => return env.config_block(format!("schema is not valid JSON: {e}")),
    };

    let validator = match jsonschema::validator_for(&schema) {
        Ok(v) => v,
        Err(e) => return env.config_block(format!("could not compile schema: {e}")),
    };

    let errors: Vec<String> = validator
        .iter_errors(&value)
        .map(|e| format!("{} (at {})", e, e.instance_path()))
        .collect();

    if errors.is_empty() {
        env.config_allow("settings.json is valid")
    } else {
        env.config_block(format!(
            "settings.json failed schema validation:\n{}",
            errors.join("\n")
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::Mode;
    use crate::hooks::env::{ConfigDecision, HookEnv};
    use std::path::Path;

    fn env(settings: &str) -> HookEnv {
        HookEnv::test("", "", "", settings)
    }

    #[test]
    fn json_only_allows_valid_json() {
        let mut env = env(r#"{"key":"value"}"#);
        super::check(Path::new("."), &mut env, Mode::JsonOnly, false);
        assert_eq!(env.config_decision(), Some(&ConfigDecision::Allow));
    }

    #[test]
    fn json_only_blocks_invalid_json() {
        let mut env = env("not json {{{");
        super::check(Path::new("."), &mut env, Mode::JsonOnly, false);
        assert_eq!(env.config_decision(), Some(&ConfigDecision::Block));
    }

    #[test]
    fn json_only_blocks_unreadable_settings() {
        let mut env = HookEnv::test("", "", "", "");
        // simulate unreadable by using a File path that doesn't exist
        env.settings = crate::hooks::env::HookConfig::File("/nonexistent/settings.json".into());
        super::check(Path::new("."), &mut env, Mode::JsonOnly, false);
        assert_eq!(env.config_decision(), Some(&ConfigDecision::Block));
    }

    fn write_schema(name: &str, schema: serde_json::Value) -> String {
        let path = std::env::temp_dir().join(name);
        std::fs::write(&path, schema.to_string()).unwrap();
        format!("file://{}", path.display())
    }

    #[test]
    fn schema_file_allows_conforming_settings() {
        let url = write_schema(
            "fishing_test_schema_allow.json",
            serde_json::json!({
                "type": "object",
                "properties": { "key": { "type": "string" } }
            }),
        );
        let mut env = env(r#"{"key":"value"}"#);
        super::check(Path::new("."), &mut env, Mode::Schema(url), false);
        assert_eq!(env.config_decision(), Some(&ConfigDecision::Allow));
    }

    #[test]
    fn schema_file_blocks_non_conforming_settings() {
        let url = write_schema(
            "fishing_test_schema_block.json",
            serde_json::json!({
                "type": "object",
                "properties": { "key": { "type": "string" } },
                "required": ["key"]
            }),
        );
        let mut env = env(r#"{}"#);
        super::check(Path::new("."), &mut env, Mode::Schema(url), false);
        assert_eq!(env.config_decision(), Some(&ConfigDecision::Block));
    }
}

fn load_schema(mode: Mode) -> Result<String, String> {
    let url = match mode {
        Mode::Schema(ref s) => s.as_str(),
        Mode::Default => DEFAULT_SCHEMA_URL,
        Mode::JsonOnly => unreachable!(),
    };

    if let Some(path) = url.strip_prefix("file://") {
        return std::fs::read_to_string(path)
            .map_err(|e| format!("could not read schema file {path:?}: {e}"));
    }

    reqwest::blocking::get(url)
        .map_err(|e| format!("could not fetch schema from {url}: {e}"))?
        .text()
        .map_err(|e| format!("could not read schema response: {e}"))
}
