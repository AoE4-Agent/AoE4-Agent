//! Where config and cache files live.
//!
//! Steam's working directory for a launch is the game install folder, and a
//! double-clicked `.bat` has yet another current directory. Relative config
//! and cache paths are therefore resolved against the directory that contains
//! the executable, not against that current directory.

use crate::{Error, Result};
use std::path::{Path, PathBuf};

/// Directory containing the running executable.
///
/// `current_exe` is canonicalized when the platform allows it, so a symlink
/// still points at the real folder beside the binary.
pub fn exe_dir() -> Result<PathBuf> {
    let exe = std::env::current_exe().map_err(|source| Error::io("current_exe", source))?;
    let exe = std::fs::canonicalize(&exe).unwrap_or(exe);
    exe.parent()
        .filter(|dir| !dir.as_os_str().is_empty())
        .map(Path::to_path_buf)
        .ok_or_else(|| Error::Config("could not locate the directory of this executable".into()))
}

/// A path is rooted when the OS considers it absolute, or when it is a
/// Windows drive path (`C:\...`, `C:/...`) or a UNC path. The Windows forms
/// are recognized on every host so a copied config does not get joined onto
/// the exe directory.
pub fn is_rooted(path: &Path) -> bool {
    if path.is_absolute() {
        return true;
    }
    let text = path.to_string_lossy();
    let bytes = text.as_bytes();
    if bytes.len() >= 3
        && bytes[0].is_ascii_alphabetic()
        && bytes[1] == b':'
        && (bytes[2] == b'\\' || bytes[2] == b'/')
    {
        return true;
    }
    text.starts_with(r"\\") || text.starts_with("//")
}

/// Join `path` onto `base` when `path` is relative.
pub fn resolve_against(base: &Path, path: &Path) -> PathBuf {
    if is_rooted(path) {
        path.to_path_buf()
    } else {
        base.join(path)
    }
}

/// Prefer `exe_dir/requested`. If that file is missing and `requested` is
/// relative, also try the process current directory. The exe directory wins
/// when both exist, which is what a Steam launch needs: the game folder may
/// contain some other `config.yaml`, and the one next to `aoe4-agent.exe`
/// is the one that was installed with it.
pub fn resolve_config_file(exe_dir: &Path, requested: &Path) -> Result<PathBuf> {
    let cwd = std::env::current_dir().ok();
    resolve_config_file_in(exe_dir, requested, cwd.as_deref())
}

pub fn resolve_config_file_in(
    exe_dir: &Path,
    requested: &Path,
    cwd: Option<&Path>,
) -> Result<PathBuf> {
    let beside_exe = resolve_against(exe_dir, requested);
    if beside_exe.is_file() {
        return Ok(beside_exe);
    }
    if !is_rooted(requested) {
        if let Some(cwd) = cwd {
            let beside_cwd = cwd.join(requested);
            if beside_cwd.is_file() {
                return Ok(beside_cwd);
            }
            return Err(Error::Config(format!(
                "config file {} does not exist, and {} does not either. Copy config.example.yaml to config.yaml next to the executable.",
                beside_exe.display(),
                beside_cwd.display()
            )));
        }
    }
    Err(Error::Config(format!(
        "config file {} does not exist. Copy config.example.yaml to config.yaml next to the executable.",
        beside_exe.display()
    )))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn relative_paths_join_the_exe_directory_and_rooted_paths_do_not() {
        let base = Path::new("/games/Age of Empires IV/AoE4Agent");
        assert_eq!(
            resolve_against(base, Path::new("config.yaml")),
            base.join("config.yaml")
        );
        assert_eq!(
            resolve_against(base, Path::new("cache")),
            base.join("cache")
        );
        assert_eq!(
            resolve_against(base, Path::new("/var/aoe4/config.yaml")),
            Path::new("/var/aoe4/config.yaml")
        );
        let windows = Path::new(r"C:\AoE4Agent\config.yaml");
        assert!(is_rooted(windows));
        assert_eq!(resolve_against(base, windows), windows);
        assert!(is_rooted(Path::new(r"\\server\share\config.yaml")));
    }

    #[test]
    fn config_lookup_prefers_the_exe_directory() {
        let root = std::env::temp_dir().join(format!(
            "aoe4-agent-paths-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let exe_dir = root.join("exe");
        let cwd = root.join("cwd");
        fs::create_dir_all(&exe_dir).unwrap();
        fs::create_dir_all(&cwd).unwrap();
        fs::write(exe_dir.join("config.yaml"), "exe").unwrap();
        fs::write(cwd.join("config.yaml"), "cwd").unwrap();

        let found = resolve_config_file_in(&exe_dir, Path::new("config.yaml"), Some(&cwd)).unwrap();
        assert_eq!(found, exe_dir.join("config.yaml"));

        fs::remove_file(exe_dir.join("config.yaml")).unwrap();
        let found = resolve_config_file_in(&exe_dir, Path::new("config.yaml"), Some(&cwd)).unwrap();
        assert_eq!(found, cwd.join("config.yaml"));

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn exe_dir_is_an_absolute_directory() {
        let dir = exe_dir().unwrap();
        assert!(dir.is_absolute());
        assert!(dir.is_dir());
    }
}
