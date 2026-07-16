use std::{env, path::{Path, PathBuf}};

/// Returns `$CLAUDE_PROJECT_DIR` if set, otherwise the current working directory.
pub fn project_dir() -> PathBuf {
    if let Ok(d) = env::var("CLAUDE_PROJECT_DIR") {
        PathBuf::from(d)
    } else {
        env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
    }
}

/// Resolve `path` relative to `root`, then verify it does not escape `root`
/// via `..` components. Returns an error string on failure.
pub fn resolve_safe(root: &Path, path: &str) -> Result<PathBuf, String> {
    let candidate = if Path::new(path).is_absolute() {
        PathBuf::from(path)
    } else {
        root.join(path)
    };

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

    if !out.starts_with(root) {
        return Err(format!("path {path:?} escapes the project root"));
    }

    Ok(out)
}
