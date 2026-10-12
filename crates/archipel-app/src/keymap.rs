//! Keymaps: chords bound to commands, from the plugin, the app options and `keymap.toml`.

use archipel_plugin_api::{Contributions, KeyChord};
use std::collections::BTreeMap;
use std::path::Path;

/// Chord → command bindings. Later layers win: plugin contributions, then
/// `AppOptions::keybindings`, then the user's `keymap.toml`.
#[derive(Debug, Clone, Default)]
pub struct Keymap {
    bindings: Vec<(KeyChord, String)>,
}

impl Keymap {
    /// Binds `chord` to `command`, replacing a binding of the same chord.
    pub fn bind(&mut self, chord: KeyChord, command: impl Into<String>) {
        self.bindings.retain(|(existing, _)| *existing != chord);
        self.bindings.push((chord, command.into()));
    }

    /// Like [`Keymap::bind`], parsing `chord` (`Mod+Shift+K` or `⇧⌘K`).
    pub fn bind_str(&mut self, chord: &str, command: &str) -> Result<(), String> {
        self.bind(KeyChord::parse(chord)?, command);
        Ok(())
    }

    pub fn from_contributions(contributions: &Contributions) -> (Self, Vec<String>) {
        let mut keymap = Self::default();
        let mut problems = Vec::new();
        for binding in &contributions.keybindings {
            if let Err(error) = keymap.bind_str(&binding.key, &binding.command) {
                problems.push(error);
            }
        }
        (keymap, problems)
    }

    /// Applies the user's `keymap.toml`: `"command.id" = "Mod+K"` pairs under `[bindings]`.
    /// An empty chord removes the command's bindings. A missing file is fine; the problems
    /// of an invalid one or of single entries are returned.
    pub fn apply_user_file(&mut self, path: &Path) -> Vec<String> {
        let text = match std::fs::read_to_string(path) {
            Ok(text) => text,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Vec::new(),
            Err(e) => return vec![format!("{}: {e}", path.display())],
        };
        #[derive(serde::Deserialize)]
        struct File {
            #[serde(default)]
            bindings: BTreeMap<String, String>,
        }
        let file: File = match toml::from_str(&text) {
            Ok(file) => file,
            Err(e) => return vec![format!("{}: {e}", path.display())],
        };
        let mut problems = Vec::new();
        for (command, chord) in file.bindings {
            if chord.trim().is_empty() {
                self.bindings.retain(|(_, bound)| *bound != command);
            } else if let Err(error) = self.bind_str(&chord, &command) {
                problems.push(format!("{}: {error}", path.display()));
            }
        }
        problems
    }

    /// The command bound to the chord pressed in `modifiers` + `key`.
    pub fn command_for(&self, key: egui::Key, modifiers: egui::Modifiers) -> Option<&str> {
        let mac = cfg!(target_os = "macos");
        self.bindings
            .iter()
            .find(|(chord, _)| chord_matches(chord, key, modifiers, mac))
            .map(|(_, command)| command.as_str())
    }

    /// The first chord bound to `command`, in the notation shown to users.
    pub fn shortcut_of(&self, command: &str) -> Option<String> {
        self.bindings
            .iter()
            .find(|(_, bound)| bound == command)
            .map(|(chord, _)| display_chord(chord, cfg!(target_os = "macos")))
    }

    pub fn bindings(&self) -> impl Iterator<Item = (&KeyChord, &str)> {
        self.bindings
            .iter()
            .map(|(c, command)| (c, command.as_str()))
    }
}

/// Whether the key press matches `chord`. `command` is the platform primary modifier: ⌘ on
/// macOS, Ctrl elsewhere (where `ctrl` and `command` are the same physical key).
pub fn chord_matches(chord: &KeyChord, key: egui::Key, m: egui::Modifiers, mac: bool) -> bool {
    let Some(wanted) = egui::Key::from_name(&chord.key) else {
        return false;
    };
    let mods = if mac {
        m.mac_cmd == chord.command && m.ctrl == chord.ctrl
    } else {
        m.ctrl == (chord.ctrl || chord.command)
    };
    key == wanted && mods && m.alt == chord.alt && m.shift == chord.shift
}

/// `Ctrl+Shift+A` on Linux and Windows, `⇧⌘A` on macOS.
pub fn display_chord(chord: &KeyChord, mac: bool) -> String {
    if mac {
        return chord.mac_notation();
    }
    let mut parts = Vec::new();
    if chord.ctrl || chord.command {
        parts.push("Ctrl");
    }
    if chord.alt {
        parts.push("Alt");
    }
    if chord.shift {
        parts.push("Shift");
    }
    parts.push(chord.key.as_str());
    parts.join("+")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mods(ctrl: bool, shift: bool, alt: bool) -> egui::Modifiers {
        egui::Modifiers {
            ctrl,
            shift,
            alt,
            command: ctrl,
            ..Default::default()
        }
    }

    #[test]
    fn mod_is_ctrl_off_macos_and_cmd_on_macos() {
        let chord = KeyChord::parse("Mod+Shift+A").unwrap();
        assert!(chord_matches(
            &chord,
            egui::Key::A,
            mods(true, true, false),
            false
        ));
        assert!(!chord_matches(
            &chord,
            egui::Key::A,
            mods(true, false, false),
            false
        ));
        assert!(!chord_matches(
            &chord,
            egui::Key::B,
            mods(true, true, false),
            false
        ));
        let cmd = egui::Modifiers {
            mac_cmd: true,
            command: true,
            shift: true,
            ..Default::default()
        };
        assert!(chord_matches(&chord, egui::Key::A, cmd, true));
        assert!(!chord_matches(
            &chord,
            egui::Key::A,
            mods(true, true, false),
            true
        ));
        assert_eq!(display_chord(&chord, false), "Ctrl+Shift+A");
        assert_eq!(display_chord(&chord, true), "⇧⌘A");
    }

    #[test]
    fn layers_override_and_the_user_file_can_unbind() {
        let contributions = Contributions::new()
            .keybinding("a.one", "Mod+K")
            .keybinding("a.bad", "Hyper+K");
        let (mut keymap, problems) = Keymap::from_contributions(&contributions);
        assert_eq!(problems.len(), 1);
        keymap.bind_str("Mod+K", "a.two").unwrap();
        let primary = egui::Modifiers {
            ctrl: !cfg!(target_os = "macos"),
            mac_cmd: cfg!(target_os = "macos"),
            command: true,
            ..egui::Modifiers::NONE
        };
        assert_eq!(keymap.command_for(egui::Key::K, primary), Some("a.two"));

        let path =
            std::env::temp_dir().join(format!("archipel-keymap-{}.toml", std::process::id()));
        std::fs::write(
            &path,
            "[bindings]\n\"a.two\" = \"\"\n\"a.three\" = \"Alt+J\"\n\"a.x\" = \"nope+\"\n",
        )
        .unwrap();
        let problems = keymap.apply_user_file(&path);
        assert_eq!(problems.len(), 1);
        assert_eq!(keymap.command_for(egui::Key::K, primary), None);
        assert_eq!(
            keymap.command_for(egui::Key::J, mods(false, false, true)),
            Some("a.three")
        );
        assert_eq!(
            keymap.shortcut_of("a.three").as_deref(),
            Some(if cfg!(target_os = "macos") {
                "⌥J"
            } else {
                "Alt+J"
            })
        );
        assert_eq!(keymap.shortcut_of("a.two"), None);
        let _ = std::fs::remove_file(path);
        assert!(
            keymap
                .apply_user_file(Path::new("/nonexistent/keymap.toml"))
                .is_empty()
        );
    }
}
