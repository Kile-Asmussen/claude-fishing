use serde::{Deserialize, Serialize};
use std::path::Path;
use strum::VariantNames;
use strum_macros::VariantNames;

use crate::hooks;
use crate::hooks::env::HookEnv;
use crate::hooks::settings::Mode;

#[derive(Debug, Clone, Deserialize)]
pub struct ConfigChangeInput {
    #[serde(default)]
    pub cwd: String,
    pub source: Source,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Deserialize, VariantNames)]
#[serde(rename_all = "snake_case")]
pub enum Source {
    ProjectSettings,
    LocalSettings,
    #[default]
    #[serde(skip)]
    #[strum(disabled)]
    Empty,
    #[strum(disabled)]
    #[serde(untagged)]
    Other(String),
}

/// Top-level JSON object received on stdin for every PreToolUse hook invocation.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct PreToolUseInput {
    #[serde(default)]
    pub cwd: String,
    #[serde(default)]
    pub hook_event_name: String,
    #[serde(default)]
    pub permission_mode: String,
    pub agent_id: Option<String>,
    pub agent_type: Option<String>,

    #[serde(flatten)]
    pub tool_input: Option<ToolInput>,
}

/// Per-tool input payloads. Unknown tools are captured by `Other`.
#[derive(Debug, Clone, Deserialize, VariantNames)]
#[serde(tag = "tool_name", content = "tool_input", rename_all = "PascalCase")]
pub enum ToolInput {
    Bash(BashInput),
    Read(ReadInput),
    Grep(GrepInput),
    Glob(GlobInput),
    WebFetch(WebFetchInput),
    Edit(EditInput),
    Write(WriteInput),
    #[strum(disabled)]
    #[serde(untagged)]
    Other(OtherInput),
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct OtherInput {
    pub tool_name: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct BashInput {
    pub command: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ReadInput {
    /// Claude Code sends this as `file_path`, not `path`.
    pub file_path: String,
    pub offset: Option<u64>,
    pub limit: Option<u64>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GrepInput {
    pub pattern: String,
    /// Search root; defaults to cwd when absent.
    pub path: Option<String>,
    /// Glob filter applied to the search root.
    pub glob: Option<String>,
    /// `"content"` or `"files"`.
    pub output_mode: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GlobInput {
    pub pattern: String,
    /// Search root; defaults to cwd when absent.
    pub path: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct WebFetchInput {
    pub url: String,
    pub prompt: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct EditInput {
    pub file_path: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct WriteInput {
    pub file_path: String,
}

pub trait HookCheck<Extra = ()> {
    fn check(&self, project_dir: &Path, env: &mut HookEnv, extra: Extra);
}

impl HookCheck<Mode> for ConfigChangeInput {
    fn check(&self, project_dir: &Path, env: &mut HookEnv, mode: Mode) {
        if Path::new(&self.cwd) != project_dir {
            env.config_block(format!(
                "tool cwd {:?} is not the project dir {:?}",
                self.cwd, project_dir
            ));
            return;
        }

        match &self.source {
            Source::ProjectSettings => hooks::settings::check(project_dir, env, mode, false),
            Source::LocalSettings => hooks::settings::check(project_dir, env, mode, true),
            Source::Other(s) => env.config_allow(format!("Not tracking ConfigChange source {s}")),
            Source::Empty => env
                .config_block("Deserialization error of \"source\" field, blocking as precaution"),
        }
    }
}

impl HookCheck for OtherInput {
    fn check(&self, _project_dir: &Path, env: &mut HookEnv, _: ()) {
        if ToolInput::VARIANTS.contains(&&self.tool_name[..]) {
            env.deny(format!(
                "malformed tool input for {} tool invocation",
                self.tool_name
            ))
        }
    }
}

impl HookCheck for BashInput {
    fn check(&self, project_dir: &Path, env: &mut HookEnv, _: ()) {
        hooks::bash::check(project_dir, env, &self.command);
    }
}

impl HookCheck for ReadInput {
    fn check(&self, project_dir: &Path, env: &mut HookEnv, _: ()) {
        hooks::paths::check(project_dir, env, Path::new(&self.file_path));
    }
}

impl HookCheck for GrepInput {
    fn check(&self, project_dir: &Path, env: &mut HookEnv, _: ()) {
        let path = self.path.as_deref().unwrap_or(".");
        hooks::paths::check(project_dir, env, Path::new(path));
    }
}

impl HookCheck for GlobInput {
    fn check(&self, project_dir: &Path, env: &mut HookEnv, _: ()) {
        let path = self.path.as_deref().unwrap_or(".");
        hooks::paths::check(project_dir, env, Path::new(path));
    }
}

impl HookCheck for WebFetchInput {
    fn check(&self, project_dir: &Path, env: &mut HookEnv, _: ()) {
        hooks::webfetch::check(project_dir, env, &self.url);
    }
}

impl HookCheck for EditInput {
    fn check(&self, project_dir: &Path, env: &mut HookEnv, _: ()) {
        hooks::write::check(project_dir, env, "Edit", Path::new(&self.file_path));
    }
}

impl HookCheck for WriteInput {
    fn check(&self, project_dir: &Path, env: &mut HookEnv, _: ()) {
        hooks::write::check(project_dir, env, "Write", Path::new(&self.file_path));
    }
}

impl HookCheck for ToolInput {
    fn check(&self, project_dir: &Path, env: &mut HookEnv, _: ()) {
        match self {
            ToolInput::Bash(i) => i.check(project_dir, env, ()),
            ToolInput::Read(i) => i.check(project_dir, env, ()),
            ToolInput::Grep(i) => i.check(project_dir, env, ()),
            ToolInput::Glob(i) => i.check(project_dir, env, ()),
            ToolInput::WebFetch(i) => i.check(project_dir, env, ()),
            ToolInput::Edit(i) => i.check(project_dir, env, ()),
            ToolInput::Write(i) => i.check(project_dir, env, ()),
            ToolInput::Other(i) => i.check(project_dir, env, ()),
        }
    }
}

impl HookCheck for PreToolUseInput {
    fn check(&self, project_dir: &Path, env: &mut HookEnv, _: ()) {
        if self.hook_event_name != "PreToolUse" {
            env.allow(format!(
                "unrelated hook event {}, allowing by default",
                self.hook_event_name
            ));
            return;
        }

        if Path::new(&self.cwd) != project_dir {
            env.deny(format!(
                "tool cwd {:?} is not the project dir {:?}",
                self.cwd, project_dir
            ));
            return;
        }

        if self.permission_mode != "dontAsk" {
            // go/no-go unimplemented at present
        }

        if let _ = self.agent_id
            && let _ = self.agent_type
        {
            // go/no-go unimplemented at present
        }

        match &self.tool_input {
            None => env.deny("No tool_name/tool_input specified"),
            Some(tool_input) => {
                tool_input.check(project_dir, env, ());
            }
        }
    }
}
