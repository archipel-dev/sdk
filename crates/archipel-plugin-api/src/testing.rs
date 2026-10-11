//! An in-memory [`PluginHost`] for plugin unit tests.

use crate::{EditorTabRequest, Notification, Permission, PluginError, PluginHost, Value};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// Records everything a plugin asks the host to do.
#[derive(Debug, Default)]
pub struct RecordingHost {
    pub id: String,
    pub project: Option<PathBuf>,
    pub permissions: Vec<Permission>,
    pub statuses: Vec<String>,
    pub notifications: Vec<Notification>,
    pub dispatched: Vec<(String, Value)>,
    pub tool_windows: Vec<String>,
    pub editor_tabs: Vec<EditorTabRequest>,
    pub settings: BTreeMap<String, Value>,
    pub data: Option<PathBuf>,
    pub contribution_refreshes: usize,
    pub repaints: usize,
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

impl PluginHost for RecordingHost {
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

    fn refresh_contributions(&mut self) {
        self.contribution_refreshes += 1;
    }

    fn request_repaint(&mut self) {
        self.repaints += 1;
    }
}
