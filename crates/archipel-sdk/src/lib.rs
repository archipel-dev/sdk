//! # Archipel SDK
//!
//! The one dependency of an Archipel plugin.
//!
//! - [`api`]: the plugin contract: [`api::Plugin`], contributions, host services and the
//!   manifest (`archipel-plugin-api`, with the command types of `archipel-command`).
//! - [`ui`]: the egui side of plugin views (`archipel-plugin-ui`).
//! - [`kit`]: Archipel Kit, the components every plugin view is built from.
//! - `testing` (feature `testing`): a fake host and kit-based view harnesses, for
//!   `[dev-dependencies]` (`archipel-plugin-testing`).
//!
//! ```toml
//! [dependencies]
//! archipel-sdk = { git = "https://github.com/archipel-dev/sdk", tag = "v0.1.0" }
//!
//! [dev-dependencies]
//! archipel-sdk = { git = "https://github.com/archipel-dev/sdk", tag = "v0.1.0", features = ["testing"] }
//! ```

pub use archipel_plugin_api as api;
#[cfg(feature = "testing")]
pub use archipel_plugin_testing as testing;
pub use archipel_plugin_ui as ui;
pub use archipel_plugin_ui::kit;

/// The version of the plugin API this SDK implements: what manifests declare as
/// `api_version`.
pub use archipel_plugin_api::API_VERSION;
