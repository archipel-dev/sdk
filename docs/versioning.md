# Versioning and `api_version`

Two numbers matter to a plugin author.

## `api_version` (the plugin contract)

`archipel_plugin_api::API_VERSION` (also `archipel_sdk::API_VERSION`) is the version of the
contract between plugins and hosts: the `Plugin` trait, contributions, `PluginHost`, the
manifest format, declarative views and the WIT world of WASM plugins. A manifest declares the
version it targets in `api_version`, and a host refuses plugins that target a version it does
not support (`PluginError::UnsupportedApi`).

- A breaking change to the contract bumps `API_VERSION` by one. Breaking means removing or
  renaming an item, changing a signature, a manifest field or a command's meaning, or
  tightening validation so previously valid plugins fail.
- Additive changes do not bump it: new contribution kinds, new `PluginHost` methods with
  default implementations, new optional manifest fields, new `ViewNode` kinds that older
  hosts may skip.
- A host may support several versions at once; it says which in its own docs. The SDK
  announces a new `API_VERSION` in the release notes and the PR body, one release before
  hosts drop the old one wherever feasible.
- API 1 is **stable once the first-party plugins (nexus-http, mongo-raabit) build on it**.
  Until then, changes are still possible; they are listed in the changelog.

## Crate versions (semver)

The crates of this repository share one version number and follow
[semver](https://semver.org/). Before 1.0, a minor bump (`0.x`) may break Rust source
compatibility; a patch bump does not. Bumping `API_VERSION` always comes with a breaking crate
release. Depend on the facade, `archipel-sdk = { git = "https://github.com/archipel-dev/sdk", tag = "v0.1.0" }`, so you get matching crates.

`archipel-kit` has its own releases; the SDK pins the compatible one and re-exports it as
`archipel_sdk::kit`. Do not add a second, different `archipel-kit` to your plugin: two
versions would install two themes.

## Releases and depending on the SDK by git tag

Releases are automatic. Commits follow [Conventional Commits](https://www.conventionalcommits.org/);
release-please keeps a release PR open on `main` that bumps `[workspace.package] version` in
`Cargo.toml` and writes `CHANGELOG.md`. Merging it tags `v<version>` and publishes a GitHub
release. Before 1.0, `feat` and `fix` bump the patch and breaking changes (`!`) bump the minor. Never bump versions by hand.

Until the crates are on crates.io, apps (the IDE, nexus-http, mongo-raabit) and plugins depend
on the SDK by git tag:

```toml
[dependencies]
archipel-sdk = { git = "https://github.com/archipel-dev/sdk", tag = "v0.1.0" }
```

Pin a tag, never a branch, so a build changes only when you bump the tag. Upgrading is a
one-line change; read the `CHANGELOG.md` entries in between first, and check whether
`API_VERSION` changed. An app and the plugins it hosts must use the same SDK tag (or at least
the same `API_VERSION` and the same `archipel-kit`), otherwise two copies of the crates, and of
the theme, end up in one binary. `Cargo.lock` records the exact `archipel-kit` commit the SDK
resolves to.

## Manifest file names

`archipel-plugin.toml` is the manifest name. `jetthing-plugin.toml` (the name before the
rename) is still read, with the same contents, and so is the older `plugin.toml`. When a
directory holds several, the first of `archipel_plugin_api::MANIFEST_FILES` wins. New plugins
should use `archipel-plugin.toml`.
