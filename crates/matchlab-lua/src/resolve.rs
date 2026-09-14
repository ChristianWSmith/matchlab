//! Workspace-root path resolution for `plugins/...` script paths.
//!
//! The CLI runs from the workspace root, so a relative path works as-is. Crate
//! unit tests run from crate directories, so we walk up to the workspace root
//! (the first ancestor `Cargo.toml` declaring `[workspace]`) and resolve there.
use std::collections::BTreeMap;
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
    let available = list_plugins_in_dir(plugin_dir);
    Err(format!(
        "cannot find plugin '{input}' (tried as path and as {plugin_dir}/{input}.lua). Available in {plugin_dir}: {available:?}"
    ))
}

const KNOWN_PLUGIN_DIRS: &[&str] = &[
    "plugins/rating",
    "plugins/game",
    "plugins/matchmaking",
    "plugins/metrics",
    "plugins/detection",
    "plugins/ranking",
    "plugins/adversarial",
    "plugins/utility",
];

/// List available plugin names across all known subsystem directories.
///
/// Returns a map from subsystem directory (e.g. `"plugins/rating"`) to a
/// sorted list of plugin names (file stems without `.lua`). Useful for help
/// output and error messages.
pub fn list_plugins() -> BTreeMap<String, Vec<String>> {
    let mut result = BTreeMap::new();
    for dir in KNOWN_PLUGIN_DIRS {
        let names = list_plugins_in_dir(dir);
        if !names.is_empty() {
            result.insert(dir.to_string(), names);
        }
    }
    result
}

fn list_plugins_in_dir(plugin_dir: &str) -> Vec<String> {
    let dir = resolve_script_path(plugin_dir);
    if !dir.is_dir() {
        return Vec::new();
    }
    let mut names: Vec<String> = std::fs::read_dir(&dir)
        .into_iter()
        .flatten()
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.path()
                .extension()
                .map(|ext| ext == "lua")
                .unwrap_or(false)
        })
        .filter_map(|e| {
            e.path()
                .file_stem()
                .map(|s| s.to_string_lossy().into_owned())
        })
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
        assert!(
            resolved
                .to_string_lossy()
                .contains("plugins/rating/elo.lua")
        );
    }
    #[test]
    fn resolve_plugin_unknown_returns_error() {
        let err = resolve_plugin("bogus", "plugins/rating").unwrap_err();
        assert!(err.contains("bogus"));
        assert!(err.contains("plugins/rating/bogus.lua"));
    }
    #[test]
    fn list_plugins_covers_all_subsystems() {
        let all = list_plugins();
        assert!(all.contains_key("plugins/rating"));
        let rating = &all["plugins/rating"];
        assert!(rating.contains(&"elo".to_string()));
        assert!(rating.contains(&"glicko2".to_string()));
        assert!(rating.contains(&"trueskill".to_string()));
        assert!(all.contains_key("plugins/game"));
        assert!(all.contains_key("plugins/matchmaking"));
        assert!(all.contains_key("plugins/metrics"));
        assert!(all.contains_key("plugins/detection"));
        assert!(all.contains_key("plugins/ranking"));
        assert!(all.contains_key("plugins/adversarial"));
        assert!(all.contains_key("plugins/utility"));
    }
}
