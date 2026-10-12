//! Runs the app shell without a window: egui frames with injected input.

use archipel_app::api::*;
use archipel_app::{App, AppOptions, SecretBackend, ThemeMode};
use egui::{Event, Key, Modifiers, RawInput};
use std::path::PathBuf;

struct Probe {
    manifest: PluginManifest,
    activated: bool,
    rendered: Vec<String>,
}

impl Probe {
    fn new(app: bool) -> Self {
        let app = if app {
            "[app]\nview = 'probe.main'\n"
        } else {
            ""
        };
        Self {
            manifest: PluginManifest::parse(&format!(
                "id='dev.test.probe'\nname='Probe'\nversion='1'\napi_version=1\n\
                 permissions=['secrets']\n{app}"
            ))
            .unwrap(),
            activated: false,
            rendered: Vec::new(),
        }
    }
}

impl Plugin for Probe {
    fn manifest(&self) -> &PluginManifest {
        &self.manifest
    }

    fn contributions(&self) -> Contributions {
        Contributions::new()
            .command(CommandContribution::new("probe.tab", "Open Tab"))
            .command(CommandContribution::new("probe.fail", "Fail"))
            .command(CommandContribution::new("probe.task", "Task"))
            .keybinding("probe.tab", "Mod+Alt+T")
            .tool_window(ToolWindowContribution::new("side", "Side", "probe.side"))
            .settings_page(SettingsPage::new("probe", "Probe").field(SettingField::new(
                "name",
                "Name",
                SettingKind::Text {
                    default: "x".into(),
                },
            )))
    }

    fn activate(&mut self, host: &mut dyn PluginHost) -> Result<(), PluginError> {
        self.activated = true;
        host.set_secret("token", "abc")
            .map_err(|e| PluginError::Activation {
                plugin: "probe".into(),
                message: e.to_string(),
            })?;
        host.show_status("active");
        Ok(())
    }

    fn execute(&mut self, command: &str, _: Value, host: &mut dyn PluginHost) -> CommandResult {
        match command {
            "probe.tab" => {
                host.open_editor_tab(EditorTabRequest::new("probe.doc", "Doc").key("doc"));
                Ok(Value::Null)
            }
            "probe.home" => {
                host.focus_app_view();
                Ok(Value::Null)
            }
            "probe.task" => {
                let id = host.begin_task("Work");
                host.update_task(id, TaskProgress::fraction(0.25));
                Ok(Value::Null)
            }
            other => Err(CommandError::Unknown(other.into())),
        }
    }

    fn render_view(
        &mut self,
        view: &ViewContext,
        frame: &mut ViewFrame<'_>,
        _: &mut dyn PluginHost,
    ) {
        if let Some(ui) = archipel_plugin_ui::ui(frame) {
            ui.label(view.view.as_str());
            self.rendered.push(format!("{:?}", view.placement));
        }
    }
}

fn options(name: &str) -> (AppOptions, PathBuf) {
    let root = std::env::temp_dir().join(format!("archipel-app-{name}-{}", std::process::id()));
    let options = AppOptions {
        dirs_root: Some(root.clone()),
        secrets: SecretBackend::File,
        theme: ThemeMode::Dark,
        ..AppOptions::default()
    };
    (options, root)
}

fn frames(app: &mut App, ctx: &egui::Context, n: usize, events: Vec<Event>) {
    for i in 0..n {
        let input = RawInput {
            events: if i == 0 { events.clone() } else { Vec::new() },
            ..RawInput::default()
        };
        let _ = ctx.run(input, |ctx| app.update(ctx));
    }
}

fn key(key: Key, modifiers: Modifiers) -> Vec<Event> {
    // A press and a release: egui marks a press of a key that is still down as a repeat.
    [true, false]
        .map(|pressed| Event::Key {
            key,
            physical_key: None,
            pressed,
            repeat: false,
            modifiers,
        })
        .to_vec()
}

const CTRL_SHIFT: Modifiers = Modifiers {
    ctrl: !cfg!(target_os = "macos"),
    shift: true,
    command: true,
    alt: false,
    mac_cmd: cfg!(target_os = "macos"),
};

#[test]
fn plugins_without_an_app_view_are_rejected() {
    let (options, _) = options("noapp");
    let error = App::new(Probe::new(false), options)
        .err()
        .expect("no [app] view");
    assert!(error.to_string().contains("no `[app]` view"), "{error}");
}

#[test]
fn starts_on_the_second_frame_and_renders_the_app_view() {
    let (options, root) = options("start");
    let mut app = App::new(Probe::new(true), options).unwrap();
    let ctx = egui::Context::default();
    frames(&mut app, &ctx, 1, vec![]);
    assert_eq!(
        app.host().status(),
        "",
        "not activated before the window is painted"
    );
    frames(&mut app, &ctx, 2, vec![]);
    assert_eq!(app.host().status(), "active");
    assert!(app.failure().is_none());
    assert_eq!(app.host().secret("token"), Ok(Some("abc".into())));
    assert!(root.join("data/secrets.toml").exists());
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn the_palette_opens_with_ctrl_shift_a_and_runs_a_command() {
    let (options, root) = options("palette");
    let mut app = App::new(Probe::new(true), options).unwrap();
    let ctx = egui::Context::default();
    frames(&mut app, &ctx, 3, vec![]);
    assert!(!app.palette_open());

    frames(&mut app, &ctx, 2, key(Key::A, CTRL_SHIFT));
    assert!(app.palette_open());

    // Filter by typing, then Enter runs the first match.
    frames(&mut app, &ctx, 1, vec![Event::Text("open tab".into())]);
    frames(&mut app, &ctx, 2, key(Key::Enter, Modifiers::NONE));
    assert!(!app.palette_open());
    frames(&mut app, &ctx, 2, vec![]);
    assert_eq!(app.tab_titles(), ["Doc"]);

    // Escape closes without running anything.
    frames(&mut app, &ctx, 2, key(Key::A, CTRL_SHIFT));
    assert!(app.palette_open());
    frames(&mut app, &ctx, 2, key(Key::Escape, Modifiers::NONE));
    assert!(!app.palette_open());
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn plugin_keybindings_tool_windows_and_errors() {
    let (options, root) = options("misc");
    let mut app = App::new(Probe::new(true), options).unwrap();
    let ctx = egui::Context::default();
    frames(&mut app, &ctx, 3, vec![]);

    let ctrl_alt = Modifiers {
        ctrl: !cfg!(target_os = "macos"),
        alt: true,
        mac_cmd: cfg!(target_os = "macos"),
        command: true,
        ..Modifiers::NONE
    };
    frames(&mut app, &ctx, 2, key(Key::T, ctrl_alt));
    assert_eq!(
        app.tab_titles(),
        ["Doc"],
        "the plugin's keybinding ran its command"
    );

    app.run_command(&ctx, "app.toolWindow.side", Value::Null);
    assert_eq!(app.open_tool_windows(), ["side"]);
    assert!(
        app.wants_repaint(),
        "a built-in command schedules its visible effect"
    );
    frames(&mut app, &ctx, 2, vec![]);
    app.run_command(&ctx, "app.toolWindow.side", Value::Null);
    assert!(app.open_tool_windows().is_empty());

    app.run_command(&ctx, "probe.fail", Value::Null);
    let toast = app.host().toasts().last().expect("an error toast");
    assert_eq!(toast.notification.level, NotificationLevel::Error);

    app.run_command(&ctx, "probe.task", Value::Null);
    assert_eq!(app.host().tasks().len(), 1);
    frames(&mut app, &ctx, 2, vec![]);

    assert!(app.host().is_standalone());
    app.run_command(&ctx, "app.settings", Value::Null);
    assert!(app.wants_repaint());
    assert!(app.settings_open());
    assert_eq!(app.tab_titles(), ["Doc"]);
    frames(&mut app, &ctx, 2, vec![]);
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn a_failing_activation_is_reported_not_fatal() {
    let (mut options, root) = options("fail");
    options.secrets = SecretBackend::File;
    options.dirs_root = None; // no data dir known -> the secret write in `activate` fails
    options.id = String::new();
    let mut plugin = Probe::new(true);
    plugin.manifest.permissions.clear(); // and no `secrets` permission either
    let mut app = App::new(plugin, options).unwrap();
    let ctx = egui::Context::default();
    frames(&mut app, &ctx, 3, vec![]);
    assert!(
        app.failure().is_some_and(|m| m.contains("secrets")),
        "{:?}",
        app.failure()
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn a_plugin_can_return_from_an_editor_to_its_main_view() {
    let (options, root) = options("focus-main");
    let mut app = App::new(Probe::new(true), options).unwrap();
    let ctx = egui::Context::default();
    frames(&mut app, &ctx, 3, vec![]);
    app.run_command(&ctx, "probe.tab", Value::Null);
    frames(&mut app, &ctx, 2, vec![]);
    assert_eq!(app.tab_titles(), ["Doc"]);
    app.run_command(&ctx, "probe.home", Value::Null);
    assert!(app.wants_repaint());
    frames(&mut app, &ctx, 2, vec![]);
    let output = ctx.run(RawInput::default(), |ctx| app.update(ctx));
    let labels: Vec<_> = output
        .shapes
        .iter()
        .filter_map(|shape| {
            if let egui::epaint::Shape::Text(text) = &shape.shape {
                Some(text.galley.text())
            } else {
                None
            }
        })
        .collect();
    assert!(
        labels.contains(&"probe.main"),
        "the main view is painted again"
    );
    assert_eq!(app.tab_titles(), ["Doc"], "editor tabs remain open");
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn settings_are_modal_and_escape_preserves_the_editor() {
    let (options, root) = options("settings-modal");
    let mut app = App::new(Probe::new(true), options).unwrap();
    let ctx = egui::Context::default();
    frames(&mut app, &ctx, 3, vec![]);
    app.run_command(&ctx, "probe.tab", Value::Null);
    frames(&mut app, &ctx, 2, vec![]);
    app.run_command(&ctx, "app.settings", Value::Null);
    frames(&mut app, &ctx, 2, vec![]);
    assert!(app.settings_open());
    assert_eq!(app.tab_titles(), ["Doc"]);
    frames(&mut app, &ctx, 2, key(Key::A, CTRL_SHIFT));
    assert!(
        !app.palette_open(),
        "global shortcuts do not escape the modal"
    );
    frames(&mut app, &ctx, 2, key(Key::Escape, Modifiers::NONE));
    assert!(!app.settings_open());
    assert_eq!(app.tab_titles(), ["Doc"]);
    let _ = std::fs::remove_dir_all(root);
}
