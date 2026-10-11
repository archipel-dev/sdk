//! Declarative contributions: what a plugin adds to the workbench.
//!
//! Contributions are plain data so the host can list them (Plugins page,
//! Search Everywhere, keymap) without running plugin code, and so Lua and
//! WASM plugins can describe the same surface.

use archipel_command::CommandSpec;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct Contributions {
    #[serde(default)]
    pub commands: Vec<CommandContribution>,
    #[serde(default)]
    pub menus: Vec<MenuContribution>,
    #[serde(default)]
    pub keybindings: Vec<KeybindingContribution>,
    #[serde(default)]
    pub settings_pages: Vec<SettingsPage>,
    #[serde(default)]
    pub tool_windows: Vec<ToolWindowContribution>,
    #[serde(default)]
    pub editors: Vec<EditorContribution>,
    #[serde(default)]
    pub status_widgets: Vec<StatusWidgetContribution>,
}

impl Contributions {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn command(mut self, command: CommandContribution) -> Self {
        self.commands.push(command);
        self
    }

    pub fn menu(mut self, menu: MenuLocation, command: impl Into<String>) -> Self {
        self.menus.push(MenuContribution {
            menu,
            command: command.into(),
        });
        self
    }

    /// Binds `key` (see [`KeyChord::parse`]) to `command`.
    pub fn keybinding(mut self, command: impl Into<String>, key: impl Into<String>) -> Self {
        self.keybindings.push(KeybindingContribution {
            command: command.into(),
            key: key.into(),
        });
        self
    }

    pub fn settings_page(mut self, page: SettingsPage) -> Self {
        self.settings_pages.push(page);
        self
    }

    pub fn tool_window(mut self, window: ToolWindowContribution) -> Self {
        self.tool_windows.push(window);
        self
    }

    pub fn editor(mut self, editor: EditorContribution) -> Self {
        self.editors.push(editor);
        self
    }

    pub fn status_widget(mut self, widget: StatusWidgetContribution) -> Self {
        self.status_widgets.push(widget);
        self
    }

    /// Command specs for the shared registry; the first valid keybinding of a
    /// command becomes its displayed shortcut.
    pub fn command_specs(&self) -> Vec<CommandSpec> {
        self.commands
            .iter()
            .map(|command| CommandSpec {
                id: command.id.clone(),
                label: command.label.clone(),
                category: command.category.clone(),
                shortcut: self
                    .keybindings
                    .iter()
                    .filter(|binding| binding.command == command.id)
                    .find_map(|binding| KeyChord::parse(&binding.key).ok())
                    .map(|chord| chord.mac_notation()),
                params: command.params.clone(),
            })
            .collect()
    }

    /// Checks internal consistency: unique ids and references to declared
    /// commands. Returns human-readable problems.
    pub fn problems(&self) -> Vec<String> {
        let mut problems = Vec::new();
        let mut seen = std::collections::BTreeSet::new();
        for command in &self.commands {
            if !seen.insert(command.id.as_str()) {
                problems.push(format!("duplicate command `{}`", command.id));
            }
            if !command.id.contains('.') {
                problems.push(format!(
                    "command `{}` must be namespaced (`plugin.action`)",
                    command.id
                ));
            }
        }
        let declared = |id: &str| self.commands.iter().any(|command| command.id == id);
        for menu in &self.menus {
            if !declared(&menu.command) {
                problems.push(format!(
                    "menu item refers to unknown command `{}`",
                    menu.command
                ));
            }
        }
        for binding in &self.keybindings {
            if !declared(&binding.command) {
                problems.push(format!(
                    "keybinding refers to unknown command `{}`",
                    binding.command
                ));
            }
            if let Err(error) = KeyChord::parse(&binding.key) {
                problems.push(error);
            }
        }
        for widget in &self.status_widgets {
            if let Some(command) = &widget.command
                && !declared(command)
            {
                problems.push(format!(
                    "status widget `{}` refers to unknown command `{command}`",
                    widget.id
                ));
            }
        }
        problems
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CommandContribution {
    /// Globally unique, namespaced id (`nexus.sendRequest`).
    pub id: String,
    pub label: String,
    /// Search Everywhere / menu category. Defaults to `Tools`.
    #[serde(default = "default_category")]
    pub category: String,
    /// Parameter documentation in the registry's convention: a JSON object of
    /// field → type, optional fields end in `?`.
    #[serde(default = "default_params")]
    pub params: String,
}

fn default_category() -> String {
    "Tools".into()
}

fn default_params() -> String {
    "{}".into()
}

impl CommandContribution {
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            category: default_category(),
            params: default_params(),
        }
    }

    pub fn category(mut self, category: impl Into<String>) -> Self {
        self.category = category.into();
        self
    }

    pub fn params(mut self, params: impl Into<String>) -> Self {
        self.params = params.into();
        self
    }
}

/// Where a command appears in the main menu.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum MenuLocation {
    File,
    Edit,
    View,
    Navigate,
    Code,
    Build,
    Run,
    Git,
    Tools,
    Help,
}

impl MenuLocation {
    pub fn title(self) -> &'static str {
        match self {
            MenuLocation::File => "File",
            MenuLocation::Edit => "Edit",
            MenuLocation::View => "View",
            MenuLocation::Navigate => "Navigate",
            MenuLocation::Code => "Code",
            MenuLocation::Build => "Build",
            MenuLocation::Run => "Run",
            MenuLocation::Git => "Git",
            MenuLocation::Tools => "Tools",
            MenuLocation::Help => "Help",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MenuContribution {
    pub menu: MenuLocation,
    pub command: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct KeybindingContribution {
    pub command: String,
    /// `Mod+Shift+K` (Mod = ⌘ on macOS, Ctrl elsewhere) or macOS glyph
    /// notation `⇧⌘K`.
    pub key: String,
}

/// A parsed key chord. `command` is the platform primary modifier
/// (⌘ on macOS, Ctrl on Windows/Linux).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct KeyChord {
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
    pub command: bool,
    /// Key name: a single uppercase letter/digit, `F1`…`F12`, or a named key
    /// (`Enter`, `Space`, `Tab`, `Escape`, `ArrowUp`, …).
    pub key: String,
}

impl KeyChord {
    pub fn parse(input: &str) -> Result<Self, String> {
        let error = || format!("invalid key chord `{input}`");
        let mut chord = KeyChord {
            ctrl: false,
            alt: false,
            shift: false,
            command: false,
            key: String::new(),
        };
        let trimmed = input.trim();
        if trimmed.is_empty() {
            return Err(error());
        }
        let key = if trimmed.contains('+') && trimmed.len() > 1 {
            let mut parts: Vec<&str> = trimmed.split('+').map(str::trim).collect();
            let key = parts
                .pop()
                .filter(|key| !key.is_empty())
                .ok_or_else(error)?;
            for part in parts {
                match part.to_ascii_lowercase().as_str() {
                    "mod" | "cmd" | "command" | "meta" => chord.command = true,
                    "ctrl" | "control" => chord.ctrl = true,
                    "alt" | "option" | "opt" => chord.alt = true,
                    "shift" => chord.shift = true,
                    _ => return Err(error()),
                }
            }
            key.to_owned()
        } else {
            let mut rest = trimmed;
            while let Some(c) = rest.chars().next() {
                match c {
                    '⌃' => chord.ctrl = true,
                    '⌥' => chord.alt = true,
                    '⇧' => chord.shift = true,
                    '⌘' => chord.command = true,
                    _ => break,
                }
                rest = &rest[c.len_utf8()..];
            }
            rest.to_owned()
        };
        chord.key = normalize_key(&key).ok_or_else(error)?;
        Ok(chord)
    }

    /// macOS glyph notation used by command metadata (`⇧⌘K`).
    pub fn mac_notation(&self) -> String {
        let mut out = String::new();
        if self.ctrl {
            out.push('⌃');
        }
        if self.alt {
            out.push('⌥');
        }
        if self.shift {
            out.push('⇧');
        }
        if self.command {
            out.push('⌘');
        }
        out.push_str(&self.key);
        out
    }
}

fn normalize_key(key: &str) -> Option<String> {
    let mut chars = key.chars();
    if let (Some(c), None) = (chars.next(), chars.next()) {
        return (c.is_ascii_alphanumeric()).then(|| c.to_ascii_uppercase().to_string());
    }
    let lower = key.to_ascii_lowercase();
    if let Some(number) = lower.strip_prefix('f')
        && let Ok(n) = number.parse::<u8>()
    {
        return (1..=20).contains(&n).then(|| format!("F{n}"));
    }
    let named = match lower.as_str() {
        "enter" | "return" => "Enter",
        "space" => "Space",
        "tab" => "Tab",
        "escape" | "esc" => "Escape",
        "backspace" => "Backspace",
        "delete" | "del" => "Delete",
        "up" | "arrowup" => "ArrowUp",
        "down" | "arrowdown" => "ArrowDown",
        "left" | "arrowleft" => "ArrowLeft",
        "right" | "arrowright" => "ArrowRight",
        "home" => "Home",
        "end" => "End",
        "pageup" => "PageUp",
        "pagedown" => "PageDown",
        _ => return None,
    };
    Some(named.into())
}

/// A page under Settings. Fields are rendered by the host and stored in the
/// plugin's settings ([`crate::PluginHost::setting`]).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SettingsPage {
    pub id: String,
    pub title: String,
    #[serde(default)]
    pub fields: Vec<SettingField>,
}

impl SettingsPage {
    pub fn new(id: impl Into<String>, title: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            fields: Vec::new(),
        }
    }

    pub fn field(mut self, field: SettingField) -> Self {
        self.fields.push(field);
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SettingField {
    /// Settings key, unique within the plugin.
    pub key: String,
    pub label: String,
    #[serde(default)]
    pub description: Option<String>,
    pub kind: SettingKind,
}

impl SettingField {
    pub fn new(key: impl Into<String>, label: impl Into<String>, kind: SettingKind) -> Self {
        Self {
            key: key.into(),
            label: label.into(),
            description: None,
            kind,
        }
    }

    pub fn description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum SettingKind {
    Text {
        default: String,
    },
    Bool {
        default: bool,
    },
    Number {
        default: f64,
        min: Option<f64>,
        max: Option<f64>,
    },
    Choice {
        options: Vec<String>,
        default: String,
    },
}

impl SettingKind {
    pub fn default_value(&self) -> Value {
        match self {
            SettingKind::Text { default } => Value::from(default.clone()),
            SettingKind::Bool { default } => Value::from(*default),
            SettingKind::Number { default, .. } => Value::from(*default),
            SettingKind::Choice { default, .. } => Value::from(default.clone()),
        }
    }

    /// Validates a value entered by the user; numbers are clamped to the range.
    pub fn coerce(&self, value: Value) -> Result<Value, String> {
        match (self, value) {
            (SettingKind::Text { .. }, Value::String(text)) => Ok(Value::String(text)),
            (SettingKind::Bool { .. }, Value::Bool(flag)) => Ok(Value::Bool(flag)),
            (SettingKind::Number { min, max, .. }, Value::Number(number)) => {
                let mut n = number.as_f64().ok_or("not a number")?;
                if let Some(min) = min {
                    n = n.max(*min);
                }
                if let Some(max) = max {
                    n = n.min(*max);
                }
                Ok(Value::from(n))
            }
            (SettingKind::Choice { options, .. }, Value::String(choice))
                if options.contains(&choice) =>
            {
                Ok(Value::String(choice))
            }
            (kind, value) => Err(format!("{value} is not a valid value for {kind:?}")),
        }
    }
}

/// Where a plugin tool window docks.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum ToolWindowAnchor {
    #[default]
    Right,
    Bottom,
    Left,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ToolWindowContribution {
    pub id: String,
    pub title: String,
    #[serde(default)]
    pub anchor: ToolWindowAnchor,
    /// View rendered inside the tool window.
    pub view: String,
    /// One or two characters shown as the stripe icon when no icon is known.
    #[serde(default)]
    pub icon_text: Option<String>,
}

impl ToolWindowContribution {
    pub fn new(id: impl Into<String>, title: impl Into<String>, view: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            anchor: ToolWindowAnchor::Right,
            view: view.into(),
            icon_text: None,
        }
    }

    pub fn anchor(mut self, anchor: ToolWindowAnchor) -> Self {
        self.anchor = anchor;
        self
    }

    pub fn icon_text(mut self, text: impl Into<String>) -> Self {
        self.icon_text = Some(text.into());
        self
    }
}

/// An editor-area view: opened as a tab via
/// [`crate::PluginHost::open_editor_tab`], or for files with one of
/// `file_extensions` (custom editors).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EditorContribution {
    pub view: String,
    pub title: String,
    #[serde(default)]
    pub file_extensions: Vec<String>,
}

impl EditorContribution {
    pub fn new(view: impl Into<String>, title: impl Into<String>) -> Self {
        Self {
            view: view.into(),
            title: title.into(),
            file_extensions: Vec::new(),
        }
    }

    pub fn file_extension(mut self, extension: impl Into<String>) -> Self {
        self.file_extensions.push(extension.into());
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StatusWidgetContribution {
    pub id: String,
    /// Command run when the widget is clicked.
    #[serde(default)]
    pub command: Option<String>,
}

impl StatusWidgetContribution {
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            command: None,
        }
    }

    pub fn command(mut self, command: impl Into<String>) -> Self {
        self.command = Some(command.into());
        self
    }
}

/// Current text of a status bar widget ([`crate::Plugin::status_widget`]).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct StatusWidgetState {
    pub text: String,
    #[serde(default)]
    pub tooltip: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn key_chords_accept_portable_and_mac_notation() {
        let portable = KeyChord::parse("Mod+Shift+k").unwrap();
        let mac = KeyChord::parse("⇧⌘K").unwrap();
        assert_eq!(portable, mac);
        assert_eq!(portable.mac_notation(), "⇧⌘K");
        assert_eq!(
            KeyChord::parse("Ctrl+Alt+F12").unwrap().mac_notation(),
            "⌃⌥F12"
        );
        assert_eq!(KeyChord::parse("Alt+Enter").unwrap().key, "Enter");
        for bad in ["", "Mod+", "Hyper+K", "Mod+Shift+Banana", "F99"] {
            assert!(KeyChord::parse(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn contributions_produce_specs_and_report_problems() {
        let contributions = Contributions::new()
            .command(CommandContribution::new("hello.say", "Say Hello"))
            .keybinding("hello.say", "Mod+Alt+H")
            .menu(MenuLocation::Tools, "hello.say")
            .menu(MenuLocation::Tools, "hello.missing");
        let specs = contributions.command_specs();
        assert_eq!(specs[0].shortcut.as_deref(), Some("⌥⌘H"));
        assert_eq!(specs[0].category, "Tools");
        assert_eq!(
            contributions.problems(),
            ["menu item refers to unknown command `hello.missing`"]
        );
    }

    #[test]
    fn setting_values_are_validated() {
        let number = SettingKind::Number {
            default: 1.0,
            min: Some(0.0),
            max: Some(10.0),
        };
        assert_eq!(number.coerce(Value::from(42)).unwrap(), Value::from(10.0));
        assert!(number.coerce(Value::from("x")).is_err());
        let choice = SettingKind::Choice {
            options: vec!["a".into(), "b".into()],
            default: "a".into(),
        };
        assert!(choice.coerce(Value::from("c")).is_err());
        assert_eq!(choice.default_value(), Value::from("a"));
    }
}
