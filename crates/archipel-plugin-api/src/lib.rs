//! Archipel plugin API (version [`API_VERSION`]).
//!
//! A plugin is an object implementing [`Plugin`]. It describes what it adds to
//! the workbench as plain-data [`Contributions`] (commands, menu items,
//! keybindings, settings pages, tool windows, editor tabs, status widgets),
//! executes its own commands, and renders its views into an opaque
//! [`ViewFrame`]. The host talks back to the IDE through [`PluginHost`].
//!
//! This crate is deliberately free of any UI toolkit so engine crates can
//! depend on it. Rust plugins render with egui through `archipel-plugin-ui`,
//! which downcasts the frame. See `docs/api.md`.

mod contributions;
mod manifest;
pub mod testing;
mod view;

pub use archipel_command::{CommandError, CommandResult};
pub use contributions::*;
pub use manifest::*;
pub use serde_json::{Value, json};
pub use view::*;

use std::any::Any;
use std::path::{Path, PathBuf};
use thiserror::Error;

/// Version of this API. Manifests declare the version they target in
/// `api_version`; the host refuses other versions.
pub const API_VERSION: u32 = 1;

#[derive(Debug, Clone, Error, PartialEq, Eq)]
pub enum PluginError {
    #[error("invalid plugin manifest: {0}")]
    Manifest(String),
    #[error("plugin id must use reverse-domain syntax (e.g. `com.example.tool`): {0}")]
    InvalidId(String),
    #[error(
        "plugin {plugin} targets plugin API version {requested}; this host supports version {supported}"
    )]
    UnsupportedApi {
        plugin: String,
        requested: u32,
        supported: u32,
    },
    #[error("plugin {plugin} needs permission `{permission}` but its manifest does not declare it")]
    PermissionDenied { plugin: String, permission: String },
    #[error("plugin not found: {0}")]
    NotFound(String),
    #[error("plugin {plugin} failed to activate: {message}")]
    Activation { plugin: String, message: String },
    #[error("plugin {0}: {1}")]
    Io(String, String),
}

/// The plugin contract.
///
/// All methods run on the UI thread; long work must move to a background
/// thread and report back through [`Plugin::poll`].
pub trait Plugin {
    fn manifest(&self) -> &PluginManifest;

    /// What the plugin adds to the workbench. Called once after activation;
    /// call [`PluginHost::refresh_contributions`] when it changes.
    fn contributions(&self) -> Contributions {
        Contributions::default()
    }

    fn activate(&mut self, _host: &mut dyn PluginHost) -> Result<(), PluginError> {
        Ok(())
    }

    /// Called when the plugin is disabled or the IDE exits.
    fn deactivate(&mut self, _host: &mut dyn PluginHost) {}

    /// Executes one of the plugin's contributed commands.
    fn execute(
        &mut self,
        command: &str,
        _params: Value,
        _host: &mut dyn PluginHost,
    ) -> CommandResult {
        Err(CommandError::Unknown(command.into()))
    }

    /// Renders `view` (a tool window, editor tab, settings page or the
    /// standalone app window) into `frame`.
    fn render_view(
        &mut self,
        _view: &ViewContext,
        _frame: &mut ViewFrame<'_>,
        _host: &mut dyn PluginHost,
    ) {
    }

    /// Text of a contributed status bar widget.
    fn status_widget(&self, _id: &str) -> Option<StatusWidgetState> {
        None
    }

    /// Called every frame while the plugin is active; collect background
    /// results here and call [`PluginHost::request_repaint`] while busy.
    fn poll(&mut self, _host: &mut dyn PluginHost) {}
}

/// Where a view is being shown.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ViewPlacement {
    ToolWindow {
        id: String,
    },
    EditorTab {
        key: String,
        path: Option<PathBuf>,
    },
    /// The whole window in standalone app mode.
    App,
    SettingsPage {
        id: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ViewContext {
    pub view: String,
    pub placement: ViewPlacement,
}

/// Opaque render target. The egui host passes `&mut egui::Ui`; use
/// `archipel_plugin_ui::ui(frame)` to get it.
pub struct ViewFrame<'a> {
    surface: &'a mut dyn Any,
}

impl<'a> ViewFrame<'a> {
    pub fn new<T: Any>(surface: &'a mut T) -> Self {
        Self { surface }
    }

    pub fn downcast_mut<T: Any>(&mut self) -> Option<&mut T> {
        self.surface.downcast_mut::<T>()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotificationLevel {
    Info,
    Warning,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Notification {
    pub level: NotificationLevel,
    pub title: String,
    pub body: String,
}

impl Notification {
    pub fn info(title: impl Into<String>, body: impl Into<String>) -> Self {
        Self {
            level: NotificationLevel::Info,
            title: title.into(),
            body: body.into(),
        }
    }

    pub fn error(title: impl Into<String>, body: impl Into<String>) -> Self {
        Self {
            level: NotificationLevel::Error,
            title: title.into(),
            body: body.into(),
        }
    }
}

/// Request to open an editor tab showing one of the plugin's views.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditorTabRequest {
    pub view: String,
    pub title: String,
    /// Tabs with the same key are reused instead of duplicated.
    pub key: String,
    pub path: Option<PathBuf>,
}

impl EditorTabRequest {
    pub fn new(view: impl Into<String>, title: impl Into<String>) -> Self {
        let view = view.into();
        Self {
            key: view.clone(),
            view,
            title: title.into(),
            path: None,
        }
    }

    pub fn key(mut self, key: impl Into<String>) -> Self {
        self.key = key.into();
        self
    }

    pub fn path(mut self, path: impl Into<PathBuf>) -> Self {
        self.path = Some(path.into());
        self
    }
}

/// Services the IDE offers to a plugin. Each plugin gets a host scoped to
/// its id, settings and permissions.
pub trait PluginHost {
    fn plugin_id(&self) -> &str;
    fn project_root(&self) -> Option<&Path>;
    /// Short message in the status bar.
    fn show_status(&mut self, message: &str);
    fn notify(&mut self, notification: Notification);
    /// Queues a command (built-in or contributed) to run after the current call.
    fn dispatch(&mut self, command: &str, params: Value);
    /// Shows one of this plugin's tool windows.
    fn open_tool_window(&mut self, id: &str);
    fn open_editor_tab(&mut self, request: EditorTabRequest);
    /// This plugin's setting, or the default declared by a settings page.
    fn setting(&self, key: &str) -> Option<Value>;
    fn set_setting(&mut self, key: &str, value: Value) -> Result<(), PluginError>;
    /// Per-user directory owned by this plugin (created on demand).
    fn data_dir(&self) -> Result<PathBuf, PluginError>;
    fn has_permission(&self, permission: Permission) -> bool;
    /// Fails with [`PluginError::PermissionDenied`] unless declared.
    fn require_permission(&self, permission: Permission) -> Result<(), PluginError> {
        if self.has_permission(permission) {
            Ok(())
        } else {
            Err(PluginError::PermissionDenied {
                plugin: self.plugin_id().into(),
                permission: permission.id().into(),
            })
        }
    }
    /// Re-reads [`Plugin::contributions`] after the current call.
    fn refresh_contributions(&mut self);
    /// Keeps the UI repainting (e.g. while a request is in flight).
    fn request_repaint(&mut self);
}
