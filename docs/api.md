# Archipel plugin API (version 1)

Plugins add commands, menu items, keybindings, settings pages, tool windows, editor tabs and
status bar widgets to a host: the Archipel IDE, or another Archipel tool. A plugin can also
run on its own as a standalone app. Plugin API 1 covers **compiled-in Rust plugins**, **Lua
plugins** ([lua.md](lua.md)) and **WebAssembly component plugins** ([wasm.md](wasm.md)).
New here? Start with [getting-started.md](getting-started.md).

A plugin depends on one crate, [`archipel-sdk`](../crates/archipel-sdk), which re-exports the
crates of this repository:

| Crate | `archipel_sdk::` | Purpose |
|---|---|---|
| `archipel-command` | (re-exported by `api`) | `CommandSpec`, `CommandError`, `CommandResult` |
| `archipel-plugin-api` | `api` | engine side, no UI toolkit: `Plugin` trait, `Contributions`, `PluginHost`, `PluginManifest`, `ViewFrame`, declarative `ViewNode`, `testing::RecordingHost` |
| `archipel-plugin-ui` | `ui`, `kit` | egui side: `ui(frame) -> Option<&mut egui::Ui>`, `EguiView`, `render_node`, kit-based helpers (`header`, `section`, `property`, `empty_state`), and `archipel_kit` as `kit` |
| `archipel-plugin-testing` | `testing` (feature) | fake host, `Fixture`, `view_harness`, and `archipel_kit::testing` (`audit`, `conformance`, snapshots) |

The host side lives in the host's own repository and is only described here: the registry
(discovery, enable/disable, settings store, activation order, `.jtplugin` archives), the
`PluginHost` implementation and command routing, the workbench that renders contributions,
and the Lua and WASM runtimes. In the Archipel IDE these are `jetthing-plugins`,
`jetthing-plugin-lua`, `jetthing-plugin-wasm`, `jetthing-core` and `jetthing-ui`.

Engine crates never see egui. A view is rendered into an opaque `ViewFrame`, and the egui host
puts its `&mut egui::Ui` inside. `archipel_sdk::ui::ui(frame)` gives it back to the plugin.
The host has already installed the Archipel Kit theme and fonts in the `egui::Context`
(`archipel_kit::install`), so views built from kit components match the host without any
setup. See [plugin-ui.md](plugin-ui.md).

## Minimal plugin

The complete example is [`examples/hello`](../examples/hello/src/lib.rs). Its parts:

```toml
# my-tool/archipel-plugin.toml
id = "com.example.mytool"        # reverse-domain, unique
name = "My Tool"
version = "0.1.0"
api_version = 1                  # must equal archipel_sdk::API_VERSION
kind = "rust"                    # rust | lua | wasm | declarative
vendor = "Example"
description = "Shown on the Plugins page."
permissions = ["network"]        # network, process, fs.project, fs.user, clipboard, secrets
dependencies = []                # other plugin ids that must be active first

[app]                            # optional: lets the host open the plugin as a standalone app
view = "mytool.main"
title = "My Tool"
```

The host reads `archipel-plugin.toml`; it also still reads `jetthing-plugin.toml`, the name
used before the rename (see `archipel_plugin_api::MANIFEST_FILES`). When a directory holds
both, `archipel-plugin.toml` wins.

```toml
# my-tool/Cargo.toml
[package]
name = "my-tool"
version = "0.1.0"
edition = "2024"

[dependencies]
archipel-sdk = { git = "https://github.com/archipel-dev/sdk", tag = "v0.1.0" }

[dev-dependencies]
archipel-sdk = { git = "https://github.com/archipel-dev/sdk", tag = "v0.1.0", features = ["testing"] }
```

```rust,ignore
use archipel_sdk::api::*;
use archipel_sdk::kit::{components::Button, egui};
use archipel_sdk::ui;

pub struct MyTool { manifest: PluginManifest, clicks: u32 }

impl MyTool {
    pub fn new() -> Self {
        Self {
            manifest: PluginManifest::parse(include_str!("../archipel-plugin.toml"))
                .expect("bundled manifest is valid"),
            clicks: 0,
        }
    }
}

impl Plugin for MyTool {
    fn manifest(&self) -> &PluginManifest { &self.manifest }

    fn contributions(&self) -> Contributions {
        Contributions::new()
            .command(CommandContribution::new("mytool.click", "Click"))
            .menu(MenuLocation::Tools, "mytool.click")
            .keybinding("mytool.click", "Mod+Shift+K")   // Mod = ⌘ on macOS, Ctrl elsewhere
            .tool_window(ToolWindowContribution::new("mytool", "My Tool", "mytool.main"))
    }

    fn execute(&mut self, command: &str, _params: Value, host: &mut dyn PluginHost) -> CommandResult {
        match command {
            "mytool.click" => {
                self.clicks += 1;
                host.show_status(&format!("{} clicks", self.clicks));
                Ok(json!(self.clicks))
            }
            other => Err(CommandError::Unknown(other.into())),
        }
    }

    fn render_view(&mut self, _view: &ViewContext, frame: &mut ViewFrame<'_>, host: &mut dyn PluginHost) {
        let Some(ui) = ui::ui(frame) else { return };
        if ui.add(Button::primary("Click")).clicked() {
            host.dispatch("mytool.click", Value::Null); // or call your own method directly
        }
    }
}
```

How a host registers a Rust plugin is up to the host: compiled-in plugins export a constructor
(`MyTool::new`) and the host's app crate adds a factory for it. The Archipel IDE does this in
`crates/jetthing-app/src/plugins.rs`.

## Lifecycle

1. **Discovery.** In the Archipel IDE, at startup the registry adds the bundled factories. It then reads
   manifests from the per-user plugin directory (`<config>/plugins/*/archipel-plugin.toml`, or `jetthing-plugin.toml`)
   and from the open project's `plugins/` directory. `<config>` is `$JETTHING_CONFIG_DIR`,
   or `~/.config/jetthing`, `~/Library/Application Support/jetthing` or `%APPDATA%\jetthing`.
   Invalid manifests are listed with their error in File ▸ Plugins….
2. **Activation.** Enabled plugins activate in dependency order. `activate(host)` runs first;
   then `contributions()` is read and validated (`Contributions::problems()`). If activation
   fails, or the contributions are inconsistent, the plugin is marked *Failed* with the reason.
3. **Running.** Contributed commands go into the shared command registry. That makes them
   available in Search Everywhere / Find Action, the main menu, keybindings and the CLI
   surface, and they run through `Plugin::execute`. `Plugin::poll` runs every frame;
   collect background results there and call `host.request_repaint()` while work is in flight.
4. **Disable / enable.** This works without a restart. `deactivate(host)` runs, the plugin's
   commands are unregistered and its tool window and tabs close. Re-enabling creates a fresh
   instance. The choice persists in `<config>/plugins.json`.
5. **Exit.** `deactivate` runs for every active plugin.

All plugin methods run on the UI thread. Never block in them: spawn a thread
(or a runtime) and hand results back through `poll`.

## Contributions

| Contribution | Builder | Where it appears |
|---|---|---|
| Command | `CommandContribution::new(id, label).category(..).params(..)` | Search Everywhere, Find Action, CLI; ids must be namespaced (`plugin.action`) and must not clash with built-ins (built-ins win) |
| Menu item | `.menu(MenuLocation::Tools, command)` | main menu (File, Edit, View, Navigate, Code, Build, Run, Git, Tools, Help) |
| Keybinding | `.keybinding(command, "Mod+Alt+H")` or `"⌥⌘H"` | global, after built-in shortcuts; also shown as the command's shortcut |
| Settings page | `SettingsPage::new(id, title).field(SettingField::new(key, label, SettingKind::…))` | Settings tab of the plugin's details on the Plugins page; values via `host.setting(key)`; `Text`, `Bool`, `Number{min,max}` (clamped), `Choice` |
| Tool window | `ToolWindowContribution::new(id, title, view).icon_text("H")` | right tool window stripe and panel |
| Editor view | `EditorContribution::new(view, title)`; open with `host.open_editor_tab(EditorTabRequest::new(view, title).key(..))` | editor tab bar; the same key focuses the existing tab |
| Status widget | `StatusWidgetContribution::new(id).command(..)` | status bar; text from `Plugin::status_widget(id)` |
| App view | `[app] view = …` in the manifest | standalone app window |

Views can also return a declarative `ViewNode` tree (columns, rows, labels, buttons, inputs,
lists, tables, trees) instead of drawing with egui. `archipel_sdk::ui::render_node` renders it with kit components
and returns `ViewEvent`s. Lua and WASM plugins use this model.

`ViewContext::placement` tells a view where it is rendered:
`ToolWindow { id }`, `EditorTab { key, path }`, `App` or `SettingsPage { id }`.

## Host services (`PluginHost`)

| Method | Notes |
|---|---|
| `plugin_id()`, `project_root()` | |
| `show_status(msg)`, `notify(Notification::info/error)` | notifications show as balloons |
| `dispatch(command, params)` | queued; runs after the current call (built-in or contributed commands) |
| `open_tool_window(id)`, `open_editor_tab(request)` | own views only |
| `setting(key)`, `set_setting(key, value)` | settings-page defaults apply when unset |
| `data_dir()` | `<config>/plugin-data/<id>`, created on demand |
| `has_permission(p)`, `require_permission(p)` | checks the manifest's `permissions` |
| `refresh_contributions()` | re-reads `contributions()` after the call |
| `request_repaint()` | keeps the UI animating while busy |

## Testing

`archipel_sdk::testing` (feature `testing`) has everything to test a plugin without a host:

- `RecordingHost` (also `archipel_sdk::api::testing::RecordingHost`) records statuses,
  notifications, dispatched commands, opened views and tabs; it holds settings and
  permissions in memory, and `DataDir` gives it a temporary data directory.
- `Fixture` and `view_harness` render a view in a themed kit harness. Find widgets by role
  and visible label, click, type, and assert on `harness.state()`.
- `audit(&mut harness).assert_clean()` checks the view for unnamed controls, tiny targets,
  overlaps and unreachable focus; `conformance(label, ..)` runs the kit's component battery.

Rendering uses wgpu: on Linux CI install `mesa-vulkan-drivers`. See the tests in
[`examples/hello`](../examples/hello/src/lib.rs).

## Host commands and packaging (Archipel IDE)

The rest of this page describes the Archipel IDE as a host. Other hosts may differ; the plugin-facing
contract above is what the SDK guarantees.

### Commands for plugin management

| Command | Params |
|---|---|
| `plugin.manager.open` | — open the Plugins page (File ▸ Plugins…) |
| `plugin.install` | `{"path"}` — install a `.jtplugin` archive |
| `plugin.uninstall` | `{"id"}` — remove an installed plugin |
| `plugin.refresh` | — rediscover manifests |
| `plugin.list` | — JSON list with status and failure reason |
| `plugin.enable`, `plugin.disable` | `{"id"}` |
| `plugin.setSetting` | `{"plugin","key","value"}`, validated against the settings page |
| `plugin.toolwindow.toggle` | `{"plugin","id"}` |
| `plugin.tab.activate`, `plugin.tab.close` | `{"index"}` (close defaults to the active tab) |

### Packaging and installing (`.jtplugin`)

A `.jtplugin` file is a zip archive. `archipel-plugin.toml` (or `jetthing-plugin.toml`) sits at its root, or inside one
top-level directory, next to the plugin's files.

```bash
cargo run -p xtask -- pack-plugin path/to/plugin [out.jtplugin]   # default: dist/plugins/<id>-<version>.jtplugin
```

Hidden files and `target/` are skipped. Install from **File ▸ Plugins… ▸ ⚙ ▸ Install Plugin from
Disk…** or with `plugin.install {"path"}`. The archive is validated before anything is written.
Installation rejects archives that:

- have an invalid manifest, more than one plugin, or a missing `entry`;
- contain entries outside the plugin directory, or symbolic links;
- have more than 10 000 entries or unpack to more than 512 MiB;
- contain a Rust plugin (Rust plugins are compiled in).

Files are extracted to a staging directory first, then swapped into
`<config>/plugins/<id>/`. Reinstalling replaces the previous version without a restart.
`plugin.uninstall {"id"}` (or **Uninstall** on the Plugins page) deactivates the plugin and removes
its directory. Bundled and project plugins cannot be uninstalled, and an archive cannot replace a
bundled plugin's id.

### Standalone app mode

The IDE's `--app <plugin-id>` (or `--app=<id>`) opens a window that contains only the
plugin's `[app]` view, with a title bar, window controls, status text, the plugin's
status widgets and its notifications. An unknown, disabled or app-less plugin exits
with code 2 and a message that lists the plugins that do have an app view.

## Stability

See [versioning.md](versioning.md) for the `api_version` policy.
