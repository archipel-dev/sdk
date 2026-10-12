//! [`AppHost`]: the [`PluginHost`] of a standalone app.

use crate::dirs::AppDirs;
use crate::secrets::{SecretBackend, SecretStore};
use crate::settings::SettingsStore;
use archipel_plugin_api::{
    EditorTabRequest, Notification, NotificationLevel, Permission, PluginError, PluginHost,
    PluginManifest, SecretError, SettingsPage, TaskId, TaskProgress, Value,
};
use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

/// A notification on screen, shown as a kit Toast.
#[derive(Debug, Clone)]
pub struct ToastEntry {
    pub id: u64,
    pub notification: Notification,
    pub created: Instant,
}

impl ToastEntry {
    /// How long the toast stays: errors linger.
    pub fn lifetime(&self) -> Duration {
        match self.notification.level {
            NotificationLevel::Error => Duration::from_secs(12),
            NotificationLevel::Warning => Duration::from_secs(8),
            NotificationLevel::Info => Duration::from_secs(5),
        }
    }
}

/// A running task, shown in the status bar.
#[derive(Debug, Clone, PartialEq)]
pub struct TaskEntry {
    pub id: TaskId,
    pub title: String,
    pub progress: TaskProgress,
}

/// What the plugin asked for during a call; the app drains it every frame.
#[derive(Debug, Default)]
pub(crate) struct Requests {
    pub commands: VecDeque<(String, Value)>,
    pub tool_windows: Vec<String>,
    pub tabs: Vec<EditorTabRequest>,
    pub contributions_changed: bool,
    pub repaint: bool,
    pub focus_app: bool,
}

/// The services a standalone app offers its plugin: per-app directories, TOML settings,
/// secrets, notifications as toasts, a status bar and tasks. Opening tool windows and editor
/// tabs and dispatching commands are queued for the [`crate::App`].
pub struct AppHost {
    id: String,
    permissions: Vec<Permission>,
    dirs: Option<AppDirs>,
    settings: SettingsStore,
    secrets: SecretStore,
    status: String,
    toasts: Vec<ToastEntry>,
    next_id: u64,
    tasks: Vec<TaskEntry>,
    pub(crate) requests: Requests,
}

impl AppHost {
    /// Builds the host of `manifest`'s plugin. `pages` is the settings schema. The second
    /// value is a problem found while loading `settings.toml`, if any.
    pub fn new(
        manifest: &PluginManifest,
        pages: &[SettingsPage],
        dirs: Option<AppDirs>,
        secret_backend: SecretBackend,
    ) -> (Self, Option<String>) {
        let (settings, problem) = SettingsStore::load(
            &manifest.id,
            dirs.as_ref().map(|d| d.config.join("settings.toml")),
            pages,
        );
        let secrets = SecretStore::new(
            &manifest.id,
            dirs.as_ref().map(|d| d.data.clone()),
            secret_backend,
        );
        let host = Self {
            id: manifest.id.clone(),
            permissions: manifest.permissions.clone(),
            dirs,
            settings,
            secrets,
            status: String::new(),
            toasts: Vec::new(),
            next_id: 0,
            tasks: Vec::new(),
            requests: Requests::default(),
        };
        (host, problem)
    }

    pub fn dirs(&self) -> Option<&AppDirs> {
        self.dirs.as_ref()
    }

    /// The app's config directory (created on demand).
    pub fn config_dir(&self) -> Result<PathBuf, PluginError> {
        self.ensure(self.dirs.as_ref().map(|d| d.config.clone()))
    }

    /// The app's cache directory (created on demand).
    pub fn cache_dir(&self) -> Result<PathBuf, PluginError> {
        self.ensure(self.dirs.as_ref().map(|d| d.cache.clone()))
    }

    fn ensure(&self, dir: Option<PathBuf>) -> Result<PathBuf, PluginError> {
        let dir =
            dir.ok_or_else(|| PluginError::Io(self.id.clone(), "no per-user directory".into()))?;
        std::fs::create_dir_all(&dir)
            .map_err(|e| PluginError::Io(self.id.clone(), format!("{}: {e}", dir.display())))?;
        Ok(dir)
    }

    pub fn status(&self) -> &str {
        &self.status
    }

    pub fn toasts(&self) -> &[ToastEntry] {
        &self.toasts
    }

    pub fn tasks(&self) -> &[TaskEntry] {
        &self.tasks
    }

    pub fn dismiss_toast(&mut self, id: u64) {
        self.toasts.retain(|toast| toast.id != id);
    }

    /// Drops toasts that outlived their lifetime; returns when the next one expires.
    pub fn expire_toasts(&mut self, now: Instant) -> Option<Duration> {
        self.toasts
            .retain(|toast| now.duration_since(toast.created) < toast.lifetime());
        self.toasts
            .iter()
            .map(|toast| {
                toast
                    .lifetime()
                    .saturating_sub(now.duration_since(toast.created))
            })
            .min()
    }

    pub(crate) fn set_pages(&mut self, pages: &[SettingsPage]) {
        self.settings.set_pages(pages);
    }

    fn check_secrets(&self) -> Result<(), SecretError> {
        if self.has_permission(Permission::Secrets) {
            Ok(())
        } else {
            Err(SecretError::PermissionDenied(self.id.clone()))
        }
    }
}

impl PluginHost for AppHost {
    fn is_standalone(&self) -> bool {
        true
    }

    fn focus_app_view(&mut self) {
        self.requests.focus_app = true;
        self.requests.repaint = true;
    }
    fn plugin_id(&self) -> &str {
        &self.id
    }

    fn project_root(&self) -> Option<&Path> {
        None
    }

    fn show_status(&mut self, message: &str) {
        self.status = message.into();
        self.requests.repaint = true;
    }

    fn notify(&mut self, notification: Notification) {
        self.next_id += 1;
        self.toasts.push(ToastEntry {
            id: self.next_id,
            notification,
            created: Instant::now(),
        });
        self.requests.repaint = true;
    }

    fn dispatch(&mut self, command: &str, params: Value) {
        self.requests.commands.push_back((command.into(), params));
        self.requests.repaint = true;
    }

    fn open_tool_window(&mut self, id: &str) {
        self.requests.tool_windows.push(id.into());
        self.requests.repaint = true;
    }

    fn open_editor_tab(&mut self, request: EditorTabRequest) {
        self.requests.tabs.push(request);
        self.requests.repaint = true;
    }

    fn setting(&self, key: &str) -> Option<Value> {
        self.settings.get(key)
    }

    fn set_setting(&mut self, key: &str, value: Value) -> Result<(), PluginError> {
        self.settings.set(key, value)?;
        self.requests.repaint = true;
        Ok(())
    }

    fn data_dir(&self) -> Result<PathBuf, PluginError> {
        self.ensure(self.dirs.as_ref().map(|d| d.data.clone()))
    }

    fn has_permission(&self, permission: Permission) -> bool {
        self.permissions.contains(&permission)
    }

    fn secret(&self, key: &str) -> Result<Option<String>, SecretError> {
        self.check_secrets()?;
        self.secrets.get(key)
    }

    fn set_secret(&mut self, key: &str, value: &str) -> Result<(), SecretError> {
        self.check_secrets()?;
        self.secrets.set(key, value)
    }

    fn delete_secret(&mut self, key: &str) -> Result<(), SecretError> {
        self.check_secrets()?;
        self.secrets.delete(key)
    }

    fn begin_task(&mut self, title: &str) -> TaskId {
        self.next_id += 1;
        let id = TaskId(self.next_id);
        self.tasks.push(TaskEntry {
            id,
            title: title.into(),
            progress: TaskProgress::indeterminate(),
        });
        self.requests.repaint = true;
        id
    }

    fn update_task(&mut self, task: TaskId, progress: TaskProgress) {
        if let Some(entry) = self.tasks.iter_mut().find(|entry| entry.id == task) {
            entry.progress = progress;
            self.requests.repaint = true;
        }
    }

    fn end_task(&mut self, task: TaskId) {
        self.tasks.retain(|entry| entry.id != task);
        self.requests.repaint = true;
    }

    fn refresh_contributions(&mut self) {
        self.requests.contributions_changed = true;
        self.requests.repaint = true;
    }

    fn request_repaint(&mut self) {
        self.requests.repaint = true;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::secrets::SecretBackend;
    use archipel_plugin_api::json;

    fn manifest(permissions: &str) -> PluginManifest {
        PluginManifest::parse(&format!(
            "id='x.y'\nname='X'\nversion='1'\napi_version=1\npermissions={permissions}\n"
        ))
        .unwrap()
    }

    fn host(permissions: &str, name: &str) -> (AppHost, PathBuf) {
        let root =
            std::env::temp_dir().join(format!("archipel-host-{name}-{}", std::process::id()));
        let (host, problem) = AppHost::new(
            &manifest(permissions),
            &[],
            Some(AppDirs::under(&root)),
            SecretBackend::File,
        );
        assert!(problem.is_none());
        (host, root)
    }

    #[test]
    fn directories_are_created_on_demand_and_separate() {
        let (host, root) = host("[]", "dirs");
        assert!(!root.exists());
        let data = host.data_dir().unwrap();
        let config = host.config_dir().unwrap();
        let cache = host.cache_dir().unwrap();
        assert!(data.is_dir() && config.is_dir() && cache.is_dir());
        assert!(data != config && config != cache);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn secrets_are_gated_by_the_manifest_permission() {
        let (mut denied, root) = host("[]", "denied");
        assert_eq!(
            denied.set_secret("k", "v"),
            Err(SecretError::PermissionDenied("x.y".into()))
        );
        let (mut allowed, root2) = host("['secrets']", "allowed");
        allowed.set_secret("k", "v").unwrap();
        assert_eq!(allowed.secret("k"), Ok(Some("v".into())));
        allowed.delete_secret("k").unwrap();
        assert_eq!(allowed.secret("k"), Ok(None));
        let _ = std::fs::remove_dir_all(root);
        let _ = std::fs::remove_dir_all(root2);
    }

    #[test]
    fn requests_notifications_and_tasks_are_recorded() {
        let (mut host, _) = host("[]", "requests");
        host.show_status("ready");
        host.notify(Notification::error("Oops", "details"));
        host.dispatch("a.b", json!(1));
        host.open_tool_window("panel");
        host.open_editor_tab(EditorTabRequest::new("a.view", "A"));
        assert_eq!(host.status(), "ready");
        assert_eq!(host.toasts().len(), 1);
        assert_eq!(host.requests.commands.len(), 1);
        assert_eq!(host.requests.tool_windows, ["panel"]);
        assert_eq!(host.requests.tabs.len(), 1);

        let id = host.begin_task("Import");
        host.update_task(id, TaskProgress::fraction(0.5).message("half"));
        assert_eq!(host.tasks()[0].progress.fraction, Some(0.5));
        host.end_task(id);
        assert!(host.tasks().is_empty());
    }

    #[test]
    fn toasts_expire_and_can_be_dismissed() {
        let (mut host, _) = host("[]", "toasts");
        host.notify(Notification::info("a", ""));
        host.notify(Notification::info("b", ""));
        let first = host.toasts()[0].id;
        host.dismiss_toast(first);
        assert_eq!(host.toasts().len(), 1);
        assert!(host.expire_toasts(Instant::now()).is_some());
        let later = Instant::now() + Duration::from_secs(60);
        assert_eq!(host.expire_toasts(later), None);
        assert!(host.toasts().is_empty());
    }
}
