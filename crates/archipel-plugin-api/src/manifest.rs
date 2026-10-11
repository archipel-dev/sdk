//! `archipel-plugin.toml`: identity, engine compatibility, entry point,
//! permissions and dependencies of a plugin.

use crate::PluginError;
use serde::{Deserialize, Serialize};

/// Manifest file name inside a plugin directory or archive.
pub const MANIFEST_FILE: &str = "archipel-plugin.toml";
/// Name used before the rename to Archipel; still read, with the same contents.
pub const JETTHING_MANIFEST_FILE: &str = "jetthing-plugin.toml";
/// Pre-API-1 manifest name, still discovered for compatibility.
pub const LEGACY_MANIFEST_FILE: &str = "plugin.toml";
/// Every accepted manifest file name, most preferred first. When a directory holds more
/// than one, hosts read the first that exists.
pub const MANIFEST_FILES: [&str; 3] = [MANIFEST_FILE, JETTHING_MANIFEST_FILE, LEGACY_MANIFEST_FILE];

/// How a plugin's code is loaded.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "lowercase")]
pub enum PluginKind {
    /// Manifest-only plugin: metadata and capabilities, no executable code.
    #[default]
    Declarative,
    /// Rust crate compiled into the host (first-party plugins under `plugins/`).
    Rust,
    /// Lua script loaded through the embedded Lua runtime.
    Lua,
    /// WebAssembly component loaded in the sandboxed runtime.
    Wasm,
}

impl PluginKind {
    pub fn label(self) -> &'static str {
        match self {
            PluginKind::Declarative => "declarative",
            PluginKind::Rust => "rust",
            PluginKind::Lua => "lua",
            PluginKind::Wasm => "wasm",
        }
    }
}

/// Capabilities a plugin must declare before the host grants the matching
/// services. Bundled Rust plugins are trusted but still declare what they use
/// so the Plugins page can show it.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Permission {
    /// Outgoing network connections (HTTP clients, database drivers).
    #[serde(rename = "network")]
    Network,
    /// Spawning external processes.
    #[serde(rename = "process")]
    Process,
    /// Reading and writing files inside the open project.
    #[serde(rename = "fs.project")]
    ProjectFiles,
    /// Reading and writing files anywhere the user can.
    #[serde(rename = "fs.user")]
    UserFiles,
    /// Reading and writing the system clipboard.
    #[serde(rename = "clipboard")]
    Clipboard,
    /// Storing credentials in the host's secret storage.
    #[serde(rename = "secrets")]
    Secrets,
}

impl Permission {
    pub const ALL: [Permission; 6] = [
        Permission::Network,
        Permission::Process,
        Permission::ProjectFiles,
        Permission::UserFiles,
        Permission::Clipboard,
        Permission::Secrets,
    ];

    pub fn id(self) -> &'static str {
        match self {
            Permission::Network => "network",
            Permission::Process => "process",
            Permission::ProjectFiles => "fs.project",
            Permission::UserFiles => "fs.user",
            Permission::Clipboard => "clipboard",
            Permission::Secrets => "secrets",
        }
    }

    pub fn description(self) -> &'static str {
        match self {
            Permission::Network => "Connect to network services",
            Permission::Process => "Run external programs",
            Permission::ProjectFiles => "Read and write files in the open project",
            Permission::UserFiles => "Read and write files outside the project",
            Permission::Clipboard => "Access the clipboard",
            Permission::Secrets => "Store credentials",
        }
    }
}

/// Standalone application mode (`--app <plugin-id>`).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AppManifest {
    /// View rendered as the window content.
    pub view: String,
    /// Window title; defaults to the plugin name.
    #[serde(default)]
    pub title: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PluginManifest {
    /// Reverse-domain identifier, e.g. `dev.archipel.hello`.
    pub id: String,
    pub name: String,
    pub version: String,
    /// Plugin API version the plugin was written against ([`crate::API_VERSION`]).
    pub api_version: u32,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub vendor: String,
    #[serde(default)]
    pub license: Option<String>,
    /// Plugin homepage shown on the Plugins page.
    #[serde(default)]
    pub homepage: Option<String>,
    #[serde(default)]
    pub kind: PluginKind,
    /// Entry point relative to the plugin directory (`main.lua`, `plugin.wasm`).
    #[serde(default)]
    pub entry: Option<String>,
    #[serde(default)]
    pub permissions: Vec<Permission>,
    /// Free-form feature tags shown on the Plugins page (`language`, `lsp`, …).
    #[serde(default)]
    pub capabilities: Vec<String>,
    /// Plugin ids that must be enabled for this plugin to activate.
    #[serde(default)]
    pub dependencies: Vec<String>,
    #[serde(default)]
    pub app: Option<AppManifest>,
}

impl PluginManifest {
    /// Parses and validates a manifest.
    pub fn parse(input: &str) -> Result<Self, PluginError> {
        let manifest: Self =
            toml::from_str(input).map_err(|error| PluginError::Manifest(error.to_string()))?;
        manifest.validate()?;
        Ok(manifest)
    }

    pub fn validate(&self) -> Result<(), PluginError> {
        if !valid_id(&self.id) {
            return Err(PluginError::InvalidId(self.id.clone()));
        }
        if self.api_version != crate::API_VERSION {
            return Err(PluginError::UnsupportedApi {
                plugin: self.id.clone(),
                requested: self.api_version,
                supported: crate::API_VERSION,
            });
        }
        if self.name.trim().is_empty() {
            return Err(PluginError::Manifest(format!("{}: name is empty", self.id)));
        }
        if self.version.trim().is_empty() {
            return Err(PluginError::Manifest(format!(
                "{}: version is empty",
                self.id
            )));
        }
        if matches!(self.kind, PluginKind::Lua | PluginKind::Wasm) && self.entry.is_none() {
            return Err(PluginError::Manifest(format!(
                "{}: {} plugins need an `entry`",
                self.id,
                self.kind.label()
            )));
        }
        if let Some(entry) = &self.entry
            && (entry.starts_with('/') || entry.contains("..") || entry.contains('\\'))
        {
            return Err(PluginError::Manifest(format!(
                "{}: entry must be a relative path inside the plugin: {entry}",
                self.id
            )));
        }
        if let Some(dependency) = self.dependencies.iter().find(|id| !valid_id(id)) {
            return Err(PluginError::InvalidId(dependency.clone()));
        }
        Ok(())
    }

    pub fn has_permission(&self, permission: Permission) -> bool {
        self.permissions.contains(&permission)
    }

    /// Window title in standalone app mode.
    pub fn app_title(&self) -> String {
        self.app
            .as_ref()
            .and_then(|app| app.title.clone())
            .unwrap_or_else(|| self.name.clone())
    }
}

/// Reverse-domain id: at least two non-empty dot-separated segments of
/// ASCII alphanumerics, `-` and `_`.
pub fn valid_id(id: &str) -> bool {
    id.split('.').count() >= 2
        && id.split('.').all(|segment| {
            !segment.is_empty()
                && segment
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_'))
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_full_manifest() {
        let manifest = PluginManifest::parse(
            r#"
id = "dev.archipel.hello"
name = "Hello"
version = "0.1.0"
api_version = 1
kind = "lua"
entry = "main.lua"
permissions = ["network", "fs.project"]
dependencies = ["dev.archipel.core-ui"]

[app]
view = "main"
title = "Hello App"
"#,
        )
        .unwrap();
        assert_eq!(manifest.kind, PluginKind::Lua);
        assert!(manifest.has_permission(Permission::Network));
        assert!(!manifest.has_permission(Permission::Process));
        assert_eq!(manifest.app_title(), "Hello App");
    }

    #[test]
    fn legacy_flat_manifest_still_parses() {
        let manifest = PluginManifest::parse(
            "id='dev.archipel.rust'\nname='Rust'\nversion='0.1.0'\napi_version=1\ncapabilities=['language']\n",
        )
        .unwrap();
        assert_eq!(manifest.kind, PluginKind::Declarative);
        assert_eq!(manifest.capabilities, ["language"]);
    }

    #[test]
    fn rejects_invalid_manifests_with_actionable_errors() {
        let base = "name='X'\nversion='1'\napi_version=1\n";
        for (input, needle) in [
            (format!("id='bad'\n{base}"), "reverse-domain"),
            (format!("id='a..b'\n{base}"), "reverse-domain"),
            (
                "id='a.b'\nname='X'\nversion='1'\napi_version=99\n".to_owned(),
                "API version 99",
            ),
            (format!("id='a.b'\n{base}kind='wasm'\n"), "entry"),
            (
                format!("id='a.b'\n{base}kind='lua'\nentry='../x.lua'\n"),
                "relative path",
            ),
            (format!("id='a.b'\n{base}permissions=['root']\n"), "invalid"),
        ] {
            let error = PluginManifest::parse(&input).unwrap_err().to_string();
            assert!(error.contains(needle), "{input:?} -> {error}");
        }
    }

    #[test]
    fn accepts_archipel_and_jetthing_manifest_names() {
        assert_eq!(MANIFEST_FILES[0], "archipel-plugin.toml");
        assert!(MANIFEST_FILES.contains(&"jetthing-plugin.toml"));
        assert!(MANIFEST_FILES.contains(&LEGACY_MANIFEST_FILE));
    }
}
