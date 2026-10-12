//! A standalone host for Archipel plugins: runs any [`Plugin`] as a desktop app.
//!
//! ```no_run
//! use archipel_app::{AppOptions, api::{Plugin, PluginManifest}};
//!
//! struct MyPlugin(PluginManifest);
//!
//! impl Plugin for MyPlugin {
//!     fn manifest(&self) -> &PluginManifest {
//!         &self.0
//!     }
//! }
//!
//! fn main() -> Result<(), archipel_app::AppError> {
//!     let manifest = PluginManifest::parse(
//!         "id = 'com.example.tool'\nname = 'Tool'\nversion = '0.1.0'\napi_version = 1\n\
//!          [app]\nview = 'tool.main'\n",
//!     )
//!     .expect("valid manifest");
//!     archipel_app::run(MyPlugin(manifest), AppOptions::new("Tool", "com.example.tool"))
//! }
//! ```
//!
//! The window installs the Archipel Kit theme and fonts and follows the OS dark/light
//! setting. It implements [`PluginHost`] ([`AppHost`]) with per-app directories, TOML
//! settings, secrets (OS keychain), toasts, a status bar and tasks; hosts the plugin's `[app]`
//! view in the window, tool windows as side panels and editor tabs in a tab bar; and has a
//! command palette (Ctrl+Shift+A). See `docs/standalone-apps.md`.

mod app;
mod dirs;
mod host;
mod keymap;
mod palette;
mod secrets;
mod settings;
mod settings_ui;

pub use app::{App, CMD_PALETTE, CMD_QUIT, CMD_SETTINGS};
pub use archipel_plugin_api::{self as api, Plugin, PluginHost};
pub use dirs::AppDirs;
pub use host::{AppHost, TaskEntry, ToastEntry};
pub use keymap::{Keymap, chord_matches, display_chord};
pub use palette::{Palette, PaletteEntry, filter as filter_commands};
pub use secrets::{SecretBackend, SecretStore};
pub use settings::SettingsStore;

use std::path::PathBuf;
use std::sync::Arc;
use thiserror::Error;

/// Environment variable that picks the renderer: `wgpu` (default) or `glow`.
pub const RENDERER_ENV: &str = "ARCHIPEL_RENDERER";

#[derive(Debug, Error)]
pub enum AppError {
    #[error("plugin {0} has no `[app]` view in its manifest, so it cannot run as an app")]
    NoAppView(String),
    #[error("{RENDERER_ENV}: {0}")]
    Renderer(String),
    #[error("window failed: {0}")]
    Window(String),
}

/// The graphics backend of the window.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Renderer {
    /// wgpu on Vulkan, Metal, DX12 or GL.
    #[default]
    Wgpu,
    /// OpenGL through glow: the fallback for machines without a working Vulkan driver.
    Glow,
}

impl Renderer {
    /// Parses the value of `ARCHIPEL_RENDERER`; `None` and the empty string mean the default.
    pub fn parse(value: Option<&str>) -> Result<Self, AppError> {
        match value.map(|v| v.trim().to_ascii_lowercase()).as_deref() {
            None | Some("") | Some("wgpu") => Ok(Self::Wgpu),
            Some("glow") => Ok(Self::Glow),
            Some(other) => Err(AppError::Renderer(format!(
                "unknown renderer `{other}`; use `wgpu` or `glow`"
            ))),
        }
    }

    pub fn from_env() -> Result<Self, AppError> {
        Self::parse(std::env::var(RENDERER_ENV).ok().as_deref())
    }

    fn eframe(self) -> eframe::Renderer {
        match self {
            Self::Wgpu => eframe::Renderer::Wgpu,
            Self::Glow => eframe::Renderer::Glow,
        }
    }
}

/// How the app picks light or dark.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ThemeMode {
    /// Follow the OS setting (dark when it is unknown).
    #[default]
    System,
    Light,
    Dark,
}

impl ThemeMode {
    /// Reads `--light` / `--dark` from command-line arguments (the last one wins).
    pub fn from_args(args: impl IntoIterator<Item = String>) -> Result<Self, String> {
        let mut mode = Self::System;
        for arg in args {
            match arg.as_str() {
                "--light" => mode = Self::Light,
                "--dark" => mode = Self::Dark,
                other => return Err(format!("unknown argument `{other}`; use --light or --dark")),
            }
        }
        Ok(mode)
    }
}

/// Options of [`run`]. `AppOptions::default()` takes everything from the plugin's manifest.
#[derive(Debug, Clone)]
pub struct AppOptions {
    /// Window title; the manifest's `[app] title` (or name) when empty.
    pub title: String,
    /// Id of the app: names its directories and the window's app id (Wayland, X11 class).
    /// The plugin id when empty.
    pub id: String,
    /// Initial inner size in points.
    pub size: [f32; 2],
    pub min_size: [f32; 2],
    pub theme: ThemeMode,
    /// Extra keybindings `(chord, command)`, on top of the plugin's and under the user's
    /// `keymap.toml`.
    pub keybindings: Vec<(String, String)>,
    /// Keeps data, config and cache below this directory instead of the platform's.
    pub dirs_root: Option<PathBuf>,
    pub secrets: SecretBackend,
    /// Overrides `ARCHIPEL_RENDERER`.
    pub renderer: Option<Renderer>,
    /// Window and taskbar icon; the platform's default when `None`. See
    /// [`AppOptions::with_icon_png`].
    pub icon: Option<Arc<egui::IconData>>,
}

impl Default for AppOptions {
    fn default() -> Self {
        Self {
            title: String::new(),
            id: String::new(),
            size: [1280.0, 800.0],
            min_size: [760.0, 480.0],
            theme: ThemeMode::System,
            keybindings: Vec::new(),
            dirs_root: None,
            secrets: SecretBackend::Auto,
            renderer: None,
            icon: None,
        }
    }
}

impl AppOptions {
    pub fn new(title: impl Into<String>, id: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            id: id.into(),
            ..Self::default()
        }
    }

    /// Sets the window icon from PNG bytes, typically `include_bytes!` of a 256 or 512 px
    /// square image with a transparent background.
    ///
    /// # Errors
    /// The bytes are not a PNG the decoder understands.
    pub fn with_icon_png(mut self, png: &[u8]) -> Result<Self, String> {
        let icon = eframe::icon_data::from_png_bytes(png)
            .map_err(|error| format!("app icon is not a valid PNG: {error}"))?;
        self.icon = Some(Arc::new(icon));
        Ok(self)
    }
}

/// The eframe options for `options`: window geometry, app id, icon and renderer.
pub fn native_options(
    options: &AppOptions,
    title: &str,
    renderer: Renderer,
) -> eframe::NativeOptions {
    let mut viewport = egui::ViewportBuilder::default()
        .with_title(title)
        .with_inner_size(options.size)
        .with_min_inner_size(options.min_size);
    if !options.id.is_empty() {
        viewport = viewport.with_app_id(options.id.clone());
    }
    if let Some(icon) = &options.icon {
        viewport = viewport.with_icon(Arc::clone(icon));
    }
    eframe::NativeOptions {
        viewport,
        renderer: renderer.eframe(),
        ..Default::default()
    }
}

struct Shell(App);

impl eframe::App for Shell {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.0.update(ctx);
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.0.shutdown();
    }
}

/// Opens a window with `plugin` as the app and runs until it closes.
///
/// Fails before opening a window when the plugin has no `[app]` view or
/// `ARCHIPEL_RENDERER` is invalid.
pub fn run(plugin: impl Plugin + 'static, options: AppOptions) -> Result<(), AppError> {
    let renderer = match options.renderer {
        Some(renderer) => renderer,
        None => Renderer::from_env()?,
    };
    let app = App::new(plugin, options.clone())?;
    let native = native_options(&options, app.title(), renderer);
    let title = app.title().to_owned();
    let mut app = Some(app);
    eframe::run_native(
        &title,
        native,
        Box::new(move |_cc| {
            let app = app.take().ok_or("the app was already started")?;
            Ok(Box::new(Shell(app)))
        }),
    )
    .map_err(|error| AppError::Window(error.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_renderer_env_picks_glow_and_rejects_typos() {
        assert_eq!(Renderer::parse(None).unwrap(), Renderer::Wgpu);
        assert_eq!(Renderer::parse(Some("")).unwrap(), Renderer::Wgpu);
        assert_eq!(Renderer::parse(Some("wgpu")).unwrap(), Renderer::Wgpu);
        assert_eq!(Renderer::parse(Some(" GLOW ")).unwrap(), Renderer::Glow);
        let error = Renderer::parse(Some("vulkan")).unwrap_err().to_string();
        assert!(error.contains("ARCHIPEL_RENDERER") && error.contains("vulkan"));
        let native = native_options(&AppOptions::default(), "T", Renderer::Glow);
        assert!(matches!(native.renderer, eframe::Renderer::Glow));
        let native = native_options(&AppOptions::default(), "T", Renderer::Wgpu);
        assert!(matches!(native.renderer, eframe::Renderer::Wgpu));
    }

    /// A 2×1 RGBA PNG: one opaque red pixel, one transparent pixel.
    fn tiny_png() -> Vec<u8> {
        let image = eframe::icon_data::IconDataExt::to_png_bytes(&egui::IconData {
            rgba: vec![255, 0, 0, 255, 0, 0, 0, 0],
            width: 2,
            height: 1,
        });
        image.expect("encodes")
    }

    #[test]
    fn the_icon_png_reaches_the_viewport() {
        let native = native_options(&AppOptions::default(), "T", Renderer::Glow);
        assert!(native.viewport.icon.is_none());

        let options = AppOptions::default().with_icon_png(&tiny_png()).unwrap();
        let icon = options.icon.as_deref().expect("icon set");
        assert_eq!((icon.width, icon.height), (2, 1));
        assert_eq!(icon.rgba, [255, 0, 0, 255, 0, 0, 0, 0]);

        let native = native_options(&options, "T", Renderer::Glow);
        let applied = native.viewport.icon.expect("viewport icon");
        assert_eq!(*applied, *icon);
    }

    #[test]
    fn an_invalid_icon_png_is_an_error_not_a_panic() {
        for bytes in [&b""[..], b"not a png", &tiny_png()[..20]] {
            let error = AppOptions::default().with_icon_png(bytes).unwrap_err();
            assert!(error.contains("PNG"), "{error}");
        }
    }

    #[test]
    fn theme_flags() {
        let args = |a: &[&str]| ThemeMode::from_args(a.iter().map(|s| s.to_string()));
        assert_eq!(args(&[]), Ok(ThemeMode::System));
        assert_eq!(args(&["--light", "--dark"]), Ok(ThemeMode::Dark));
        assert!(args(&["--nope"]).is_err());
    }
}
