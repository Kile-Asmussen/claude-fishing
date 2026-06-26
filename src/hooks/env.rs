use serde::Serialize;
use std::io::Write as _;
use std::path::PathBuf;

pub enum HookConfig {
    #[allow(unused, reason = "for testing")]
    Direct(String),
    File(PathBuf),
}

impl HookConfig {
    fn load(&self, label: &str) -> Result<String, String> {
        match self {
            HookConfig::Direct(s) => Ok(s.clone()),
            HookConfig::File(path) => std::fs::read_to_string(path)
                .map_err(|e| format!("could not read {label} ({path:?}): {e}")),
        }
    }

    fn write(&self, label: &str, value: &str) -> Result<(), String> {
        match self {
            HookConfig::Direct(_) => {
                Err(format!("cannot write {label}: config is Direct, not File"))
            }
            HookConfig::File(path) => std::fs::write(path, value)
                .map_err(|e| format!("could not write {label} ({path:?}): {e}")),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum PreToolDecision {
    Allow,
    #[default]
    Deny,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum ConfigDecision {
    Allow,
    #[default]
    Block,
}

#[derive(Debug, Clone, Copy, Serialize, Default)]
pub enum PreToolUseLiteral {
    #[default]
    PreToolUse,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum HookResponse {
    #[serde(rename_all = "camelCase")]
    HookSpecificOutput {
        hook_event_name: PreToolUseLiteral,
        permission_decision: PreToolDecision,
        permission_decision_reason: String,
    },
    #[serde(untagged)]
    ConfigChange {
        decision: ConfigDecision,
        reason: String,
    },
}

impl HookResponse {
    fn log_label(&self) -> &'static str {
        match self {
            HookResponse::HookSpecificOutput {
                permission_decision: PreToolDecision::Allow,
                ..
            } => "ALLOW",
            HookResponse::HookSpecificOutput { .. } => "DENY",
            HookResponse::ConfigChange {
                decision: ConfigDecision::Allow,
                ..
            } => "CONFIG_ALLOW",
            HookResponse::ConfigChange { .. } => "CONFIG_BLOCK",
        }
    }

    fn reason(&self) -> &str {
        match self {
            HookResponse::HookSpecificOutput {
                permission_decision_reason,
                ..
            } => permission_decision_reason,
            HookResponse::ConfigChange { reason, .. } => reason,
        }
    }
}

pub struct HookEnv {
    pub bash: HookConfig,
    pub paths: HookConfig,
    pub webfetch: HookConfig,
    pub settings: HookConfig,
    pub log_path: Option<PathBuf>,
    pub log_buf: String,
    pub response: Option<HookResponse>,
}

impl HookEnv {
    /// Construct for testing, supplying config contents directly; logging is suppressed.
    #[allow(unused, reason = "for testing")]
    pub fn test(
        bash: impl Into<String>,
        paths: impl Into<String>,
        webfetch: impl Into<String>,
        settings: impl Into<String>,
    ) -> Self {
        HookEnv {
            bash: HookConfig::Direct(bash.into()),
            paths: HookConfig::Direct(paths.into()),
            webfetch: HookConfig::Direct(webfetch.into()),
            settings: HookConfig::Direct(settings.into()),
            log_path: None,
            log_buf: String::new(),
            response: None,
        }
    }

    // ── Config accessors ──────────────────────────────────────────────────────

    pub fn bash_config(&self) -> Result<String, String> {
        self.bash.load("bash config")
    }

    pub fn paths_config(&self) -> Result<String, String> {
        self.paths.load("paths config")
    }

    pub fn webfetch_config(&self) -> Result<String, String> {
        self.webfetch.load("webfetch config")
    }

    pub fn settings_json(&self) -> Result<String, String> {
        self.settings.load("settings.json")
    }

    pub fn write_settings_json(&self, value: &str) -> Result<(), String> {
        self.settings.write("settings.json", value)
    }

    // ── Logging ───────────────────────────────────────────────────────────────

    pub fn log(&mut self, line: impl Into<String>) {
        self.log_buf.push_str(&line.into());
        self.log_buf.push('\n');
    }

    // ── Response emitters ─────────────────────────────────────────────────────

    fn push(&mut self, response: HookResponse) {
        assert!(
            self.response.is_none(),
            "HookEnv: second response emitted (would produce invalid JSON output)"
        );
        self.log(format!("[{}] {}", response.log_label(), response.reason()));
        self.response = Some(response);
    }

    pub fn allow(&mut self, reason: impl Into<String>) {
        self.push(HookResponse::HookSpecificOutput {
            hook_event_name: PreToolUseLiteral::PreToolUse,
            permission_decision: PreToolDecision::Allow,
            permission_decision_reason: reason.into(),
        });
    }

    pub fn deny(&mut self, reason: impl Into<String>) {
        self.push(HookResponse::HookSpecificOutput {
            hook_event_name: PreToolUseLiteral::PreToolUse,
            permission_decision: PreToolDecision::Deny,
            permission_decision_reason: reason.into(),
        });
    }

    pub fn config_allow(&mut self, reason: impl Into<String>) {
        self.push(HookResponse::ConfigChange {
            decision: ConfigDecision::Allow,
            reason: reason.into(),
        });
    }

    pub fn config_block(&mut self, reason: impl Into<String>) {
        self.push(HookResponse::ConfigChange {
            decision: ConfigDecision::Block,
            reason: reason.into(),
        });
    }

    // ── Output ────────────────────────────────────────────────────────────────

    pub fn flush(&mut self) {
        if let Some(ref r) = self.response {
            let json = serde_json::to_string(r).unwrap();
            println!("{json}");
            self.log(format!("verdict: {json}"));
        }
        if !self.log_buf.is_empty() {
            if let Some(ref path) = self.log_path {
                if let Ok(mut f) = std::fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(path)
                {
                    let _ = f.write_all(self.log_buf.as_bytes());
                }
            }
        }
    }

    pub fn response(&self) -> Option<&HookResponse> {
        self.response.as_ref()
    }

    #[cfg(test)]
    pub fn decision(&self) -> Option<&PreToolDecision> {
        match self.response.as_ref()? {
            HookResponse::HookSpecificOutput {
                permission_decision,
                ..
            } => Some(permission_decision),
            HookResponse::ConfigChange { .. } => None,
        }
    }

    #[cfg(test)]
    pub fn config_decision(&self) -> Option<&ConfigDecision> {
        match self.response.as_ref()? {
            HookResponse::ConfigChange { decision, .. } => Some(decision),
            HookResponse::HookSpecificOutput { .. } => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serialize_pre_tool_allow() {
        let mut env = HookEnv::test("", "", "", "");
        env.allow("test reason");
        let json = serde_json::to_string(env.response().unwrap()).unwrap();
        let v: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(v["hookSpecificOutput"]["permissionDecision"], "allow");
        assert_eq!(v["hookSpecificOutput"]["hookEventName"], "PreToolUse");
        assert_eq!(
            v["hookSpecificOutput"]["permissionDecisionReason"],
            "test reason"
        );
    }

    #[test]
    fn serialize_pre_tool_deny() {
        let mut env = HookEnv::test("", "", "", "");
        env.deny("blocked");
        let json = serde_json::to_string(env.response().unwrap()).unwrap();
        let v: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(v["hookSpecificOutput"]["permissionDecision"], "deny");
    }

    #[test]
    fn serialize_config_allow() {
        let mut env = HookEnv::test("", "", "", "");
        env.config_allow("ok");
        let json = serde_json::to_string(env.response().unwrap()).unwrap();
        let v: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(v["decision"], "allow");
        assert_eq!(v["reason"], "ok");
        assert!(v.get("hookSpecificOutput").is_none());
    }

    #[test]
    fn serialize_config_block() {
        let mut env = HookEnv::test("", "", "", "");
        env.config_block("bad settings");
        let json = serde_json::to_string(env.response().unwrap()).unwrap();
        let v: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(v["decision"], "block");
    }

    #[test]
    #[should_panic]
    fn double_response_panics() {
        let mut env = HookEnv::test("", "", "", "");
        env.allow("first");
        env.deny("second");
    }
}
