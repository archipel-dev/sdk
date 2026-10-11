//! Per-app directories.

use directories::ProjectDirs;
use std::path::{Path, PathBuf};

/// The data, config and cache directories of one app id. They are not created until used.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppDirs {
    /// Plugin data (`PluginHost::data_dir`), secrets file fallback.
    pub data: PathBuf,
    /// `settings.toml`, `keymap.toml`.
    pub config: PathBuf,
    /// Disposable files.
    pub cache: PathBuf,
}

impl AppDirs {
    /// The platform directories of `app_id`: `~/.local/share/<id>` and friends on Linux,
    /// `~/Library/Application Support/dev.Archipel.<id>` on macOS, `%APPDATA%\Archipel\<id>`
    /// on Windows. `None` when the home directory is unknown.
    pub fn for_app(app_id: &str) -> Option<Self> {
        let dirs = ProjectDirs::from("dev", "Archipel", app_id)?;
        Some(Self {
            data: dirs.data_dir().to_owned(),
            config: dirs.config_dir().to_owned(),
            cache: dirs.cache_dir().to_owned(),
        })
    }

    /// Everything below `root` (`root/data`, `root/config`, `root/cache`): portable installs
    /// and tests.
    pub fn under(root: impl AsRef<Path>) -> Self {
        let root = root.as_ref();
        Self {
            data: root.join("data"),
            config: root.join("config"),
            cache: root.join("cache"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn directories_are_separate_and_named_after_the_app() {
        let dirs = AppDirs::under("/x");
        assert_eq!(dirs.data, PathBuf::from("/x/data"));
        assert_ne!(dirs.config, dirs.cache);
        if let Some(dirs) = AppDirs::for_app("dev.archipel.hello") {
            assert!(dirs.data.to_string_lossy().contains("dev.archipel.hello"));
            assert_ne!(dirs.config, dirs.cache);
        }
    }
}
