//! An in-memory [`PluginHost`] for plugin unit tests.

use crate::{
    EditorTabRequest, Notification, Permission, PluginError, PluginHost, SecretError, TaskId,
    TaskProgress, Value,
};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// Records everything a plugin asks the host to do.
#[derive(Debug, Default)]
pub struct RecordingHost {
    pub id: String,
    pub project: Option<PathBuf>,
    /// Simulate a standalone application host (false by default).
    pub standalone: bool,
    pub permissions: Vec<Permission>,
    pub statuses: Vec<String>,
    pub notifications: Vec<Notification>,
    pub dispatched: Vec<(String, Value)>,
    pub tool_windows: Vec<String>,
    pub editor_tabs: Vec<EditorTabRequest>,
    pub settings: BTreeMap<String, Value>,
    /// In-memory secrets (`secrets` permission required).
    pub secrets: BTreeMap<String, String>,
    /// Titles of the tasks started, by id; ended tasks are removed.
    pub tasks: BTreeMap<TaskId, String>,
    pub task_updates: Vec<(TaskId, TaskProgress)>,
    next_task: u64,
    pub data: Option<PathBuf>,
    pub contribution_refreshes: usize,
    pub repaints: usize,
    pub app_focuses: usize,
}

impl RecordingHost {
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            ..Self::default()
        }
    }

    pub fn with_permissions(mut self, permissions: &[Permission]) -> Self {
        self.permissions = permissions.to_vec();
        self
    }
}

impl RecordingHost {
    fn check_secrets(&self) -> Result<(), SecretError> {
        if self.has_permission(Permission::Secrets) {
            Ok(())
        } else {
            Err(SecretError::PermissionDenied(self.id.clone()))
        }
    }
}

impl PluginHost for RecordingHost {
    fn is_standalone(&self) -> bool {
        self.standalone
    }

    fn focus_app_view(&mut self) {
        self.app_focuses += 1;
    }
    fn plugin_id(&self) -> &str {
        &self.id
    }

    fn project_root(&self) -> Option<&Path> {
        self.project.as_deref()
    }

    fn show_status(&mut self, message: &str) {
        self.statuses.push(message.into());
    }

    fn notify(&mut self, notification: Notification) {
        self.notifications.push(notification);
    }

    fn dispatch(&mut self, command: &str, params: Value) {
        self.dispatched.push((command.into(), params));
    }

    fn open_tool_window(&mut self, id: &str) {
        self.tool_windows.push(id.into());
    }

    fn open_editor_tab(&mut self, request: EditorTabRequest) {
        self.editor_tabs.push(request);
    }

    fn setting(&self, key: &str) -> Option<Value> {
        self.settings.get(key).cloned()
    }

    fn set_setting(&mut self, key: &str, value: Value) -> Result<(), PluginError> {
        self.settings.insert(key.into(), value);
        Ok(())
    }

    fn data_dir(&self) -> Result<PathBuf, PluginError> {
        self.data
            .clone()
            .ok_or_else(|| PluginError::Io(self.id.clone(), "no data directory in tests".into()))
    }

    fn has_permission(&self, permission: Permission) -> bool {
        self.permissions.contains(&permission)
    }

    fn secret(&self, key: &str) -> Result<Option<String>, SecretError> {
        self.check_secrets()?;
        Ok(self.secrets.get(key).cloned())
    }

    fn set_secret(&mut self, key: &str, value: &str) -> Result<(), SecretError> {
        self.check_secrets()?;
        self.secrets.insert(key.into(), value.into());
        Ok(())
    }

    fn delete_secret(&mut self, key: &str) -> Result<(), SecretError> {
        self.check_secrets()?;
        self.secrets.remove(key);
        Ok(())
    }

    fn begin_task(&mut self, title: &str) -> TaskId {
        self.next_task += 1;
        let id = TaskId(self.next_task);
        self.tasks.insert(id, title.into());
        id
    }

    fn update_task(&mut self, task: TaskId, progress: TaskProgress) {
        self.task_updates.push((task, progress));
    }

    fn end_task(&mut self, task: TaskId) {
        self.tasks.remove(&task);
    }

    fn refresh_contributions(&mut self) {
        self.contribution_refreshes += 1;
    }

    fn request_repaint(&mut self) {
        self.repaints += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ViewContext, ViewPlacement};

    #[test]
    fn secrets_need_the_permission_and_round_trip() {
        let mut host = RecordingHost::new("x.y");
        assert_eq!(
            host.secret("k"),
            Err(SecretError::PermissionDenied("x.y".into()))
        );
        assert!(host.set_secret("k", "v").is_err());
        let mut host = host.with_permissions(&[Permission::Secrets]);
        assert_eq!(host.secret("k"), Ok(None));
        host.set_secret("k", "v").unwrap();
        assert_eq!(host.secret("k"), Ok(Some("v".into())));
        host.delete_secret("k").unwrap();
        host.delete_secret("k").unwrap();
        assert_eq!(host.secret("k"), Ok(None));
    }

    #[test]
    fn hosts_without_secret_storage_say_unsupported() {
        struct Bare;
        impl PluginHost for Bare {
            fn plugin_id(&self) -> &str {
                "x.y"
            }
            fn project_root(&self) -> Option<&Path> {
                None
            }
            fn show_status(&mut self, _: &str) {}
            fn notify(&mut self, _: Notification) {}
            fn dispatch(&mut self, _: &str, _: Value) {}
            fn open_tool_window(&mut self, _: &str) {}
            fn setting(&self, _: &str) -> Option<Value> {
                None
            }
            fn set_setting(&mut self, _: &str, _: Value) -> Result<(), PluginError> {
                Ok(())
            }
            fn data_dir(&self) -> Result<PathBuf, PluginError> {
                Err(PluginError::NotFound("x".into()))
            }
            fn has_permission(&self, _: Permission) -> bool {
                true
            }
            fn refresh_contributions(&mut self) {}
            fn request_repaint(&mut self) {}
        }
        let mut host = Bare;
        assert_eq!(host.secret("k"), Err(SecretError::Unsupported));
        assert_eq!(host.set_secret("k", "v"), Err(SecretError::Unsupported));
        assert_eq!(host.delete_secret("k"), Err(SecretError::Unsupported));
        host.open_editor_tab(EditorTabRequest::new("a.b", "B"));
    }

    #[test]
    fn a_tool_window_opens_an_editor_tab_once_per_key() {
        let mut host = RecordingHost::new("x.y");
        let tool = ViewContext {
            view: "x.panel".into(),
            placement: ViewPlacement::ToolWindow { id: "panel".into() },
        };
        let request = || {
            EditorTabRequest::new("x.doc", "Doc")
                .key("doc:1")
                .path("/tmp/a")
        };
        assert!(tool.open_editor_tab(&mut host, request()));
        assert_eq!(host.editor_tabs, vec![request()]);

        let tab = ViewContext {
            view: "x.doc".into(),
            placement: ViewPlacement::EditorTab {
                key: "doc:1".into(),
                path: None,
            },
        };
        assert!(
            !tab.open_editor_tab(&mut host, request()),
            "already this tab"
        );
        assert!(tab.open_editor_tab(&mut host, request().key("doc:2")));
        assert_eq!(host.editor_tabs.len(), 2);
    }

    #[test]
    fn tasks_are_tracked_until_ended() {
        let mut host = RecordingHost::new("x.y");
        let id = host.begin_task("Import");
        host.update_task(id, TaskProgress::fraction(2.0).message("half"));
        assert_eq!(host.tasks.get(&id).map(String::as_str), Some("Import"));
        assert_eq!(host.task_updates[0].1.fraction, Some(1.0));
        host.end_task(id);
        assert!(host.tasks.is_empty());
    }
}
