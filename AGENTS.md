# Archipel SDK engineering guide

The plugin SDK of the Archipel tools. Plugin authors depend only on `archipel-sdk` (plus
`archipel-kit` through it). The IDE and the first-party plugins build on the same crates, so
API changes here are API changes for everyone.

## Non-negotiables

- `archipel-command` and `archipel-plugin-api` stay free of egui and any UI toolkit.
- The plugin API is versioned (`API_VERSION`, `docs/versioning.md`). Breaking changes bump it
  and are called out in the PR; additive changes (default methods, optional fields) do not.
- Plugin views are built from Archipel Kit components. No ad-hoc colors, fonts or sizes; the
  host installs the theme (`archipel_kit::install`), plugins and this SDK never do.
- No panics on user input (commands, params, manifests, view events). Return structured
  errors. `expect` only on bundled, compile-time-known data.
- Clean-room: no third-party or JetBrains code, icons or artwork.
- Every example and doc snippet that claims to compile must compile in CI (`examples/hello`).

## Layout

`archipel-command` <- `archipel-plugin-api` <- `archipel-plugin-ui` (+ `archipel-kit`) <-
`archipel-plugin-testing`; `archipel-sdk` re-exports them. Lower crates never depend on
higher ones.

## Required checks

```bash
export CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

View tests need a GPU or `mesa-vulkan-drivers`.

Commit messages and PR titles are Conventional Commits (`feat`, `fix`, `perf`, `refactor`,
`docs`, `test`, `chore`, `ci`; `!` for breaking).

## Releases

Semantic releases: release-please keeps a release PR open on main; merging it bumps the
version, writes `CHANGELOG.md` and tags `v<version>`. Don't bump versions by hand. Apps
depend on the SDK by git tag; see `docs/versioning.md`.

## Kit dependency

`archipel-kit` is a git dependency on `branch = "main"` (`[workspace.dependencies]` in
`Cargo.toml`).
