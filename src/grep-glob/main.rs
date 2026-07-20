use std::{fs, io::Read as _, path::Path};

use chrono::Local;
use claude_fishing::hooks::env::HookEnv;
use claude_fishing::hooks::glob_exclude;
use claude_fishing::hooks::paths::is_path_allowed;
use claude_fishing::util::{project_dir, resolve_safe};
use globset::{GlobBuilder, GlobSet, GlobSetBuilder};
use regex::Regex;
use rmcp::{
    ServerHandler,
    handler::server::wrapper::Parameters,
    model::{CallToolResult, ContentBlock, Implementation, ServerCapabilities, ServerInfo},
    schemars,
    service::serve_server,
    tool, tool_handler, tool_router,
    transport::io::stdio,
};
use serde::Deserialize;
use walkdir::WalkDir;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn check_path_allowed(paths_cfg: &str, root: &Path, path: &Path) -> Result<(), String> {
    match is_path_allowed(paths_cfg, root, path) {
        Ok(true) => Ok(()),
        Ok(false) => Err(format!(
            "path {path:?} is not permitted by .claude/paths (project root: {root:?})\n\
             pattern syntax: prefix / for absolute paths, ./ or no prefix for paths relative to the project root\n\
             ask the user to add a matching pattern to .claude/paths or .claude/paths-local"
        )),
        Err(e) => Err(e),
    }
}

/// Build a GlobSet from a single glob string.
fn compile_glob(pattern: &str) -> Result<GlobSet, String> {
    let glob = GlobBuilder::new(pattern)
        .literal_separator(true)
        .build()
        .map_err(|e| {
            format!(
                "invalid glob pattern {pattern:?}: {e}\n\
                               use ** to match across path separators, * for a single segment"
            )
        })?;
    let mut builder = GlobSetBuilder::new();
    builder.add(glob);
    builder
        .build()
        .map_err(|e| format!("glob build error: {e}"))
}

/// Returns `true` if the first 8 KiB of `path` contains a null byte.
fn is_binary(path: &Path) -> bool {
    let Ok(mut f) = fs::File::open(path) else {
        return false;
    };
    let mut buf = [0u8; 8192];
    let n = f.read(&mut buf).unwrap_or(0);
    buf[..n].contains(&0u8)
}

// ---------------------------------------------------------------------------
// Tool parameter structs
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct GlobParams {
    /// Glob pattern to match against relative file paths (default: `**/*`)
    pattern: Option<String>,
    /// Directory to search within, relative to the project root (default: project root)
    path: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct GrepParams {
    /// Regex pattern to search for inside file contents (required)
    pattern: String,
    /// Glob filter applied to file paths before searching (optional)
    include: Option<String>,
    /// Directory to search within, relative to the project root (default: project root)
    path: Option<String>,
    /// When true, output matched lines as `path:lineno: content` instead of just file paths
    include_lines: Option<bool>,
}

// ---------------------------------------------------------------------------
// Server struct
// ---------------------------------------------------------------------------

fn make_env() -> HookEnv {
    let root = project_dir();
    let claude = root.join(".claude");
    let log_path = Some(claude.join("fishing.log"));
    HookEnv::from_claude_dir(&claude, log_path)
}

#[derive(Clone)]
struct GrepGlobMcp;

#[tool_router]
impl GrepGlobMcp {
    /// List files under a directory whose paths match a glob pattern.
    /// Returns one relative path per line, sorted lexicographically.
    /// Hidden directories and paths listed in .claude/glob-exclude are skipped.
    /// Only files permitted by .claude/paths are returned.
    #[tool(description = "List files matching a glob pattern under a directory.")]
    fn glob(&self, Parameters(params): Parameters<GlobParams>) -> CallToolResult {
        let root = project_dir();
        let mut env = make_env();
        env.log(format!(
            "[{}] glob pattern={:?} path={:?}",
            Local::now().format("%Y-%m-%d %H:%M:%S"),
            params.pattern,
            params.path,
        ));

        let result: Result<String, String> = (|| {
            let exclude_cfg = env.glob_exclude_config().unwrap_or_default();
            let search_root = match params.path.as_deref() {
                Some(p) => resolve_safe(&root, p).map_err(|e| {
                    let msg = format!("{e}\nthe `path` parameter must be a relative path inside the project root {root:?}");
                    env.log(format!("ERROR: {msg}")); msg
                })?,
                None => root.clone(),
            };

            let pattern = params.pattern.as_deref().unwrap_or("**/*");
            let globset = compile_glob(pattern).map_err(|e| {
                env.log(format!("ERROR: {e}"));
                e
            })?;

            let mut matches: Vec<String> = WalkDir::new(&search_root)
                .into_iter()
                .filter_entry(|e| {
                    if e.file_type().is_dir() {
                        !glob_exclude::is_excluded(&exclude_cfg, &root, e.path())
                    } else {
                        true
                    }
                })
                .filter_map(|e| e.ok())
                .filter(|e| e.file_type().is_file())
                .filter_map(|e| {
                    let rel = e.path().strip_prefix(&search_root).ok()?;
                    if !globset.is_match(rel) {
                        return None;
                    }
                    Some(rel.to_string_lossy().into_owned())
                })
                .collect();

            matches.sort();
            env.log(format!("glob: {} result(s)", matches.len()));
            Ok(matches.join("\n"))
        })();

        env.flush();
        match result {
            Ok(text) => CallToolResult::success(vec![ContentBlock::text(text)]),
            Err(e) => CallToolResult::error(vec![ContentBlock::text(e)]),
        }
    }

    /// Search file contents for a regex pattern.
    /// Hidden directories and paths listed in .claude/glob-exclude are skipped.
    /// Only searches files permitted by .claude/paths.
    /// By default returns one matching file path per line; set include_lines=true
    /// to get `path:lineno: content` output for every matched line.
    #[tool(
        description = "Search file contents for a regex. Returns matching file paths, or matched lines when include_lines is true."
    )]
    fn grep(&self, Parameters(params): Parameters<GrepParams>) -> CallToolResult {
        let root = project_dir();
        let mut env = make_env();
        env.log(format!(
            "[{}] grep pattern={:?} include={:?} path={:?} include_lines={:?}",
            Local::now().format("%Y-%m-%d %H:%M:%S"),
            params.pattern,
            params.include,
            params.path,
            params.include_lines,
        ));

        let result: Result<String, String> = (|| {
            let paths_cfg = env.paths_config().map_err(|e| {
                env.log(format!("ERROR: {e}"));
                e
            })?;
            let exclude_cfg = env.glob_exclude_config().unwrap_or_default();
            let search_root = match params.path.as_deref() {
                Some(p) => resolve_safe(&root, p).map_err(|e| {
                    let msg = format!("{e}\nthe `path` parameter must be a relative path inside the project root {root:?}");
                    env.log(format!("ERROR: {msg}")); msg
                })?,
                None => root.clone(),
            };

            let re = Regex::new(&params.pattern).map_err(|e| {
                let msg = format!(
                    "invalid regex in `pattern` parameter {:?}: {e}\n\
                     use a Rust-syntax regex; note that lookaheads are not supported",
                    params.pattern
                );
                env.log(format!("ERROR: {msg}"));
                msg
            })?;

            let include_glob = match params.include.as_deref() {
                Some(g) => Some(compile_glob(g).map_err(|e| {
                    env.log(format!("ERROR: {e}"));
                    e
                })?),
                None => None,
            };

            let include_lines = params.include_lines.unwrap_or(false);

            let mut output: Vec<String> = WalkDir::new(&search_root)
                .into_iter()
                .filter_entry(|e| {
                    if e.file_type().is_dir() {
                        !glob_exclude::is_excluded(&exclude_cfg, &root, e.path())
                    } else {
                        true
                    }
                })
                .filter_map(|e| e.ok())
                .filter(|e| e.file_type().is_file())
                .filter_map(|e| {
                    let rel = e.path().strip_prefix(&search_root).ok()?;

                    if let Some(gs) = &include_glob {
                        if !gs.is_match(rel) {
                            return None;
                        }
                    }

                    check_path_allowed(&paths_cfg, &root, e.path()).ok()?;

                    if is_binary(e.path()) {
                        return None;
                    }

                    let contents = fs::read_to_string(e.path()).ok()?;
                    let rel_str = rel.to_string_lossy();

                    if include_lines {
                        let lines: Vec<String> = contents
                            .lines()
                            .enumerate()
                            .filter_map(|(i, line)| {
                                if re.is_match(line) {
                                    Some(format!("{}:{}: {}", rel_str, i + 1, line))
                                } else {
                                    None
                                }
                            })
                            .collect();
                        if lines.is_empty() {
                            None
                        } else {
                            Some(lines.join("\n"))
                        }
                    } else if re.is_match(&contents) {
                        Some(rel_str.into_owned())
                    } else {
                        None
                    }
                })
                .collect();

            output.sort();
            env.log(format!("grep: {} result(s)", output.len()));
            Ok(output.join("\n"))
        })();

        env.flush();
        match result {
            Ok(text) => CallToolResult::success(vec![ContentBlock::text(text)]),
            Err(e) => CallToolResult::error(vec![ContentBlock::text(e)]),
        }
    }
}

#[tool_handler]
impl ServerHandler for GrepGlobMcp {
    fn get_info(&self) -> ServerInfo {
        let mut info = ServerInfo::default();
        info.server_info = Implementation::new("fishing-grep-glob-mcp", env!("CARGO_PKG_VERSION"));
        info.capabilities = ServerCapabilities::builder().enable_tools().build();
        info
    }
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let service = serve_server(GrepGlobMcp, stdio()).await?;
    service.waiting().await?;
    Ok(())
}
