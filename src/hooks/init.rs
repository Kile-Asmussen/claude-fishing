use std::path::Path;

use crate::defaults;
use super::env::HookEnv;

pub fn init(project_dir: &Path, env: &mut HookEnv, inject: Option<&str>) -> Result<(), String> {
    create_missing_configs(project_dir)?;
    update_gitignore(project_dir)?;

    if let Some(cmd) = inject {
        inject_hooks(env, cmd)?;
    }

    Ok(())
}

fn create_missing_configs(project_dir: &Path) -> Result<(), String> {
    for f in defaults::missing_files(project_dir) {
        let path = project_dir.join(f.rel_path);
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        std::fs::write(&path, f.default_content)
            .map_err(|e| format!("failed to create {path:?}: {e}"))?;
    }
    Ok(())
}

fn update_gitignore(project_dir: &Path) -> Result<(), String> {
    let gitignore = project_dir.join(".gitignore");
    let contents = std::fs::read_to_string(&gitignore).unwrap_or_default();
    let missing = defaults::missing_gitignore_entries(&contents);

    if missing.is_empty() {
        return Ok(());
    }

    let mut appended = contents;
    if !appended.ends_with('\n') && !appended.is_empty() {
        appended.push('\n');
    }
    for entry in missing {
        appended.push_str(entry);
        appended.push('\n');
    }
    std::fs::write(&gitignore, appended)
        .map_err(|e| format!("failed to update .gitignore: {e}"))
}

fn inject_hooks(env: &mut HookEnv, cmd: &str) -> Result<(), String> {
    let raw = env.settings_json()?;

    let mut root: serde_json::Value = serde_json::from_str(&raw)
        .map_err(|e| format!("settings.json is not valid JSON: {e}"))?;

    let hooks = root
        .as_object_mut()
        .and_then(|o| {
            if !o.contains_key("hooks") {
                o.insert("hooks".into(), serde_json::json!({}));
            }
            o.get_mut("hooks")
        })
        .and_then(|v| v.as_object_mut())
        .ok_or_else(|| "settings.json hooks field is not an object".to_string())?;

    append_hook(hooks, "SessionStart",  None,                                           &format!("{cmd} rotate-log"));
    append_hook(hooks, "PreToolUse",    Some("Bash|Read|Glob|Grep|WebFetch|Edit|Write"), &format!("{cmd} tool-use"));
    append_hook(hooks, "ConfigChange",  Some("project_settings"),                        &format!("{cmd} settings"));

    let out = serde_json::to_string_pretty(&root)
        .map_err(|e| format!("failed to serialise settings.json: {e}"))?;

    env.write_settings_json(&out)
}

fn append_hook(
    hooks: &mut serde_json::Map<String, serde_json::Value>,
    event: &str,
    matcher: Option<&str>,
    command: &str,
) {
    let array = hooks
        .entry(event)
        .or_insert_with(|| serde_json::json!([]));

    let entry = if let Some(m) = matcher {
        serde_json::json!({
            "matcher": m,
            "hooks": [{"type": "command", "command": command}]
        })
    } else {
        serde_json::json!({
            "hooks": [{"type": "command", "command": command}]
        })
    };

    if let Some(arr) = array.as_array_mut() {
        arr.push(entry);
    }
}
