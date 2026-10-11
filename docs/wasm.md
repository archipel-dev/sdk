# WebAssembly component plugins

Third-party compiled plugins run as [WebAssembly components](https://component-model.bytecodealliance.org/)
in [wasmtime](https://wasmtime.dev/) (the IDE's `jetthing-plugin-wasm`; the runtime stays in the host, this page describes the contract). They implement the same contract as
Rust and Lua plugins ([api.md](api.md)), described in WIT:
`crates/jetthing-plugin-wasm/wit/plugin.wit` in the [IDE repository](https://github.com/archipel-dev/ide).
The example is `plugins/hello-wasm` there.

## The `jetthing:plugin@1.0.0` world

The guest **exports** `contributions`, `activate`, `deactivate`, `execute`, `render-view`,
`on-view-event`, `status-widget` and `poll`, and **imports** the `host` interface
(`status`, `notify`, `dispatch`, `open-tool-window`, `open-tab`, `setting`, `set-setting`,
`project-root`, `has-permission`, `request-repaint`).

Structured values are JSON strings with the same shapes as the Rust API. This keeps the WIT
surface small and lets every language reuse the documented types:

| WIT | JSON shape |
|---|---|
| `contributions() -> string` | `Contributions` (`commands`, `menus`, `keybindings`, `settings_pages`, `tool_windows`, `editors`, `status_widgets`) |
| `execute(command, params-json) -> result<string, string>` | params/result: any JSON |
| `render-view(view, placement-json) -> string` | `ViewNode` (declarative view, see [lua.md](lua.md#views)) |
| `on-view-event(view, event-json)` | `ViewEvent` (`clicked` / `changed` / `selected`) |

WASM views are declarative only. The host renders the returned tree and calls `on-view-event`
for each interaction.

## Building a plugin in Rust

```toml
# Cargo.toml (outside the host workspace)
[lib]
crate-type = ["cdylib"]

[dependencies]
wit-bindgen = "0.46"
serde_json = "1"
```

```rust
wit_bindgen::generate!({ path: "path/to/plugin.wit", world: "plugin" });

struct MyPlugin;
impl Guest for MyPlugin {
    fn contributions() -> String { r#"{"commands":[{"id":"my.hello","label":"Hello"}]}"#.into() }
    fn execute(command: String, _params: String) -> Result<String, String> {
        jetthing::plugin::host::status("hi");
        Ok("null".into())
    }
    // … activate, deactivate, render_view, on_view_event, status_widget, poll
}
export!(MyPlugin);
```

```bash
rustup target add wasm32-wasip2
cargo build --release --target wasm32-wasip2      # produces a component directly
```

Put the `.wasm` next to `archipel-plugin.toml` (`kind = "wasm"`, `entry = "my_plugin.wasm"`), then
pack it with `cargo run -p xtask -- pack-plugin <dir>` and install the archive.
`cargo run -p xtask -- build-wasm-example` rebuilds the checked-in example component. Any language
with component tooling (C/C++, Go, JavaScript via jco, Python via componentize-py) can target the
same world.

## Sandbox and permissions

| Resource | Default | With permission |
|---|---|---|
| Files | none: no preopened directories | `fs.project`: the open project is mounted read-write at `/project` |
| Network | denied (no sockets, no name lookup) | `network`: WASI sockets and name lookup |
| Environment, arguments, stdin | none | — |
| stderr | forwarded to the IDE's stderr | — |
| Memory | 64 MiB per instance | — |
| CPU | fuel budget per call (≈2·10⁹ for activate, commands and poll; 2·10⁸ per view frame or status widget) | — |

A call that runs out of fuel or traps fails with an error ("plugin exceeded its time budget
(out of fuel)"). The instance is then recreated, so the next call works; in-memory guest state is
lost. Host calls made during a guest call are buffered and applied when the call returns.
`setting` reads a snapshot of the keys the plugin declared on its settings pages.

The project is mounted when the plugin activates. A project opened later is mounted the next time
the plugin is enabled.
