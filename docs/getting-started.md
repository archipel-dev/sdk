# Write a plugin in 10 minutes

You will build a plugin with a command, a menu item and a view, and test it headlessly.
You need Rust 1.91 or newer. The finished result is [`examples/hello`](../examples/hello).

## 1. Create the crate

```bash
cargo new --lib my-tool && cd my-tool
```

```toml
# Cargo.toml
[dependencies]
archipel-sdk = { git = "https://github.com/archipel-dev/sdk", tag = "v0.1.0" }

[dev-dependencies]
archipel-sdk = { git = "https://github.com/archipel-dev/sdk", tag = "v0.1.0", features = ["testing"] }
```

`archipel-sdk` is the only dependency you need. It re-exports the plugin API as
`archipel_sdk::api`, the egui view support as `archipel_sdk::ui`, and the UI kit as
`archipel_sdk::kit` (components, theme, icons).

## 2. Write the manifest

```toml
# archipel-plugin.toml
id = "com.example.mytool"
name = "My Tool"
version = "0.1.0"
api_version = 1
kind = "rust"
vendor = "Example"
description = "Counts clicks."

[app]
view = "mytool.main"
title = "My Tool"
```

`api_version` must equal `archipel_sdk::API_VERSION`; hosts refuse other versions
([versioning.md](versioning.md)). The older file name `jetthing-plugin.toml` is still read.

## 3. Implement `Plugin`

```rust,ignore
// src/lib.rs
use archipel_sdk::api::*;
use archipel_sdk::kit::ui::Button;
use archipel_sdk::ui;

pub struct MyTool {
    manifest: PluginManifest,
    clicks: u32,
}

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
    fn manifest(&self) -> &PluginManifest {
        &self.manifest
    }

    fn contributions(&self) -> Contributions {
        Contributions::new()
            .command(CommandContribution::new("mytool.click", "Click"))
            .menu(MenuLocation::Tools, "mytool.click")
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
        ui::header(ui, "My Tool", |_| {});
        if ui.add(Button::primary("Click")).clicked() {
            let _ = self.execute("mytool.click", Value::Null, host);
        }
        ui::property(ui, "Clicks", &self.clicks.to_string());
    }
}
```

Notes:

- Return errors, never panic on user input: unknown commands and bad params are
  `CommandError::Unknown` and `CommandError::InvalidParams`.
- The host installs the kit theme and fonts. Do not style anything yourself: build views from
  kit components ([plugin-ui.md](plugin-ui.md)).
- Methods run on the UI thread. Do long work on a thread and hand results back in `poll`.

## 4. Test it

```rust,ignore
#[cfg(test)]
mod tests {
    use super::*;
    use archipel_sdk::testing::{Fixture, Queryable, RecordingHost, audit, view_harness};
    use archipel_sdk::kit::Theme;

    #[test]
    fn command_updates_the_status() {
        let mut plugin = MyTool::new();
        let mut host = RecordingHost::new("com.example.mytool");
        plugin.execute("mytool.click", Value::Null, &mut host).unwrap();
        assert_eq!(host.statuses, ["1 clicks"]);
    }

    #[test]
    fn view_counts_clicks_and_is_accessible() {
        let view = ViewContext { view: "mytool.main".into(), placement: ViewPlacement::App };
        let mut harness =
            view_harness(Theme::dark(), Fixture::new(MyTool::new(), "com.example.mytool"), view);
        harness.get_by_label("Click").click();
        harness.run();
        assert_eq!(harness.state().plugin.clicks, 1);
        audit(&mut harness).assert_clean();
    }
}
```

Run `cargo test`. Rendering tests need a GPU or a software Vulkan driver; on Ubuntu CI
install `mesa-vulkan-drivers` ([ready-to-copy CI step](../README.md#view-tests-on-linux-ci)).

## 5. Run it as an app

Give the manifest an `[app]` view (as in step 2) and a second binary, and the plugin runs on its
own, with theme, settings page, status bar, notifications and a command palette
(<kbd>Ctrl+Shift+A</kbd>):

```rust,ignore
// examples/app.rs
fn main() -> Result<(), archipel_sdk::app::AppError> {
    archipel_sdk::app::run(
        my_tool::MyTool::new(),
        archipel_sdk::app::AppOptions::default(),
    )
}
```

```bash
cargo run --example app
```

[standalone-apps.md](standalone-apps.md) lists what the window provides. Try the finished
example with `cargo run -p hello --example app`. If the window fails to open on a machine with
a poor GPU stack, run with `ARCHIPEL_RENDERER=glow`.

## 6. Run it in a host

A compiled-in Rust plugin is built into the host: the host's app crate depends on your crate
and registers `MyTool::new`. Lua and WASM plugins are packed and installed from the host's
Plugins page instead ([lua.md](lua.md), [wasm.md](wasm.md)). The Archipel IDE documents its own
registration steps in [api.md](api.md#minimal-plugin).

Next: the [API reference](api.md), [Plugin UI](plugin-ui.md) and
[versioning](versioning.md).
