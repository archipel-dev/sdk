//! Test helpers for Archipel plugins.
//!
//! - [`RecordingHost`]: a fake [`PluginHost`] that records commands, notifications, status
//!   messages, opened windows and tabs, and serves settings and a data directory from
//!   memory or a temporary folder ([`DataDir`]).
//! - [`Fixture`] and [`view_harness`]: render a plugin's view in a themed kit harness, so
//!   the test drives it by role and label and can run the kit's `conformance` and `audit`.
//! - [`kit_testing`]: everything of `archipel_kit::testing` (queries, `audit`,
//!   `conformance`, snapshots).
//!
//! Rendering uses wgpu: on Linux CI install `mesa-vulkan-drivers`.
//!
//! ```no_run
//! # use archipel_plugin_testing::*;
//! # use archipel_plugin_api::*;
//! # struct MyPlugin;
//! # impl Plugin for MyPlugin { fn manifest(&self) -> &PluginManifest { unimplemented!() } }
//! let fixture = Fixture::new(MyPlugin, "com.example.my");
//! let view = ViewContext { view: "my.main".into(), placement: ViewPlacement::App };
//! let mut harness = view_harness(archipel_kit::Theme::dark(), fixture, view);
//! harness.run();
//! audit(&mut harness).assert_clean();
//! ```

mod host;

pub use archipel_kit;
pub use archipel_kit::testing as kit_testing;
pub use archipel_kit::testing::{Harness, NodeExt, Queryable, audit, conformance};
pub use host::{DataDir, RecordingHost};

use archipel_kit::Theme;
use archipel_plugin_api::{Plugin, ViewContext, ViewFrame};

/// A plugin together with the fake host it runs against: the state of a view harness.
pub struct Fixture<P> {
    pub plugin: P,
    pub host: RecordingHost,
}

impl<P: Plugin> Fixture<P> {
    /// A fixture whose host reports `plugin_id` and has no permissions or data directory.
    pub fn new(plugin: P, plugin_id: impl Into<String>) -> Self {
        Self::with_host(plugin, RecordingHost::new(plugin_id))
    }

    /// A fixture with a prepared host (settings, permissions, data directory).
    pub fn with_host(plugin: P, host: RecordingHost) -> Self {
        Self { plugin, host }
    }
}

/// A harness that renders `view` of the plugin in `fixture` on an island of `theme`, with
/// the kit installed like the host does. Inspect the plugin and the host afterwards with
/// `harness.state()`.
pub fn view_harness<'a, P: Plugin + 'a>(
    theme: Theme,
    fixture: Fixture<P>,
    view: ViewContext,
) -> Harness<'a, Fixture<P>> {
    kit_testing::harness_state(theme, fixture, move |ui, fixture| {
        let Fixture { plugin, host } = fixture;
        plugin.render_view(&view, &mut ViewFrame::new(ui), host);
    })
}
