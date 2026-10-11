//! Secret storage: the OS keychain, with an owner-only file in the data dir as fallback.

use archipel_plugin_api::SecretError;
use std::collections::BTreeMap;
use std::io::Write;
use std::path::PathBuf;

/// Where secrets live.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SecretBackend {
    /// The OS keychain; when it is unavailable (no Secret Service session, locked down
    /// sandbox) the file in the data dir takes over. Reads look at both.
    #[default]
    Auto,
    /// Only the file `secrets.toml` in the data dir (mode 0600 on Unix).
    File,
}

/// The secrets of one app id.
#[derive(Debug)]
pub struct SecretStore {
    service: String,
    file: Option<PathBuf>,
    backend: SecretBackend,
}

impl SecretStore {
    pub fn new(app_id: &str, data_dir: Option<PathBuf>, backend: SecretBackend) -> Self {
        Self {
            service: app_id.to_owned(),
            file: data_dir.map(|dir| dir.join("secrets.toml")),
            backend,
        }
    }

    pub fn get(&self, key: &str) -> Result<Option<String>, SecretError> {
        if self.backend == SecretBackend::Auto {
            // Not in the keychain (or it is down): the secret may have been saved to the
            // file while the keychain was unavailable, so fall through to it.
            if let Ok(secret) = keychain(&self.service, key).and_then(|e| e.get_password()) {
                return Ok(Some(secret));
            }
        }
        Ok(self.read_file()?.remove(key))
    }

    pub fn set(&self, key: &str, value: &str) -> Result<(), SecretError> {
        if self.backend == SecretBackend::Auto
            && keychain(&self.service, key)
                .and_then(|e| e.set_password(value))
                .is_ok()
        {
            // Do not leave a stale copy in the file.
            return self.update_file(|map| map.remove(key).is_some());
        }
        self.update_file(|map| {
            map.insert(key.to_owned(), value.to_owned());
            true
        })
    }

    pub fn delete(&self, key: &str) -> Result<(), SecretError> {
        if self.backend == SecretBackend::Auto {
            // Deleting what is not there is fine; an unreachable keychain holds nothing we
            // could have stored.
            let _ = keychain(&self.service, key).and_then(|e| e.delete_credential());
        }
        self.update_file(|map| map.remove(key).is_some())
    }

    fn read_file(&self) -> Result<BTreeMap<String, String>, SecretError> {
        let Some(path) = &self.file else {
            return Ok(BTreeMap::new());
        };
        match std::fs::read_to_string(path) {
            Ok(text) => toml::from_str(&text)
                .map_err(|e| SecretError::Backend(format!("{}: {e}", path.display()))),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(BTreeMap::new()),
            Err(e) => Err(SecretError::Backend(format!("{}: {e}", path.display()))),
        }
    }

    /// Edits the file; `edit` returns whether it changed anything.
    fn update_file(
        &self,
        edit: impl FnOnce(&mut BTreeMap<String, String>) -> bool,
    ) -> Result<(), SecretError> {
        let Some(path) = &self.file else {
            return if self.backend == SecretBackend::File {
                Err(SecretError::Backend("no data directory".into()))
            } else {
                Ok(())
            };
        };
        let mut map = self.read_file()?;
        if !edit(&mut map) {
            return Ok(());
        }
        let backend = |e: std::io::Error| SecretError::Backend(format!("{}: {e}", path.display()));
        let text = toml::to_string(&map).map_err(|e| SecretError::Backend(e.to_string()))?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(backend)?;
        }
        // Write a private temp file, then rename: the secrets are never world-readable.
        let tmp = path.with_extension("toml.tmp");
        let mut file = open_private(&tmp).map_err(backend)?;
        file.write_all(text.as_bytes()).map_err(backend)?;
        file.sync_all().map_err(backend)?;
        std::fs::rename(&tmp, path).map_err(backend)
    }
}

fn keychain(service: &str, key: &str) -> keyring::Result<keyring::Entry> {
    keyring::Entry::new(service, key)
}

#[cfg(unix)]
fn open_private(path: &std::path::Path) -> std::io::Result<std::fs::File> {
    use std::os::unix::fs::OpenOptionsExt;
    std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(path)
}

#[cfg(not(unix))]
fn open_private(path: &std::path::Path) -> std::io::Result<std::fs::File> {
    // The per-user data directory (%APPDATA%) is already private to the user.
    std::fs::File::create(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_backend_round_trips_with_owner_only_permissions() {
        let dir = std::env::temp_dir().join(format!("archipel-secrets-{}", std::process::id()));
        let store = SecretStore::new("x.y", Some(dir.clone()), SecretBackend::File);
        assert_eq!(store.get("token").unwrap(), None);
        store.set("token", "s3cret").unwrap();
        store.set("other", "x").unwrap();
        assert_eq!(store.get("token").unwrap().as_deref(), Some("s3cret"));
        store.delete("token").unwrap();
        store.delete("token").unwrap();
        assert_eq!(store.get("token").unwrap(), None);
        assert_eq!(store.get("other").unwrap().as_deref(), Some("x"));
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(dir.join("secrets.toml"))
                .unwrap()
                .permissions()
                .mode();
            assert_eq!(mode & 0o777, 0o600);
        }
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn file_backend_without_a_data_dir_reports_an_error() {
        let store = SecretStore::new("x.y", None, SecretBackend::File);
        assert!(store.set("k", "v").is_err());
        assert_eq!(store.get("k").unwrap(), None);
    }
}
