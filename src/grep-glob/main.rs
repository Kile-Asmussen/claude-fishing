use std::{
    env, fs,
    io::Read as _,
    path::{Path, PathBuf},
};

use globset::{GlobBuilder, GlobSet, GlobSetBuilder};
use regex::Regex;
use rmcp::{
    ServerHandler,
    handler::server::wrapper::Parameters,
    model::{CallToolResult, ContentBlock, Implementation, ServerInfo},
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

fn project_dir() -> PathBuf {
    if let Ok(d) = env::var("CLAUDE_PROJECT_DIR") {
        PathBuf::from(d)
    } else {
        env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
    }
}

/// Resolve `path` relative to the project root, then verify it does not
/// escape the root via `..` components.  Returns an error string on failure.
fn resolve_safe(root: &Path, path: &str) -> Result<PathBuf, String> {
    let candidate = if Path::new(path).is_absolute() {
        PathBuf::from(path)
    } else {
        root.join(path)
    };

    // Lexically canonicalise (collapse `.` and `..` without hitting the FS).
    let mut out = PathBuf::new();
    for component in candidate.components() {
        match component {
            std::path::Component::ParentDir => {
                if !out.pop() {
                    return Err(format!("path {path:?} escapes the project root"));
                }
            }
            c => out.push(c),
        }
    }

    // Must still be under root after normalisation.
    if !out.starts_with(root) {
        return Err(format!("path {path:?} escapes the project root"));
    }

    Ok(out)
}

/// Build a GlobSet from a single glob string.
fn compile_glob(pattern: &str) -> Result<GlobSet, String> {
    let glob = GlobBuilder::new(pattern)
        .literal_separator(true)
        .build()
        .map_err(|e| format!("invalid glob {pattern:?}: {e}"))?;
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
}

// ---------------------------------------------------------------------------
// Server struct
// ---------------------------------------------------------------------------

#[derive(Clone)]
struct GrepGlobMcp;

#[tool_router]
impl GrepGlobMcp {
    /// List files under a directory whose paths match a glob pattern.
    /// Returns one relative path per line, sorted lexicographically.
    #[tool(description = "List files matching a glob pattern under a directory.")]
    fn glob(&self, Parameters(params): Parameters<GlobParams>) -> CallToolResult {
        let root = project_dir();
        let search_root = match params.path.as_deref() {
            Some(p) => match resolve_safe(&root, p) {
                Ok(r) => r,
                Err(e) => return CallToolResult::error(vec![ContentBlock::text(e)]),
            },
            None => root.clone(),
        };

        let pattern = params.pattern.as_deref().unwrap_or("**/*");
        let globset = match compile_glob(pattern) {
            Ok(g) => g,
            Err(e) => return CallToolResult::error(vec![ContentBlock::text(e)]),
        };

        let mut matches: Vec<String> = WalkDir::new(&search_root)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
            .filter_map(|e| {
                let rel = e.path().strip_prefix(&search_root).ok()?;
                if globset.is_match(rel) {
                    Some(rel.to_string_lossy().into_owned())
                } else {
                    None
                }
            })
            .collect();

        matches.sort();
        CallToolResult::success(vec![ContentBlock::text(matches.join("\n"))])
    }

    /// Search file contents for a regex pattern.
    /// Returns one matching file path per line (not the matched lines themselves).
    /// Optionally restricts the search to files whose paths match a glob.
    #[tool(description = "Search file contents for a regex and return matching file paths.")]
    fn grep(&self, Parameters(params): Parameters<GrepParams>) -> CallToolResult {
        let root = project_dir();
        let search_root = match params.path.as_deref() {
            Some(p) => match resolve_safe(&root, p) {
                Ok(r) => r,
                Err(e) => return CallToolResult::error(vec![ContentBlock::text(e)]),
            },
            None => root.clone(),
        };

        let re = match Regex::new(&params.pattern) {
            Ok(r) => r,
            Err(e) => {
                return CallToolResult::error(vec![ContentBlock::text(format!(
                    "invalid regex {:?}: {e}",
                    params.pattern
                ))]);
            }
        };

        let include_glob = match params.include.as_deref() {
            Some(g) => match compile_glob(g) {
                Ok(gs) => Some(gs),
                Err(e) => return CallToolResult::error(vec![ContentBlock::text(e)]),
            },
            None => None,
        };

        let mut matches: Vec<String> = WalkDir::new(&search_root)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
            .filter_map(|e| {
                let rel = e.path().strip_prefix(&search_root).ok()?;

                if let Some(gs) = &include_glob {
                    if !gs.is_match(rel) {
                        return None;
                    }
                }

                if is_binary(e.path()) {
                    return None;
                }

                let contents = fs::read_to_string(e.path()).ok()?;
                if re.is_match(&contents) {
                    Some(rel.to_string_lossy().into_owned())
                } else {
                    None
                }
            })
            .collect();

        matches.sort();
        CallToolResult::success(vec![ContentBlock::text(matches.join("\n"))])
    }
}

#[tool_handler]
impl ServerHandler for GrepGlobMcp {
    fn get_info(&self) -> ServerInfo {
        let mut info = ServerInfo::default();
        info.server_info = Implementation::new("grep-glob-mcp", env!("CARGO_PKG_VERSION"));
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
