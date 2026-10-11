//! The fake host.

pub use archipel_plugin_api::testing::RecordingHost;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};

static NEXT: AtomicU32 = AtomicU32::new(0);

/// A temporary data directory that is removed on drop: give it to a [`RecordingHost`]
/// with [`DataDir::attach`] to test plugins that store files.
#[derive(Debug)]
pub struct DataDir {
    path: PathBuf,
}

impl DataDir {
    /// Creates an empty, unique directory under the system temp directory.
    pub fn create() -> std::io::Result<Self> {
        let path = std::env::temp_dir().join(format!(
            "archipel-plugin-test-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&path)?;
        Ok(Self { path })
    }

    /// The directory.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Makes `host.data_dir()` return this directory.
    pub fn attach(&self, host: &mut RecordingHost) {
        host.data = Some(self.path.clone());
    }
}

impl Drop for DataDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use archipel_plugin_api::{Notification, PluginHost, json};

    #[test]
    fn records_what_the_plugin_asks() {
        let mut host = RecordingHost::new("com.example.t");
        host.dispatch("a.b", json!({"x": 1}));
        host.notify(Notification::info("T", "B"));
        host.show_status("s");
        assert_eq!(host.dispatched[0].0, "a.b");
        assert_eq!(host.notifications.len(), 1);
        assert_eq!(host.statuses, ["s"]);
        assert!(host.data_dir().is_err(), "no data dir unless attached");
    }

    #[test]
    fn data_dir_is_unique_and_removed() {
        let (a, b) = (DataDir::create().unwrap(), DataDir::create().unwrap());
        assert_ne!(a.path(), b.path());
        let mut host = RecordingHost::new("com.example.t");
        a.attach(&mut host);
        let path = host.data_dir().unwrap();
        assert!(path.is_dir());
        drop(a);
        assert!(!path.exists());
    }
}
