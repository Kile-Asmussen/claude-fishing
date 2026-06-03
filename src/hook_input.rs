use std::path::Path;
use serde::{Deserialize, Serialize};

use crate::hooks;
use crate::hooks::env::HookEnv;

/// Top-level JSON object received on stdin for every PreToolUse hook invocation.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct HookInput {
    pub session_id: Option<String>,
    pub transcript_path: Option<String>,
    pub cwd: Option<String>,
    pub hook_event_name: Option<String>,
    pub permission_mode: Option<String>,
    pub agent_id: Option<String>,
    pub agent_type: Option<String>,

    pub tool_name: Option<String>,
    pub tool_input: Option<ToolInput>,
}

/// Per-tool input payloads. Unknown tools are captured by `Other`.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(tag = "tool_name", content = "tool_input", rename_all = "PascalCase")]
pub enum ToolInput {
    Bash(BashInput),
    Read(ReadInput),
    Grep(GrepInput),
    Glob(GlobInput),
    WebFetch(WebFetchInput),
    Edit(EditInput),
    Write(WriteInput),
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

pub trait HookCheck {
    fn check(&self, project_dir: &Path, env: &mut HookEnv);
}

impl HookCheck for BashInput {
    fn check(&self, project_dir: &Path, env: &mut HookEnv) {
        hooks::bash::check(project_dir, env, &self.command);
    }
}

impl HookCheck for ReadInput {
    fn check(&self, project_dir: &Path, env: &mut HookEnv) {
        hooks::paths::check(project_dir, env, Path::new(&self.file_path));
    }
}

impl HookCheck for GrepInput {
    fn check(&self, project_dir: &Path, env: &mut HookEnv) {
        let path = self.path.as_deref().unwrap_or(".");
        hooks::paths::check(project_dir, env, Path::new(path));
    }
}

impl HookCheck for GlobInput {
    fn check(&self, project_dir: &Path, env: &mut HookEnv) {
        let path = self.path.as_deref().unwrap_or(".");
        hooks::paths::check(project_dir, env, Path::new(path));
    }
}

impl HookCheck for WebFetchInput {
    fn check(&self, project_dir: &Path, env: &mut HookEnv) {
        hooks::webfetch::check(project_dir, env, &self.url);
    }
}

impl HookCheck for EditInput {
    fn check(&self, project_dir: &Path, env: &mut HookEnv) {
        hooks::write::check(project_dir, env, "Edit", Path::new(&self.file_path));
    }
}

impl HookCheck for WriteInput {
    fn check(&self, project_dir: &Path, env: &mut HookEnv) {
        hooks::write::check(project_dir, env, "Write", Path::new(&self.file_path));
    }
}

impl HookCheck for ToolInput {
    fn check(&self, project_dir: &Path, env: &mut HookEnv) {
        match self {
            ToolInput::Bash(i)     => i.check(project_dir, env),
            ToolInput::Read(i)     => i.check(project_dir, env),
            ToolInput::Grep(i)     => i.check(project_dir, env),
            ToolInput::Glob(i)     => i.check(project_dir, env),
            ToolInput::WebFetch(i) => i.check(project_dir, env),
            ToolInput::Edit(i)     => i.check(project_dir, env),
            ToolInput::Write(i)    => i.check(project_dir, env),
        }
    }
}

