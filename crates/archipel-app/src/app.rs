//! [`App`]: the window of a standalone app, independent of eframe so it can run headless.

use crate::dirs::AppDirs;
use crate::host::AppHost;
use crate::keymap::Keymap;
use crate::palette::{Palette, PaletteEntry};
use crate::settings_ui::SettingsView;
use crate::{AppError, AppOptions, ThemeMode};
use archipel_plugin_api::{
    Contributions, EditorTabRequest, Notification, Plugin, PluginHost, ToolWindowAnchor,
    ToolWindowContribution, Value, ViewContext, ViewFrame, ViewPlacement,
};
use archipel_plugin_ui::kit::components::{
    Avatar, Button, EmptyState, IconButton, MenuItem, MenuSeparator, Panel, ProgressBar, Severity,
    Tab, TabBar, Toast, ToolbarButton, WindowChrome, WindowToolbar,
};
use archipel_plugin_ui::kit::parts::{Popover, Slot};
use archipel_plugin_ui::kit::{self, Glyph, Theme, tokens};
use archipel_plugin_ui::{empty_state, muted};
use egui::{Context, Ui};
use std::time::Instant;

/// Built-in command ids; everything under `app.` belongs to the shell.
pub const CMD_PALETTE: &str = "app.commandPalette";
pub const CMD_SETTINGS: &str = "app.settings";
pub const CMD_QUIT: &str = "app.quit";
const CMD_TOOL_WINDOW: &str = "app.toolWindow.";

/// How the window meets the native title bar: on macOS the main toolbar takes the title
/// bar's place and holds the traffic lights; elsewhere the native decorations stay.
pub const CHROME: WindowChrome = WindowChrome::Integrated;

/// Width of the app menu under the app widget.
const APP_MENU_WIDTH: f32 = 240.0;

/// How many chained `dispatch` calls run in one frame before the rest waits for the next.
const MAX_DISPATCH_PER_FRAME: usize = 64;

#[derive(Debug, Clone, PartialEq)]
enum TabKind {
    Editor(EditorTabRequest),
    Settings,
}

#[derive(Debug, Clone, PartialEq)]
struct OpenTab {
    key: String,
    title: String,
    kind: TabKind,
}

/// A plugin running as a standalone app: the plugin, its [`AppHost`] and the window
/// state (tool windows, tabs, palette, toasts).
pub struct App {
    plugin: Box<dyn Plugin>,
    host: AppHost,
    title: String,
    app_view: ViewContext,
    contributions: Contributions,
    keymap: Keymap,
    user_keymap: std::path::PathBuf,
    extra_bindings: Vec<(String, String)>,
    palette: Palette,
    /// The app widget's menu is open.
    app_menu: bool,
    settings_view: SettingsView,
    open_tool_windows: Vec<String>,
    tabs: Vec<OpenTab>,
    /// `None`: the app view; `Some(i)`: `tabs[i]`.
    current: Option<usize>,
    theme: ThemeMode,
    shown: Option<bool>,
    frames: u32,
    failed: Option<String>,
}

impl App {
    /// Builds the app around `plugin`. Fails when the plugin has no `[app]` view. Nothing
    /// is created on disk and the plugin is not activated until the second frame, so the
    /// window shows before any setup runs.
    pub fn new(plugin: impl Plugin + 'static, options: AppOptions) -> Result<Self, AppError> {
        let plugin: Box<dyn Plugin> = Box::new(plugin);
        let manifest = plugin.manifest().clone();
        let app = manifest
            .app
            .clone()
            .ok_or_else(|| AppError::NoAppView(manifest.id.clone()))?;
        let contributions = plugin.contributions();
        let id = if options.id.is_empty() {
            manifest.id.clone()
        } else {
            options.id.clone()
        };
        let dirs = match &options.dirs_root {
            Some(root) => Some(AppDirs::under(root)),
            None => AppDirs::for_app(&id),
        };
        let (mut host, problem) = AppHost::new(
            &manifest,
            &contributions.settings_pages,
            dirs.clone(),
            options.secrets,
        );
        if let Some(problem) = problem {
            host.notify(Notification::error("Settings not loaded", problem));
        }
        let user_keymap = dirs
            .map(|d| d.config.join("keymap.toml"))
            .unwrap_or_default();
        let mut this = Self {
            plugin,
            host,
            title: if options.title.is_empty() {
                manifest.app_title()
            } else {
                options.title
            },
            app_view: ViewContext {
                view: app.view,
                placement: ViewPlacement::App,
            },
            contributions: Contributions::default(),
            keymap: Keymap::default(),
            user_keymap,
            extra_bindings: options.keybindings,
            palette: Palette::default(),
            app_menu: false,
            settings_view: SettingsView::default(),
            open_tool_windows: Vec::new(),
            tabs: Vec::new(),
            current: None,
            theme: options.theme,
            shown: None,
            frames: 0,
            failed: None,
        };
        this.set_contributions(contributions);
        Ok(this)
    }

    pub fn host(&self) -> &AppHost {
        &self.host
    }

    pub fn host_mut(&mut self) -> &mut AppHost {
        &mut self.host
    }

    pub fn title(&self) -> &str {
        &self.title
    }

    pub fn contributions(&self) -> &Contributions {
        &self.contributions
    }

    pub fn keymap(&self) -> &Keymap {
        &self.keymap
    }

    pub fn palette_open(&self) -> bool {
        self.palette.is_open()
    }

    /// Whether the app widget's menu (in the main toolbar) is open.
    pub fn app_menu_open(&self) -> bool {
        self.app_menu
    }

    /// Titles of the open editor tabs (and the settings tab).
    pub fn tab_titles(&self) -> Vec<&str> {
        self.tabs.iter().map(|tab| tab.title.as_str()).collect()
    }

    /// Ids of the open tool windows.
    pub fn open_tool_windows(&self) -> &[String] {
        &self.open_tool_windows
    }

    /// Why activation failed, if it did.
    pub fn failure(&self) -> Option<&str> {
        self.failed.as_deref()
    }

    /// Whether the app needs another frame soon (animations, tasks, toasts, queued work).
    pub fn wants_repaint(&mut self) -> bool {
        std::mem::take(&mut self.host.requests.repaint)
    }

    /// Deactivates the plugin; `run` calls it when the window closes.
    pub fn shutdown(&mut self) {
        if self.frames > 1 && self.failed.is_none() {
            self.plugin.deactivate(&mut self.host);
        }
    }

    /// Runs `command`: a built-in of the shell or one of the plugin's. Errors become toasts.
    pub fn run_command(&mut self, ctx: &Context, command: &str, params: Value) {
        // Palette picks happen after the central view has painted this frame.
        self.host.requests.repaint = true;
        ctx.request_repaint();
        if command == CMD_PALETTE {
            self.palette.toggle();
        } else if command == CMD_SETTINGS {
            self.open_settings();
        } else if command == CMD_QUIT {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        } else if let Some(id) = command.strip_prefix(CMD_TOOL_WINDOW) {
            self.toggle_tool_window(id);
        } else if let Err(error) = self.plugin.execute(command, params, &mut self.host) {
            let label = self
                .contributions
                .commands
                .iter()
                .find(|c| c.id == command)
                .map_or(command, |c| c.label.as_str())
                .to_owned();
            self.host
                .notify(Notification::error(label, error.to_string()));
        }
    }

    fn set_contributions(&mut self, contributions: Contributions) {
        for problem in contributions.problems() {
            self.host
                .notify(Notification::error("Plugin contributions", problem));
        }
        self.host.set_pages(&contributions.settings_pages);
        let (mut keymap, _) = Keymap::from_contributions(&contributions);
        let _ = keymap.bind_str("Mod+Shift+A", CMD_PALETTE);
        for (chord, command) in &self.extra_bindings {
            if let Err(error) = keymap.bind_str(chord, command) {
                self.host
                    .notify(Notification::error("Invalid keybinding", error));
            }
        }
        for problem in keymap.apply_user_file(&self.user_keymap) {
            self.host
                .notify(Notification::error("keymap.toml", problem));
        }
        self.keymap = keymap;
        self.contributions = contributions;
        self.open_tool_windows
            .retain(|id| self.contributions.tool_windows.iter().any(|w| &w.id == id));
    }

    fn open_settings(&mut self) {
        if self.contributions.settings_pages.is_empty() {
            self.host
                .notify(Notification::info("Settings", "This app has no settings."));
            return;
        }
        self.open_tab(OpenTab {
            key: "app.settings".into(),
            title: "Settings".into(),
            kind: TabKind::Settings,
        });
    }

    fn open_tab(&mut self, tab: OpenTab) {
        let index = match self.tabs.iter().position(|t| t.key == tab.key) {
            Some(index) => index,
            None => {
                self.tabs.push(tab);
                self.tabs.len() - 1
            }
        };
        self.current = Some(index);
    }

    fn toggle_tool_window(&mut self, id: &str) {
        if let Some(position) = self.open_tool_windows.iter().position(|w| w == id) {
            self.open_tool_windows.remove(position);
        } else if self.contributions.tool_windows.iter().any(|w| w.id == id) {
            self.open_tool_windows.push(id.to_owned());
        }
    }

    fn palette_entries(&self) -> Vec<PaletteEntry> {
        let builtin = |id: &str, label: &str| PaletteEntry {
            id: id.into(),
            label: label.into(),
            category: "App".into(),
            shortcut: self.keymap.shortcut_of(id),
        };
        let mut entries: Vec<PaletteEntry> = self
            .contributions
            .commands
            .iter()
            .map(|command| PaletteEntry {
                id: command.id.clone(),
                label: command.label.clone(),
                category: command.category.clone(),
                shortcut: self.keymap.shortcut_of(&command.id),
            })
            .collect();
        entries.push(builtin(CMD_PALETTE, "Command Palette"));
        if !self.contributions.settings_pages.is_empty() {
            entries.push(builtin(CMD_SETTINGS, "Open Settings"));
        }
        for window in &self.contributions.tool_windows {
            entries.push(builtin(
                &format!("{CMD_TOOL_WINDOW}{}", window.id),
                &format!("Toggle {}", window.title),
            ));
        }
        entries.push(builtin(CMD_QUIT, "Quit"));
        entries
    }

    fn follow_theme(&mut self, ctx: &Context) {
        let dark = match self.theme {
            ThemeMode::Light => false,
            ThemeMode::Dark => true,
            ThemeMode::System => !matches!(ctx.system_theme(), Some(egui::Theme::Light)),
        };
        if self.shown != Some(dark) {
            self.shown = Some(dark);
            kit::install(ctx, if dark { Theme::dark() } else { Theme::light() });
        }
    }

    /// The command whose chord was pressed this frame.
    fn pressed_command(&self, ctx: &Context) -> Option<String> {
        ctx.input(|input| {
            input.events.iter().find_map(|event| match event {
                egui::Event::Key {
                    key,
                    pressed: true,
                    repeat: false,
                    modifiers,
                    ..
                } => self.keymap.command_for(*key, *modifiers).map(str::to_owned),
                _ => None,
            })
        })
    }

    /// Runs one frame. Call from `eframe::App::update`.
    pub fn update(&mut self, ctx: &Context) {
        self.follow_theme(ctx);
        let frame = self.frames;
        self.frames = self.frames.saturating_add(1);
        if frame == 1 {
            // The first frame only paints the window; the plugin starts now.
            match self.plugin.activate(&mut self.host) {
                Ok(()) => self.set_contributions(self.plugin.contributions()),
                Err(error) => {
                    let message = error.to_string();
                    self.host
                        .notify(Notification::error("Plugin failed to start", &message));
                    self.failed = Some(message);
                }
            }
        }
        let running = frame >= 1 && self.failed.is_none();
        if running {
            if let Some(command) = self.pressed_command(ctx)
                && (!self.palette.is_open() || command == CMD_PALETTE)
            {
                self.run_command(ctx, &command, Value::Null);
            }
            self.plugin.poll(&mut self.host);
            self.drain_requests(ctx);
        }

        self.main_toolbar(ctx);
        self.status_bar(ctx);
        self.tool_windows(ctx, running);
        self.central(ctx, running);
        let entries = self.palette_entries();
        if let Some(command) = self.palette.show(ctx, &entries) {
            self.run_command(ctx, &command, Value::Null);
        }
        self.toasts(ctx);

        if frame == 0 || self.wants_repaint() {
            ctx.request_repaint();
        }
        if !self.host.tasks().is_empty() {
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
        }
    }

    fn drain_requests(&mut self, ctx: &Context) {
        for _ in 0..MAX_DISPATCH_PER_FRAME {
            if std::mem::take(&mut self.host.requests.focus_app) {
                self.current = None;
            }
            if self.host.requests.contributions_changed {
                self.host.requests.contributions_changed = false;
                self.set_contributions(self.plugin.contributions());
            }
            for id in std::mem::take(&mut self.host.requests.tool_windows) {
                if !self.open_tool_windows.contains(&id) {
                    self.toggle_tool_window(&id);
                }
            }
            for request in std::mem::take(&mut self.host.requests.tabs) {
                self.open_tab(OpenTab {
                    key: request.key.clone(),
                    title: request.title.clone(),
                    kind: TabKind::Editor(request),
                });
            }
            let Some((command, params)) = self.host.requests.commands.pop_front() else {
                break;
            };
            self.run_command(ctx, &command, params);
        }
    }

    /// The main toolbar, like Android Studio's: the app widget (its tile, name and menu with
    /// the app's own commands) on the left; Settings and the command palette on the right.
    fn main_toolbar(&mut self, ctx: &Context) {
        let has_settings = !self.contributions.settings_pages.is_empty();
        let mut command: Option<&'static str> = None;
        let title = self.title.clone();
        let palette_shortcut = self.keymap.shortcut_of(CMD_PALETTE);
        let settings_shortcut = self.keymap.shortcut_of(CMD_SETTINGS);
        let quit_shortcut = self.keymap.shortcut_of(CMD_QUIT);
        let mut menu = self.app_menu;
        WindowToolbar::new("archipel-main-toolbar")
            .chrome(CHROME)
            .project_color(kit::theme::avatar_color(&title))
            .show(ctx, |ui| {
                let button = app_widget(&title, menu).show(ui);
                if button.clicked() {
                    menu = !menu;
                }
                let closed = Popover::below("archipel-app-menu", &button)
                    .width(APP_MENU_WIDTH)
                    .show(ui.ctx(), &mut menu, |ui| {
                        let item = |label: &str, icon: Glyph, shortcut: &Option<String>| {
                            let item = MenuItem::new(label).icon(icon);
                            match shortcut {
                                Some(shortcut) => item.shortcut(shortcut.as_str()),
                                None => item,
                            }
                        };
                        if ui
                            .add(item("Command Palette…", Glyph::Search, &palette_shortcut))
                            .clicked()
                        {
                            command = Some(CMD_PALETTE);
                        }
                        if has_settings
                            && ui
                                .add(item("Settings…", Glyph::Settings, &settings_shortcut))
                                .clicked()
                        {
                            command = Some(CMD_SETTINGS);
                        }
                        ui.add(MenuSeparator);
                        if ui.add(item("Quit", Glyph::Close, &quit_shortcut)).clicked() {
                            command = Some(CMD_QUIT);
                        }
                        command.is_some()
                    });
                menu = menu && !closed.unwrap_or(false);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.add(palette_button()).clicked() {
                        command = Some(CMD_PALETTE);
                    }
                    if has_settings && ui.add(settings_button()).clicked() {
                        command = Some(CMD_SETTINGS);
                    }
                });
            });
        self.app_menu = menu;
        if let Some(command) = command {
            self.run_command(ctx, command, Value::Null);
        }
    }

    fn status_bar(&mut self, ctx: &Context) {
        let colors = Theme::colors(ctx);
        let mut toggled = None;
        let mut widget_command = None;
        egui::TopBottomPanel::bottom("archipel-status-bar")
            .frame(
                egui::Frame::new()
                    .fill(colors.frame)
                    .inner_margin(egui::Margin::symmetric(
                        tokens::size::CONTENT_INSET as i8,
                        4,
                    )),
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label(muted(ui, self.host.status()));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        for widget in self.contributions.status_widgets.iter().rev() {
                            let Some(state) = self.plugin.status_widget(&widget.id) else {
                                continue;
                            };
                            let response = match &widget.command {
                                Some(_) => ui.add(Button::ghost(state.text).small(true)),
                                None => ui.label(state.text),
                            };
                            if let Some(tooltip) = state.tooltip {
                                response.clone().on_hover_text(tooltip);
                            }
                            if response.clicked() {
                                widget_command = widget.command.clone();
                            }
                        }
                        if let Some(task) = self.host.tasks().last() {
                            let extra = self.host.tasks().len() - 1;
                            let mut label = task.title.clone();
                            if let Some(message) = &task.progress.message {
                                label = format!("{label}: {message}");
                            }
                            if extra > 0 {
                                label = format!("{label} (+{extra})");
                            }
                            ui.add(
                                ProgressBar::new(task.progress.fraction)
                                    .label(label.as_str())
                                    .width(120.0),
                            );
                            ui.label(muted(ui, label));
                        }
                        for window in self.contributions.tool_windows.iter().rev() {
                            let open = self.open_tool_windows.contains(&window.id);
                            let label = window.icon_text.as_deref().unwrap_or(&window.title);
                            let mut button = Button::ghost(label).small(true);
                            if open {
                                button = Button::secondary(label).small(true);
                            }
                            if ui.add(button).on_hover_text(&window.title).clicked() {
                                toggled = Some(window.id.clone());
                            }
                        }
                    });
                });
            });
        if let Some(id) = toggled {
            self.toggle_tool_window(&id);
        }
        if let Some(command) = widget_command {
            self.run_command(ctx, &command, Value::Null);
        }
    }

    fn tool_windows(&mut self, ctx: &Context, running: bool) {
        let colors = Theme::colors(ctx);
        let windows: Vec<ToolWindowContribution> = self
            .open_tool_windows
            .iter()
            .filter_map(|id| self.contributions.tool_windows.iter().find(|w| &w.id == id))
            .cloned()
            .collect();
        let gap = tokens::size::ISLAND_GAP as i8;
        let frame = egui::Frame::new().fill(colors.frame).inner_margin(gap);
        let mut closed = Vec::new();
        for window in windows {
            let id = egui::Id::new(("archipel-tool-window", &window.id));
            let title = &self.title;
            let mut body = |ui: &mut Ui| {
                frame_glow(ui, title);
                let mut close = false;
                Panel::new(window.title.as_str())
                    .actions(|ui| {
                        close = ui.add(IconButton::new(Glyph::Close, "Close")).clicked();
                    })
                    .show(ui, |ui| {
                        if running {
                            let view = ViewContext {
                                view: window.view.clone(),
                                placement: ViewPlacement::ToolWindow {
                                    id: window.id.clone(),
                                },
                            };
                            render_view(self.plugin.as_mut(), &mut self.host, &view, ui);
                        }
                    });
                close
            };
            let close = match window.anchor {
                ToolWindowAnchor::Right => {
                    egui::SidePanel::right(id)
                        .default_width(340.0)
                        .frame(frame)
                        .show(ctx, &mut body)
                        .inner
                }
                ToolWindowAnchor::Left => {
                    egui::SidePanel::left(id)
                        .default_width(340.0)
                        .frame(frame)
                        .show(ctx, &mut body)
                        .inner
                }
                ToolWindowAnchor::Bottom => {
                    egui::TopBottomPanel::bottom(id)
                        .default_height(240.0)
                        .frame(frame)
                        .show(ctx, &mut body)
                        .inner
                }
            };
            if close {
                closed.push(window.id);
            }
        }
        self.open_tool_windows.retain(|id| !closed.contains(id));
    }

    fn central(&mut self, ctx: &Context, running: bool) {
        let colors = Theme::colors(ctx);
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(colors.frame))
            .show(ctx, |ui| {
                frame_glow(ui, &self.title);
                if let Some(message) = &self.failed {
                    EmptyState::new("The plugin failed to start")
                        .hint(message.as_str())
                        .show(ui);
                    return;
                }
                if !running {
                    return;
                }
                if !self.tabs.is_empty() {
                    let mut tabs = vec![Tab::new(self.title.as_str())];
                    tabs.extend(self.tabs.iter().map(|t| {
                        let mut tab = Tab::new(t.title.as_str()).closable(true);
                        if matches!(t.kind, TabKind::Settings) {
                            tab = tab.icon(kit::Icon::from(Glyph::Settings));
                        }
                        tab
                    }));
                    let selected = self.current.map_or(0, |i| i + 1);
                    let out = TabBar::new(&tabs, selected).show(ui);
                    if let Some(index) = out.selected {
                        self.current = index.checked_sub(1);
                    }
                    if let Some(index) = out.closed.and_then(|i| i.checked_sub(1)) {
                        self.tabs.remove(index);
                        self.current = match self.current {
                            Some(c) if c == index => index.checked_sub(1).filter(|_| index > 0),
                            Some(c) if c > index => Some(c - 1),
                            other => other,
                        };
                        if self.tabs.is_empty() {
                            self.current = None;
                        }
                    }
                }
                match self.current.and_then(|i| self.tabs.get(i)).cloned() {
                    None => render_view(self.plugin.as_mut(), &mut self.host, &self.app_view, ui),
                    Some(OpenTab {
                        kind: TabKind::Settings,
                        ..
                    }) => {
                        let pages = self.contributions.settings_pages.clone();
                        self.settings_view.show(ui, &pages, &mut self.host);
                    }
                    Some(OpenTab {
                        kind: TabKind::Editor(request),
                        ..
                    }) => {
                        let view = ViewContext {
                            view: request.view.clone(),
                            placement: ViewPlacement::EditorTab {
                                key: request.key.clone(),
                                path: request.path.clone(),
                            },
                        };
                        render_view(self.plugin.as_mut(), &mut self.host, &view, ui);
                    }
                }
            });
    }

    fn toasts(&mut self, ctx: &Context) {
        if let Some(next) = self.host.expire_toasts(Instant::now()) {
            ctx.request_repaint_after(next);
        }
        let mut dismissed = Vec::new();
        for (index, toast) in self.host.toasts().iter().enumerate() {
            let severity = match toast.notification.level {
                archipel_plugin_api::NotificationLevel::Info => Severity::Info,
                archipel_plugin_api::NotificationLevel::Warning => Severity::Warning,
                archipel_plugin_api::NotificationLevel::Error => Severity::Error,
            };
            let mut card = Toast::new(severity, toast.notification.title.as_str());
            if !toast.notification.body.is_empty() {
                card = card.body(toast.notification.body.as_str());
            }
            if card.show_corner(ctx, toast.id, index).dismissed {
                dismissed.push(toast.id);
            }
        }
        for id in dismissed {
            self.host.dismiss_toast(id);
        }
    }
}

fn render_view(plugin: &mut dyn Plugin, host: &mut AppHost, view: &ViewContext, ui: &mut Ui) {
    plugin.render_view(view, &mut ViewFrame::new(ui), host);
    if ui.min_rect().height() < 1.0 {
        empty_state(ui, "Nothing to show", &view.view);
    }
}

/// The app widget of the main toolbar: the app's tile (its initials on its color), its name
/// and a chevron, like Android Studio's project widget.
pub fn app_widget(title: &str, open: bool) -> ToolbarButton<'static> {
    let tile = title.to_owned();
    ToolbarButton::new(title)
        .leading(Slot::ui_width(tokens::toolbar::ICON, move |ui| {
            ui.add(Avatar::new(tile).size(tokens::toolbar::ICON));
        }))
        .dropdown(true)
        .open(open)
}

/// Opens the command palette (Android Studio's Search Everywhere place).
pub fn palette_button() -> IconButton {
    IconButton::new(Glyph::Search, "Command Palette").large(true)
}

/// Opens the settings tab.
pub fn settings_button() -> IconButton {
    IconButton::new(Glyph::Settings, "Settings").large(true)
}

/// Continues the toolbar's glow over the frame of a panel below it, as Android Studio's
/// does over its stripes: every part anchors it to the window, so they meet seamlessly.
fn frame_glow(ui: &Ui, title: &str) {
    let window = ui.ctx().input(|i| i.viewport_rect());
    kit::parts::project_glow(ui.painter(), window, kit::theme::avatar_color(title));
}
