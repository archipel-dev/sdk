//! Example Archipel plugin. It uses every contribution point of plugin API 1 and is the
//! reference for `docs/api.md` and `docs/getting-started.md`.
//!
//! The view is built only from Archipel Kit components, through `archipel_sdk::ui`.

use archipel_sdk::api::{
    CommandContribution, CommandError, CommandResult, Contributions, EditorContribution,
    EditorTabRequest, MenuLocation, Notification, Plugin, PluginError, PluginHost, PluginManifest,
    SettingField, SettingKind, SettingsPage, StatusWidgetContribution, StatusWidgetState,
    ToolWindowContribution, Value, ViewContext, ViewFrame, json,
};
use archipel_sdk::kit::components::{Button, TextField};
use archipel_sdk::kit::egui;
use archipel_sdk::ui;

pub const ID: &str = "dev.archipel.hello";

pub struct HelloPlugin {
    manifest: PluginManifest,
    greetings: u32,
    name: String,
}

impl Default for HelloPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl HelloPlugin {
    pub fn new() -> Self {
        Self {
            manifest: PluginManifest::parse(include_str!("../archipel-plugin.toml"))
                .expect("bundled manifest is valid"),
            greetings: 0,
            name: "world".into(),
        }
    }

    fn greeting(&self, host: &dyn PluginHost) -> String {
        let word = host
            .setting("greeting")
            .and_then(|value| value.as_str().map(str::to_owned))
            .unwrap_or_else(|| "Hello".into());
        format!("{word}, {}!", self.name)
    }

    fn say_hello(&mut self, host: &mut dyn PluginHost) -> String {
        self.greetings += 1;
        let message = self.greeting(host);
        host.show_status(&message);
        if host
            .setting("notify")
            .and_then(|v| v.as_bool())
            .unwrap_or(true)
        {
            host.notify(Notification::info("Hello", message.clone()));
        }
        message
    }

    fn main_view(&mut self, ui: &mut egui::Ui, host: &mut dyn PluginHost) {
        ui::header(ui, "Hello", |_| {});
        ui.add(TextField::new(&mut self.name).label("Name"));
        ui.add_space(6.0);
        if ui.add(Button::primary("Say Hello")).clicked() {
            self.say_hello(host);
        }
        ui::section(ui, "Statistics");
        ui::property(ui, "Greetings", &self.greetings.to_string());
        ui::property(
            ui,
            "Project",
            &host
                .project_root()
                .map(|root| root.display().to_string())
                .unwrap_or_else(|| "none".into()),
        );
    }
}

impl Plugin for HelloPlugin {
    fn manifest(&self) -> &PluginManifest {
        &self.manifest
    }

    fn contributions(&self) -> Contributions {
        Contributions::new()
            .command(CommandContribution::new("hello.sayHello", "Say Hello"))
            .command(
                CommandContribution::new("hello.greet", "Greet Someone")
                    .params(r#"{"name":"string"}"#),
            )
            .command(CommandContribution::new("hello.openTab", "Open Hello Tab"))
            .menu(MenuLocation::Tools, "hello.sayHello")
            .menu(MenuLocation::Tools, "hello.openTab")
            .keybinding("hello.sayHello", "Mod+Alt+H")
            .tool_window(ToolWindowContribution::new("hello", "Hello", "hello.main").icon_text("H"))
            .editor(EditorContribution::new("hello.main", "Hello").file_extension("hello"))
            .settings_page(
                SettingsPage::new("hello", "Hello")
                    .field(SettingField::new(
                        "greeting",
                        "Greeting",
                        SettingKind::Text {
                            default: "Hello".into(),
                        },
                    ))
                    .field(
                        SettingField::new(
                            "notify",
                            "Show notification",
                            SettingKind::Bool { default: true },
                        )
                        .description("Show a balloon in addition to the status bar message."),
                    ),
            )
            .status_widget(StatusWidgetContribution::new("hello.count").command("hello.sayHello"))
    }

    fn activate(&mut self, host: &mut dyn PluginHost) -> Result<(), PluginError> {
        host.show_status("Hello plugin activated");
        Ok(())
    }

    fn execute(
        &mut self,
        command: &str,
        params: Value,
        host: &mut dyn PluginHost,
    ) -> CommandResult {
        match command {
            "hello.sayHello" => Ok(json!({"message": self.say_hello(host)})),
            "hello.greet" => {
                let name = params
                    .get("name")
                    .and_then(Value::as_str)
                    .filter(|name| !name.trim().is_empty())
                    .ok_or_else(|| {
                        CommandError::InvalidParams("expected string field `name`".into())
                    })?;
                self.name = name.trim().to_owned();
                Ok(json!({"message": self.say_hello(host)}))
            }
            "hello.openTab" => {
                host.open_editor_tab(EditorTabRequest::new("hello.main", "Hello"));
                Ok(Value::Null)
            }
            other => Err(CommandError::Unknown(other.into())),
        }
    }

    fn render_view(
        &mut self,
        view: &ViewContext,
        frame: &mut ViewFrame<'_>,
        host: &mut dyn PluginHost,
    ) {
        let Some(ui) = ui::ui(frame) else {
            return;
        };
        match view.view.as_str() {
            "hello.main" => self.main_view(ui, host),
            other => ui::empty_state(ui, "Unknown view", other),
        }
    }

    fn status_widget(&self, id: &str) -> Option<StatusWidgetState> {
        (id == "hello.count").then(|| StatusWidgetState {
            text: format!("Hello × {}", self.greetings),
            tooltip: Some("Greetings so far — click to say hello".into()),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use archipel_sdk::api::ViewPlacement;
    use archipel_sdk::kit::{Theme, components::Button};
    use archipel_sdk::testing::{
        Fixture, NodeExt, Queryable, RecordingHost, audit, conformance, view_harness,
    };

    fn app_view() -> ViewContext {
        ViewContext {
            view: "hello.main".into(),
            placement: ViewPlacement::App,
        }
    }

    #[test]
    fn contributions_are_consistent() {
        let plugin = HelloPlugin::new();
        assert_eq!(plugin.manifest().id, ID);
        assert!(plugin.contributions().problems().is_empty());
        let specs = plugin.contributions().command_specs();
        let say = specs
            .iter()
            .find(|spec| spec.id == "hello.sayHello")
            .expect("hello.sayHello is contributed");
        assert_eq!(say.shortcut.as_deref(), Some("⌥⌘H"));
    }

    #[test]
    fn commands_use_settings_and_host_services() {
        let mut plugin = HelloPlugin::new();
        let mut host = RecordingHost::new(ID);
        host.settings.insert("greeting".into(), json!("Hi"));
        host.settings.insert("notify".into(), json!(false));
        let result = plugin
            .execute("hello.greet", json!({"name": " Ada "}), &mut host)
            .expect("valid params");
        assert_eq!(result["message"], "Hi, Ada!");
        assert_eq!(host.statuses, ["Hi, Ada!"]);
        assert!(host.notifications.is_empty());
        assert!(matches!(
            plugin.execute("hello.greet", json!({}), &mut host),
            Err(CommandError::InvalidParams(_))
        ));
        assert!(matches!(
            plugin.execute("hello.nope", Value::Null, &mut host),
            Err(CommandError::Unknown(_))
        ));
        plugin
            .execute("hello.openTab", Value::Null, &mut host)
            .expect("opens a tab");
        assert_eq!(host.editor_tabs[0].view, "hello.main");
        assert_eq!(
            plugin.status_widget("hello.count").map(|w| w.text),
            Some("Hello × 1".into())
        );
    }

    #[test]
    fn view_greets_when_the_button_is_clicked() {
        let fixture = Fixture::new(HelloPlugin::new(), ID);
        let mut harness = view_harness(Theme::dark(), fixture, app_view());

        let name = harness.get_by_label("Name");
        name.focus();
        name.type_text("!");
        harness.get_by_label("Say Hello").click();
        harness.run();

        let state = harness.state();
        assert_eq!(state.plugin.greetings, 1);
        assert_eq!(state.host.statuses, ["Hello, world!!"]);
        assert_eq!(state.host.notifications.len(), 1);
        assert!(harness.query_by_label("Greetings").is_some());
    }

    #[test]
    fn view_passes_the_audit_in_both_themes() {
        for theme in [Theme::dark(), Theme::light()] {
            let fixture = Fixture::new(HelloPlugin::new(), ID);
            let mut harness = view_harness(theme, fixture, app_view());
            audit(&mut harness).assert_clean();
        }
    }

    #[test]
    fn unknown_views_render_a_placeholder() {
        let fixture = Fixture::new(HelloPlugin::new(), ID);
        let view = ViewContext {
            view: "hello.nope".into(),
            ..app_view()
        };
        let harness = view_harness(Theme::dark(), fixture, view);
        assert!(harness.query_by_label("Unknown view").is_some());
    }

    #[test]
    fn say_hello_button_conforms_to_the_kit() {
        conformance("Say Hello", |ui, enabled| {
            ui.add(Button::primary("Say Hello").enabled(enabled))
        })
        .run();
    }

    #[test]
    fn disabled_state_is_reported() {
        let mut harness = archipel_sdk::testing::kit_testing::harness(Theme::dark(), |ui| {
            ui.add(Button::primary("Say Hello").enabled(false));
        });
        assert!(harness.get_by_label("Say Hello").is_disabled());
        harness.run();
    }
}
