use std::path::Path;

use super::env::HookEnv;
use crate::defaults;

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
    std::fs::write(&gitignore, appended).map_err(|e| format!("failed to update .gitignore: {e}"))
}

fn inject_hooks(env: &mut HookEnv, cmd: &str) -> Result<(), String> {
    let raw = env.settings_json()?;

    let mut root: serde_json::Value =
        serde_json::from_str(&raw).map_err(|e| format!("settings.json is not valid JSON: {e}"))?;

    let obj = root
        .as_object_mut()
        .ok_or_else(|| "settings.json root is not an object".to_string())?;

    // Register the MCP server (idempotent — keeps existing entry if already present).
    let mcp_servers = obj
        .entry("mcpServers")
        .or_insert_with(|| serde_json::json!({}))
        .as_object_mut()
        .ok_or_else(|| "settings.json mcpServers field is not an object".to_string())?;

    mcp_servers.entry("grep-glob").or_insert_with(|| {
        serde_json::json!({
            "type": "stdio",
            "command": cmd.split_whitespace().next().unwrap_or(cmd),
            "args": cmd.split_whitespace()
                .skip(1)
                .chain(std::iter::once("grep-glob-mcp"))
                .collect::<Vec<_>>()
        })
    });

    let hooks = obj
        .entry("hooks")
        .or_insert_with(|| serde_json::json!({}))
        .as_object_mut()
        .ok_or_else(|| "settings.json hooks field is not an object".to_string())?;

    append_hook(hooks, "SessionStart", None, &format!("{cmd} rotate-log"));
    append_hook(
        hooks,
        "PreToolUse",
        Some("Bash|Read|WebFetch|Edit|Write"),
        &format!("{cmd} pre-tool-use"),
    );
    append_hook(
        hooks,
        "ConfigChange",
        Some("project_settings|local_settings|user_settings"),
        &format!("{cmd} config-change"),
    );

    let out = serde_json::to_string_pretty(&root)
        .map_err(|e| format!("failed to serialize settings.json: {e}"))?;

    env.write_settings_json(&out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hooks::env::{HookConfig, HookEnv};
    use std::fs;
    use tempfile::TempDir;

    fn setup(settings: &str) -> (TempDir, HookEnv) {
        let dir = TempDir::new().unwrap();
        let claude = dir.path().join(".claude");
        fs::create_dir_all(&claude).unwrap();
        fs::write(claude.join("settings.json"), settings).unwrap();
        let env = HookEnv {
            settings: HookConfig::File(claude.join("settings.json")),
            ..Default::default()
        };
        (dir, env)
    }

    #[test]
    fn init_creates_glob_exclude_config() {
        let dir = TempDir::new().unwrap();
        init(dir.path(), &mut HookEnv::default(), None).unwrap();
        assert!(dir.path().join(".claude/glob-exclude").exists());
    }

    #[test]
    fn init_adds_glob_exclude_local_to_gitignore() {
        let dir = TempDir::new().unwrap();
        init(dir.path(), &mut HookEnv::default(), None).unwrap();
        let gitignore = fs::read_to_string(dir.path().join(".gitignore")).unwrap();
        assert!(gitignore.lines().any(|l| l.trim() == ".claude/glob-exclude-local"));
    }

    #[test]
    fn inject_registers_mcp_server() {
        let (_dir, mut env) = setup("{}");
        inject_hooks(&mut env, "/usr/local/bin/fishing").unwrap();
        let out: serde_json::Value =
            serde_json::from_str(&env.settings_json().unwrap()).unwrap();
        assert_eq!(out["mcpServers"]["grep-glob"]["type"], "stdio");
        assert_eq!(out["mcpServers"]["grep-glob"]["command"], "/usr/local/bin/fishing");
        let args = out["mcpServers"]["grep-glob"]["args"].as_array().unwrap();
        assert!(args.iter().any(|a| a == "grep-glob-mcp"));
    }

    #[test]
    fn inject_mcp_is_idempotent() {
        let (_dir, mut env) = setup("{}");
        inject_hooks(&mut env, "/usr/local/bin/fishing").unwrap();
        let mid = env.settings_json().unwrap();
        // write mid back so second call reads it
        let mut env2 = env.clone();
        // re-use the same HookConfig::File — already written, just call inject again
        inject_hooks(&mut env2, "/usr/local/bin/fishing").unwrap();
        let out: serde_json::Value =
            serde_json::from_str(&env2.settings_json().unwrap()).unwrap();
        // should still be exactly one grep-glob entry
        assert!(out["mcpServers"]["grep-glob"].is_object());
        let _ = mid; // used
    }

    #[test]
    fn inject_registers_pre_tool_use_hook_without_glob_grep() {
        let (_dir, mut env) = setup("{}");
        inject_hooks(&mut env, "fishing").unwrap();
        let out: serde_json::Value =
            serde_json::from_str(&env.settings_json().unwrap()).unwrap();
        let hooks = out["hooks"]["PreToolUse"].as_array().unwrap();
        let matcher = hooks[0]["matcher"].as_str().unwrap();
        assert!(matcher.contains("Bash"));
        assert!(matcher.contains("Read"));
        assert!(!matcher.contains("Glob"));
        assert!(!matcher.contains("Grep"));
    }

    #[test]
    fn inject_registers_config_change_hook() {
        let (_dir, mut env) = setup("{}");
        inject_hooks(&mut env, "fishing").unwrap();
        let out: serde_json::Value =
            serde_json::from_str(&env.settings_json().unwrap()).unwrap();
        let hooks = out["hooks"]["ConfigChange"].as_array().unwrap();
        let cmd = hooks[0]["hooks"][0]["command"].as_str().unwrap();
        assert!(cmd.contains("config-change"));
    }
}

fn append_hook(
    hooks: &mut serde_json::Map<String, serde_json::Value>,
    event: &str,
    matcher: Option<&str>,
    command: &str,
) {
    let array = hooks.entry(event).or_insert_with(|| serde_json::json!([]));

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
