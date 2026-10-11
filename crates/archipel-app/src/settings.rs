//! Settings persisted as TOML, validated against the plugin's `SettingsPage` schema.

use archipel_plugin_api::{PluginError, SettingField, SettingsPage, Value};
use std::collections::BTreeMap;
use std::path::PathBuf;

/// The plugin's settings. Unset keys fall back to the defaults of the settings pages.
#[derive(Debug, Default)]
pub struct SettingsStore {
    path: Option<PathBuf>,
    values: BTreeMap<String, Value>,
    fields: Vec<SettingField>,
    plugin: String,
}

impl SettingsStore {
    /// Loads `path` (a missing file is fine; an unreadable or invalid one is reported and
    /// ignored so the app still starts, and is not overwritten until a setting changes).
    pub fn load(
        plugin: &str,
        path: Option<PathBuf>,
        pages: &[SettingsPage],
    ) -> (Self, Option<String>) {
        let mut problem = None;
        let mut values = BTreeMap::new();
        if let Some(path) = &path {
            match std::fs::read_to_string(path) {
                Ok(text) => match toml::from_str::<BTreeMap<String, Value>>(&text) {
                    Ok(parsed) => values = parsed,
                    Err(error) => {
                        problem = Some(format!("{}: {error}", path.display()));
                    }
                },
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => problem = Some(format!("{}: {error}", path.display())),
            }
        }
        let store = Self {
            path,
            values,
            fields: pages.iter().flat_map(|page| page.fields.clone()).collect(),
            plugin: plugin.into(),
        };
        (store, problem)
    }

    /// Replaces the schema, e.g. after `refresh_contributions`.
    pub fn set_pages(&mut self, pages: &[SettingsPage]) {
        self.fields = pages.iter().flat_map(|page| page.fields.clone()).collect();
    }

    pub fn get(&self, key: &str) -> Option<Value> {
        self.values.get(key).cloned().or_else(|| {
            self.fields
                .iter()
                .find(|field| field.key == key)
                .map(|field| field.kind.default_value())
        })
    }

    /// Validates `value` against the field's kind (numbers are clamped) and saves the file.
    pub fn set(&mut self, key: &str, value: Value) -> Result<(), PluginError> {
        let io = |message: String| PluginError::Io(self.plugin.clone(), message);
        if value.is_null() {
            return Err(io(format!("setting `{key}` cannot be null")));
        }
        let value = match self.fields.iter().find(|field| field.key == key) {
            Some(field) => field
                .kind
                .coerce(value)
                .map_err(|e| io(format!("setting `{key}`: {e}")))?,
            None => value,
        };
        let previous = self.values.insert(key.to_owned(), value);
        if let Err(error) = self.save() {
            match previous {
                Some(previous) => self.values.insert(key.to_owned(), previous),
                None => self.values.remove(key),
            };
            return Err(io(error));
        }
        Ok(())
    }

    fn save(&self) -> Result<(), String> {
        let Some(path) = &self.path else {
            return Ok(());
        };
        let text = toml::to_string(&self.values).map_err(|e| e.to_string())?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| format!("{}: {e}", parent.display()))?;
        }
        let tmp = path.with_extension("toml.tmp");
        std::fs::write(&tmp, text).map_err(|e| format!("{}: {e}", tmp.display()))?;
        std::fs::rename(&tmp, path).map_err(|e| format!("{}: {e}", path.display()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use archipel_plugin_api::{SettingKind, json};

    fn pages() -> Vec<SettingsPage> {
        vec![
            SettingsPage::new("p", "P")
                .field(SettingField::new(
                    "greeting",
                    "Greeting",
                    SettingKind::Text {
                        default: "Hello".into(),
                    },
                ))
                .field(SettingField::new(
                    "size",
                    "Size",
                    SettingKind::Number {
                        default: 1.0,
                        min: Some(0.0),
                        max: Some(10.0),
                    },
                )),
        ]
    }

    #[test]
    fn defaults_validation_and_persistence() {
        let dir = std::env::temp_dir().join(format!("archipel-settings-{}", std::process::id()));
        let path = dir.join("config/settings.toml");
        let (mut store, problem) = SettingsStore::load("x.y", Some(path.clone()), &pages());
        assert!(problem.is_none());
        assert_eq!(store.get("greeting"), Some(json!("Hello")));
        assert_eq!(store.get("nope"), None);

        store.set("greeting", json!("Hi")).unwrap();
        store.set("size", json!(99)).unwrap();
        assert_eq!(store.get("size"), Some(json!(10.0)), "clamped");
        assert!(store.set("greeting", json!(3)).is_err());
        assert!(store.set("greeting", Value::Null).is_err());
        assert_eq!(store.get("greeting"), Some(json!("Hi")));

        let (again, problem) = SettingsStore::load("x.y", Some(path.clone()), &pages());
        assert!(problem.is_none());
        assert_eq!(again.get("greeting"), Some(json!("Hi")));
        assert_eq!(again.get("size"), Some(json!(10.0)));

        std::fs::write(&path, "not = [valid").unwrap();
        let (broken, problem) = SettingsStore::load("x.y", Some(path), &pages());
        assert!(problem.is_some());
        assert_eq!(broken.get("greeting"), Some(json!("Hello")));
        let _ = std::fs::remove_dir_all(dir);
    }
}
