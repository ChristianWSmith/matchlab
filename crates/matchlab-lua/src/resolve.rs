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
}
