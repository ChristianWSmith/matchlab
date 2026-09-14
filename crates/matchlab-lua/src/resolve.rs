//! Workspace-root path resolution for `plugins/...` script paths.
//!
//! The CLI runs from the workspace root, so a relative path works as-is. Crate
//! unit tests run from crate directories, so we walk up to the workspace root
//! (the first ancestor `Cargo.toml` declaring `[workspace]`) and resolve there.
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
/// The workspace root (cached): the nearest ancestor of the current directory
/// whose `Cargo.toml` declares `[workspace]`.
pub fn workspace_root() -> PathBuf {
    static ROOT: OnceLock<PathBuf> = OnceLock::new();
    ROOT.get_or_init(|| {
        if let Ok(cwd) = std::env::current_dir() {
            let mut dir = Some(cwd.as_path());
            while let Some(d) = dir {
                if is_workspace_root(d) {
                    return d.to_path_buf();
                }
                dir = d.parent();
            }
        }
        PathBuf::from(".")
    })
    .clone()
}
fn is_workspace_root(dir: &Path) -> bool {
    let manifest = dir.join("Cargo.toml");
    if !manifest.is_file() {
        return false;
    }
    std::fs::read_to_string(&manifest)
        .map(|text| text.contains("[workspace]"))
        .unwrap_or(false)
}
/// Resolve a script path to an absolute path.
///
/// 1. Absolute paths and paths that exist relative to the current directory
///    are used as-is.
/// 2. Paths relative to `CARGO_MANIFEST_DIR` (crate test dirs) are tried.
/// 3. Otherwise the path is resolved against the workspace root.
///
/// A missing script still resolves to a path (the read error will be
/// descriptive); nothing here panics.
pub fn resolve_script_path(path: &str) -> PathBuf {
    let p = Path::new(path);
    if p.is_absolute() || p.exists() {
        return p.to_path_buf();
    }
    if let Ok(manifest_dir) = std::env::var("CARGO_MANIFEST_DIR") {
        let from_manifest = Path::new(&manifest_dir).join(path);
        if from_manifest.exists() {
            return from_manifest;
        }
    }
    workspace_root().join(path)
}

/// Resolve a plugin by name or path within a subsystem's plugin directory.
///
/// Resolution strategy:
/// 1. Try `input` as a direct script path (works for full paths like
///    `plugins/rating/elo.lua`).
/// 2. Try `{plugin_dir}/{input}.lua` (works for bare names like `elo`).
///
/// Returns the first path that exists on disk, or an error listing what was
/// tried.
pub fn resolve_plugin(input: &str, plugin_dir: &str) -> Result<PathBuf, String> {
    let as_path = resolve_script_path(input);
    if as_path.exists() {
        return Ok(as_path);
    }
    let named = resolve_script_path(&format!("{plugin_dir}/{input}.lua"));
    if named.exists() {
        return Ok(named);
    }
    Err(format!(
        "cannot find plugin '{input}' (tried as path and as {plugin_dir}/{input}.lua)"
    ))
}

/// List available plugin names in a subsystem's plugin directory.
///
/// Globs `{plugin_dir}/*.lua` under the workspace root and returns the file
/// stems (e.g. `["elo", "glicko2", ...]`). Useful for error messages and
/// help output.
pub fn list_plugins(plugin_dir: &str) -> Vec<String> {
    let dir = resolve_script_path(plugin_dir);
    if !dir.is_dir() {
        return Vec::new();
    }
    let mut names: Vec<String> = std::fs::read_dir(&dir)
        .into_iter()
        .flatten()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().map(|ext| ext == "lua").unwrap_or(false))
        .filter_map(|e| e.path().file_stem().map(|s| s.to_string_lossy().into_owned()))
        .collect();
    names.sort();
    names
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn workspace_root_is_found() {
        let root = workspace_root();
        assert!(
            root.join("Cargo.toml").is_file(),
            "workspace root must contain a Cargo.toml"
        );
        let text = std::fs::read_to_string(root.join("Cargo.toml")).unwrap();
        assert!(text.contains("[workspace]"));
    }
    #[test]
    fn nonexistent_path_resolves_to_workspace_root() {
        let resolved = resolve_script_path("plugins/nonexistent/script.lua");
        assert!(resolved.ends_with("plugins/nonexistent/script.lua"));
    }
    #[test]
    fn resolve_plugin_by_path() {
        let resolved = resolve_plugin("plugins/rating/elo.lua", "plugins/rating").unwrap();
        assert!(resolved.ends_with("plugins/rating/elo.lua"));
    }
    #[test]
    fn resolve_plugin_by_name() {
        let resolved = resolve_plugin("elo", "plugins/rating").unwrap();
        assert!(resolved.ends_with("plugins/rating/elo.lua"));
    }
    #[test]
    fn resolve_plugin_prefers_path_over_name() {
        let resolved = resolve_plugin("plugins/rating/elo.lua", "plugins/rating").unwrap();
        assert!(resolved.to_string_lossy().contains("plugins/rating/elo.lua"));
    }
    #[test]
    fn resolve_plugin_unknown_returns_error() {
        let err = resolve_plugin("bogus", "plugins/rating").unwrap_err();
        assert!(err.contains("bogus"));
        assert!(err.contains("plugins/rating/bogus.lua"));
    }
    #[test]
    fn list_plugins_rating() {
        let names = list_plugins("plugins/rating");
        assert!(names.contains(&"elo".to_string()));
        assert!(names.contains(&"glicko2".to_string()));
        assert!(names.contains(&"trueskill".to_string()));
    }
    #[test]
    fn list_plugins_nonexistent_dir() {
        let names = list_plugins("plugins/nonexistent");
        assert!(names.is_empty());
    }
}
