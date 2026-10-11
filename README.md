# Archipel SDK

The plugin SDK of the Archipel tools (Archipel IDE, Nexus HTTP, Mongo Raabit). A plugin
depends on one crate, `archipel-sdk`; the IDE and the first-party plugins build on the same
crates.

```toml
[dependencies]
archipel-sdk = { git = "https://github.com/archipel-dev/sdk", tag = "v0.1.0" }

[dev-dependencies]
archipel-sdk = { git = "https://github.com/archipel-dev/sdk", tag = "v0.1.0", features = ["testing"] }
```

| Crate | Purpose |
|---|---|
| [`archipel-sdk`](crates/archipel-sdk) | Facade: `archipel_sdk::{api, ui, kit, testing}`. The one dependency of a plugin. |
| [`archipel-plugin-api`](crates/archipel-plugin-api) | `Plugin` trait, contributions, `PluginHost`, manifest, declarative views. No UI toolkit. |
| [`archipel-command`](crates/archipel-command) | Command types shared by the host and plugins. |
| [`archipel-plugin-ui`](crates/archipel-plugin-ui) | egui side of plugin views, on [Archipel Kit](https://github.com/archipel-dev/kit); re-exports it as `kit`. |
| [`archipel-plugin-testing`](crates/archipel-plugin-testing) | Fake `PluginHost`, view harnesses, kit `audit` and `conformance`. |
| [`examples/hello`](examples/hello) | A complete minimal plugin with behavior, conformance and audit tests. |

## Docs

- [Write a plugin in 10 minutes](docs/getting-started.md)
- [API reference](docs/api.md), [Lua plugins](docs/lua.md), [WASM plugins](docs/wasm.md)
- [Plugin UI](docs/plugin-ui.md)
- [Versioning and `api_version`](docs/versioning.md)

The Lua and WASM runtimes live in the IDE; the docs describe the contract they implement.

## Development

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

View tests render through wgpu: on Linux install `mesa-vulkan-drivers` (or have a GPU).

## License

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE) or
[MIT license](LICENSE-MIT) at your option.
