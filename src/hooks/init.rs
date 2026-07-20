use std::path::Path;

use super::env::HookEnv;
use crate::defaults;

pub fn init(project_dir: &Path, env: &mut HookEnv, inject: &str, mcp: &str) -> Result<(), String> {
    create_missing_configs(project_dir)?;
    write_readme(project_dir)?;
    update_gitignore(project_dir)?;
    inject_hooks(project_dir, env, inject, mcp)?;
    Ok(())
}

fn write_readme(project_dir: &Path) -> Result<(), String> {
    let path = project_dir.join(defaults::FISHING_README_PATH);
    std::fs::write(&path, defaults::FISHING_README_CONTENT)
        .map_err(|e| format!("failed to write {path:?}: {e}"))
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
    appended.push_str("\n# fishing config files:\n");
    for entry in missing {
        appended.push_str(entry);
        appended.push('\n');
    }
    std::fs::write(&gitignore, appended).map_err(|e| format!("failed to update .gitignore: {e}"))
}

fn inject_hooks(
    project_dir: &Path,
    env: &mut HookEnv,
    cmd: &str,
    mcp_cmd: &str,
) -> Result<(), String> {
    inject_mcp(project_dir, mcp_cmd)?;
    inject_hook_entries(env, cmd)
}

fn inject_mcp(project_dir: &Path, mcp_cmd: &str) -> Result<(), String> {
    let path = project_dir.join(".mcp.json");
    let raw = std::fs::read_to_string(&path).unwrap_or_else(|_| "{}".to_string());
    let mut root: serde_json::Value =
        serde_json::from_str(&raw).map_err(|e| format!(".mcp.json is not valid JSON: {e}"))?;

    let obj = root
        .as_object_mut()
        .ok_or_else(|| ".mcp.json root is not an object".to_string())?;

    let mcp_servers = obj
        .entry("mcpServers")
        .or_insert_with(|| serde_json::json!({}))
        .as_object_mut()
        .ok_or_else(|| ".mcp.json mcpServers field is not an object".to_string())?;

    mcp_servers.entry("grep-glob").or_insert_with(|| {
        serde_json::json!({
            "type": "stdio",
            "command": mcp_cmd.split_whitespace().next().unwrap_or(mcp_cmd),
            "args": mcp_cmd.split_whitespace().skip(1).collect::<Vec<_>>()
        })
    });

    let out = serde_json::to_string_pretty(&root)
        .map_err(|e| format!("failed to serialize .mcp.json: {e}"))?;
    std::fs::write(&path, out).map_err(|e| format!("failed to write .mcp.json: {e}"))
}

fn inject_hook_entries(env: &mut HookEnv, cmd: &str) -> Result<(), String> {
    let raw = env.settings_json()?;

    let mut root: serde_json::Value =
        serde_json::from_str(&raw).map_err(|e| format!("settings.json is not valid JSON: {e}"))?;

    let obj = root
        .as_object_mut()
        .ok_or_else(|| "settings.json root is not an object".to_string())?;

    let permissions = obj
        .entry("permissions")
        .or_insert_with(|| serde_json::json!({}))
        .as_object_mut()
        .ok_or_else(|| "settings.json permissions field is not an object".to_string())?;

    let allow = permissions
        .entry("allow")
        .or_insert_with(|| serde_json::json!([]))
        .as_array_mut()
        .ok_or_else(|| "settings.json permissions.allow field is not an array".to_string())?;

    for tool in ["mcp__grep-glob__glob", "mcp__grep-glob__grep", "ToolSearch"] {
        if !allow.iter().any(|v| v.as_str() == Some(tool)) {
            allow.push(serde_json::json!(tool));
        }
    }

    let enabled_mcp = obj
        .entry("enabledMcpjsonServers")
        .or_insert_with(|| serde_json::json!([]))
        .as_array_mut()
        .ok_or_else(|| "settings.json enabledMcpjsonServers field is not an array".to_string())?;

    if !enabled_mcp.iter().any(|v| v.as_str() == Some("grep-glob")) {
        enabled_mcp.push(serde_json::json!("grep-glob"));
    }

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
        let (dir, mut env) = setup("{}");
        init(dir.path(), &mut env, "fishing", "fishing-grep-glob-mcp").unwrap();
        assert!(dir.path().join(".claude/fishing.glob-exclude.txt").exists());
    }

    #[test]
    fn init_adds_glob_exclude_local_to_gitignore() {
        let (dir, mut env) = setup("{}");
        init(dir.path(), &mut env, "fishing", "fishing-grep-glob-mcp").unwrap();
        let gitignore = fs::read_to_string(dir.path().join(".gitignore")).unwrap();
        assert!(
            gitignore
                .lines()
                .any(|l| l.trim() == ".claude/fishing.glob-exclude.local.txt")
        );
    }

    #[test]
    fn inject_registers_mcp_server() {
        let (dir, mut env) = setup("{}");
        inject_hooks(
            dir.path(),
            &mut env,
            "/usr/local/bin/fishing",
            "/usr/local/bin/fishing-grep-glob-mcp",
        )
        .unwrap();
        let mcp_raw = fs::read_to_string(dir.path().join(".mcp.json")).unwrap();
        let out: serde_json::Value = serde_json::from_str(&mcp_raw).unwrap();
        assert_eq!(out["mcpServers"]["grep-glob"]["type"], "stdio");
        assert_eq!(
            out["mcpServers"]["grep-glob"]["command"],
            "/usr/local/bin/fishing-grep-glob-mcp"
        );
    }

    #[test]
    fn inject_mcp_is_idempotent() {
        let (dir, mut env) = setup("{}");
        inject_hooks(
            dir.path(),
            &mut env,
            "/usr/local/bin/fishing",
            "/usr/local/bin/fishing-grep-glob-mcp",
        )
        .unwrap();
        inject_hooks(
            dir.path(),
            &mut env,
            "/usr/local/bin/fishing",
            "/usr/local/bin/fishing-grep-glob-mcp",
        )
        .unwrap();
        let mcp_raw = fs::read_to_string(dir.path().join(".mcp.json")).unwrap();
        let out: serde_json::Value = serde_json::from_str(&mcp_raw).unwrap();
        assert!(out["mcpServers"]["grep-glob"].is_object());
        // mcpServers should contain exactly one entry
        assert_eq!(out["mcpServers"].as_object().unwrap().len(), 1);
    }

    #[test]
    fn inject_registers_mcp_tool_permissions() {
        let (dir, mut env) = setup("{}");
        inject_hooks(dir.path(), &mut env, "fishing", "fishing-grep-glob-mcp").unwrap();
        let out: serde_json::Value = serde_json::from_str(&env.settings_json().unwrap()).unwrap();
        let allow = out["permissions"]["allow"].as_array().unwrap();
        let tools: Vec<&str> = allow.iter().filter_map(|v| v.as_str()).collect();
        assert!(tools.contains(&"mcp__grep-glob__glob"));
        assert!(tools.contains(&"mcp__grep-glob__grep"));
    }

    #[test]
    fn inject_hooks_is_idempotent() {
        let (dir, mut env) = setup("{}");
        inject_hooks(dir.path(), &mut env, "fishing", "fishing-grep-glob-mcp").unwrap();
        inject_hooks(dir.path(), &mut env, "fishing", "fishing-grep-glob-mcp").unwrap();
        let out: serde_json::Value = serde_json::from_str(&env.settings_json().unwrap()).unwrap();
        assert_eq!(out["hooks"]["SessionStart"].as_array().unwrap().len(), 1);
        assert_eq!(out["hooks"]["PreToolUse"].as_array().unwrap().len(), 1);
        assert_eq!(out["hooks"]["ConfigChange"].as_array().unwrap().len(), 1);
        let allow = out["permissions"]["allow"].as_array().unwrap();
        let glob_count = allow
            .iter()
            .filter(|v| v.as_str() == Some("mcp__grep-glob__glob"))
            .count();
        let grep_count = allow
            .iter()
            .filter(|v| v.as_str() == Some("mcp__grep-glob__grep"))
            .count();
        assert_eq!(glob_count, 1);
        assert_eq!(grep_count, 1);
    }

    #[test]
    fn inject_enables_mcp_json_server() {
        let (dir, mut env) = setup("{}");
        inject_hooks(dir.path(), &mut env, "fishing", "fishing-grep-glob-mcp").unwrap();
        let out: serde_json::Value = serde_json::from_str(&env.settings_json().unwrap()).unwrap();
        let enabled = out["enabledMcpjsonServers"].as_array().unwrap();
        assert!(enabled.iter().any(|v| v.as_str() == Some("grep-glob")));
    }

    #[test]
    fn inject_enabled_mcp_json_server_is_idempotent() {
        let (dir, mut env) = setup("{}");
        inject_hooks(dir.path(), &mut env, "fishing", "fishing-grep-glob-mcp").unwrap();
        inject_hooks(dir.path(), &mut env, "fishing", "fishing-grep-glob-mcp").unwrap();
        let out: serde_json::Value = serde_json::from_str(&env.settings_json().unwrap()).unwrap();
        let count = out["enabledMcpjsonServers"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|v| v.as_str() == Some("grep-glob"))
            .count();
        assert_eq!(count, 1);
    }

    #[test]
    fn inject_registers_pre_tool_use_hook_without_glob_grep() {
        let (dir, mut env) = setup("{}");
        inject_hooks(dir.path(), &mut env, "fishing", "fishing-grep-glob-mcp").unwrap();
        let out: serde_json::Value = serde_json::from_str(&env.settings_json().unwrap()).unwrap();
        let hooks = out["hooks"]["PreToolUse"].as_array().unwrap();
        let matcher = hooks[0]["matcher"].as_str().unwrap();
        assert!(matcher.contains("Bash"));
        assert!(matcher.contains("Read"));
        assert!(!matcher.contains("Glob"));
        assert!(!matcher.contains("Grep"));
    }

    #[test]
    fn inject_registers_config_change_hook() {
        let (dir, mut env) = setup("{}");
        inject_hooks(dir.path(), &mut env, "fishing", "fishing-grep-glob-mcp").unwrap();
        let out: serde_json::Value = serde_json::from_str(&env.settings_json().unwrap()).unwrap();
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

    if let Some(arr) = array.as_array() {
        let already_present = arr.iter().any(|entry| {
            entry
                .get("hooks")
                .and_then(|h| h.as_array())
                .map(|h| {
                    h.iter().any(|hook| {
                        hook.get("command")
                            .and_then(|c| c.as_str())
                            .map(|c| c == command)
                            .unwrap_or(false)
                    })
                })
                .unwrap_or(false)
        });
        if already_present {
            return;
        }
    }

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
