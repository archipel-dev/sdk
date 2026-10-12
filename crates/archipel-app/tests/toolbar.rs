//! The main toolbar of the standalone window, like Android Studio's: conformance of its
//! widgets, the audit and snapshots of the window, and what its controls do, driven by
//! simulated pointer and keyboard input.
//!
//! The snapshots show the toolbar as Linux and Windows draw it. On macOS the same row also
//! holds the native traffic lights (`WindowChrome::Integrated`); those belong to the OS and
//! can't be rendered here.

use archipel_app::api::*;
use archipel_app::{App, AppOptions, SecretBackend, ThemeMode};
use archipel_plugin_testing::kit_testing::{
    self as testing, Harness, NodeT, Queryable, Role, SnapshotOptions, SnapshotResults,
};
use archipel_plugin_ui::kit::components::MenuItem;
use egui::Key;

struct Tool {
    manifest: PluginManifest,
    settings: bool,
}

impl Tool {
    fn new(settings: bool) -> Self {
        Self {
            manifest: PluginManifest::parse(
                "id='dev.test.tool'\nname='Mongo Raabit'\nversion='1'\napi_version=1\n\
                 [app]\nview = 'tool.main'\n",
            )
            .unwrap(),
            settings,
        }
    }
}

impl Plugin for Tool {
    fn manifest(&self) -> &PluginManifest {
        &self.manifest
    }

    fn contributions(&self) -> Contributions {
        let contributions = Contributions::new().command(CommandContribution::new("tool.hi", "Hi"));
        if !self.settings {
            return contributions;
        }
        contributions.settings_page(SettingsPage::new("tool", "Tool").field(SettingField::new(
            "name",
            "Name",
            SettingKind::Text {
                default: "x".into(),
            },
        )))
    }

    fn render_view(&mut self, _: &ViewContext, frame: &mut ViewFrame<'_>, _: &mut dyn PluginHost) {
        if let Some(ui) = archipel_plugin_ui::ui(frame) {
            ui.label("Connections");
        }
    }
}

fn app(name: &str, theme: ThemeMode, settings: bool) -> App {
    let root = std::env::temp_dir().join(format!(
        "archipel-app-toolbar-{name}-{}",
        std::process::id()
    ));
    let options = AppOptions {
        dirs_root: Some(root),
        secrets: SecretBackend::File,
        theme,
        ..AppOptions::new("Mongo Raabit", "dev.test.tool")
    };
    App::new(Tool::new(settings), options).unwrap()
}

/// The whole window at `scale` (snapshots at 2×, input at 1×).
fn window(app: App, scale: f32) -> Harness<'static, App> {
    let mut harness = testing::builder()
        .with_size(egui::vec2(1000.0, 520.0))
        .with_pixels_per_point(scale)
        .build_state(|ctx, app: &mut App| app.update(ctx), app);
    harness.run_ok();
    harness
}

fn screen(name: &str) -> Harness<'static, App> {
    window(app(name, ThemeMode::Dark, true), 1.0)
}

fn open_app_menu(harness: &mut Harness<'_, App>) {
    harness
        .get_by_role_and_label(Role::ComboBox, "Mongo Raabit")
        .click();
    harness.run_ok();
}

#[test]
fn toolbar_widgets_conform() {
    testing::conformance("Mongo Raabit", |ui, enabled| {
        archipel_app::app_widget("Mongo Raabit", false)
            .enabled(enabled)
            .show(ui)
    })
    .run();
    testing::conformance("Command Palette", |ui, enabled| {
        ui.add(archipel_app::palette_button().enabled(enabled))
    })
    .run();
    testing::conformance("Settings", |ui, enabled| {
        ui.add(archipel_app::settings_button().enabled(enabled))
    })
    .run();
    for label in ["Command Palette…", "Settings…", "Quit"] {
        testing::conformance(label, move |ui, enabled| {
            ui.add(MenuItem::new(label).enabled(enabled))
        })
        .run();
    }
}

#[test]
fn the_window_with_its_toolbar_passes_the_audit_in_both_themes() {
    for (name, theme) in [("dark", ThemeMode::Dark), ("light", ThemeMode::Light)] {
        let mut harness = window(app(&format!("audit-{name}"), theme, true), 1.0);
        assert!(
            harness
                .query_by_role_and_label(Role::ComboBox, "Mongo Raabit")
                .is_some()
        );
        testing::audit(&mut harness).assert_clean();
    }
}

#[test]
fn window_snapshots() {
    let mut results = SnapshotResults::new();
    for (name, theme) in [("dark", ThemeMode::Dark), ("light", ThemeMode::Light)] {
        let mut harness = window(app(&format!("snapshot-{name}"), theme, true), 2.0);
        results.add(harness.try_snapshot_options(
            format!("window_toolbar.{name}"),
            &SnapshotOptions::new().output_path("tests/snapshots"),
        ));
        // A real click on the app widget. AccessKit bounds are physical pixels: at 2× the
        // node's center is halved to reach the widget in points.
        let center = harness
            .get_by_role_and_label(Role::ComboBox, "Mongo Raabit")
            .rect()
            .center();
        let at = egui::pos2(center.x / 2.0, center.y / 2.0);
        harness.hover_at(at);
        for pressed in [true, false] {
            harness.event(egui::Event::PointerButton {
                pos: at,
                button: egui::PointerButton::Primary,
                pressed,
                modifiers: egui::Modifiers::NONE,
            });
            harness.step();
        }
        harness.run_ok();
        assert!(harness.state().app_menu_open());
        results.add(harness.try_snapshot_options(
            format!("window_app_menu.{name}"),
            &SnapshotOptions::new().output_path("tests/snapshots"),
        ));
    }
    results.unwrap();
}

#[test]
fn the_app_menu_runs_the_app_commands() {
    let mut harness = screen("menu");
    assert!(harness.query_by_label("Quit").is_none());
    open_app_menu(&mut harness);
    assert!(harness.state().app_menu_open());
    assert!(harness.query_by_label("Command Palette…").is_some());
    harness.get_by_label("Settings…").click();
    harness.run_ok();
    assert!(
        !harness.state().app_menu_open(),
        "picking an item closes it"
    );
    assert!(harness.state().settings_open());
    assert!(harness.state().tab_titles().is_empty());
    harness.key_press(Key::Escape);
    harness.run_ok();
    assert!(!harness.state().settings_open());

    open_app_menu(&mut harness);
    harness.get_by_label("Command Palette…").click();
    harness.run_ok();
    assert!(harness.state().palette_open());

    // Quit asks the window to close.
    let mut harness = screen("quit");
    open_app_menu(&mut harness);
    harness.get_by_label("Quit").click();
    harness.step();
    let closing = harness
        .output()
        .viewport_output
        .values()
        .any(|v| v.commands.contains(&egui::ViewportCommand::Close));
    assert!(closing, "Quit closes the window");
}

#[test]
fn escape_and_a_click_outside_close_the_app_menu() {
    let mut harness = screen("escape");
    open_app_menu(&mut harness);
    harness.key_press(Key::Escape);
    harness.run_ok();
    assert!(!harness.state().app_menu_open());
    open_app_menu(&mut harness);
    harness.get_by_label("Connections").click();
    harness.run_ok();
    assert!(!harness.state().app_menu_open());
    assert!(harness.state().tab_titles().is_empty(), "nothing ran");
}

/// Keyboard only: Tab reaches the app widget and Enter opens its menu.
#[test]
fn the_app_menu_opens_from_the_keyboard() {
    let mut harness = screen("keyboard");
    let focused = |h: &Harness<'_, App>| {
        h.get_by_role_and_label(Role::ComboBox, "Mongo Raabit")
            .accesskit_node()
            .is_focused()
    };
    for _ in 0..30 {
        if focused(&harness) {
            break;
        }
        harness.key_press(Key::Tab);
        harness.run_ok();
    }
    assert!(focused(&harness), "Tab reaches the app widget");
    harness.key_press(Key::Enter);
    harness.run_ok();
    assert!(harness.state().app_menu_open());
}

#[test]
fn the_toolbar_buttons_open_settings_and_the_palette() {
    let mut harness = screen("buttons");
    harness
        .get_by_role_and_label(Role::Button, "Settings")
        .click();
    harness.run_ok();
    assert!(harness.state().settings_open());
    assert!(harness.state().tab_titles().is_empty());
    harness.key_press(Key::Escape);
    harness.run_ok();
    assert!(!harness.state().settings_open());
    harness
        .get_by_role_and_label(Role::Button, "Command Palette")
        .click();
    harness.run_ok();
    assert!(harness.state().palette_open());
}

#[test]
fn an_app_without_settings_has_no_settings_button_or_item() {
    let mut harness = window(app("nosettings", ThemeMode::Dark, false), 1.0);
    assert!(
        harness
            .query_by_role_and_label(Role::Button, "Settings")
            .is_none()
    );
    open_app_menu(&mut harness);
    assert!(harness.query_by_label("Settings…").is_none());
    assert!(harness.query_by_label("Quit").is_some());
}

/// A click on the search button opens the palette and it stays open. (Under xdotool the
/// palette used to close in the frame that opened it, taking the opening press for a click
/// outside; kittest delivers that press a frame earlier, so this checks the behaviour, and
/// the regression was verified with real input under Xvfb.)
#[test]
fn a_click_on_the_search_button_keeps_the_palette_open() {
    let mut harness = screen("click");
    let at = harness
        .get_by_role_and_label(Role::Button, "Command Palette")
        .rect()
        .center();
    harness.hover_at(at);
    harness.step();
    for pressed in [true, false] {
        harness.event(egui::Event::PointerButton {
            pos: at,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        });
    }
    harness.step();
    harness.step();
    assert!(harness.state().palette_open());
}

#[test]
fn settings_dialog_passes_the_audit_and_closes_without_host_tabs() {
    for theme in [ThemeMode::Dark, ThemeMode::Light] {
        let mut harness = window(app("settings-dialog-audit", theme, true), 1.0);
        harness
            .get_by_role_and_label(Role::Button, "Settings")
            .click();
        harness.run_ok();
        assert!(harness.state().settings_open());
        assert!(harness.state().tab_titles().is_empty());
        let mut audit = testing::audit(&mut harness);
        // The modal intentionally keeps the background toolbar outside the Tab cycle.
        audit.findings.retain(|finding| {
            !(finding.problem == testing::Problem::Unreachable
                && [
                    "ComboBox \"Mongo Raabit\"",
                    "Button \"Command Palette\"",
                    "Button \"Settings\"",
                ]
                .contains(&finding.element.as_str()))
        });
        audit.assert_clean();
        harness.get_by_role_and_label(Role::Button, "Close").click();
        harness.run_ok();
        assert!(!harness.state().settings_open());
        assert!(harness.state().tab_titles().is_empty());
    }
}
