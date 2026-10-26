//! egui side of Archipel plugin views, on [Archipel Kit](archipel_kit).
//!
//! The engine-level [`archipel_plugin_api`] stays toolkit-free; the egui host renders
//! plugin views by passing `&mut egui::Ui` inside a [`ViewFrame`]. Plugins use [`ui`] to
//! get it back and build the view from the kit's components, re-exported as [`kit`].
//!
//! The host installs the kit once (`archipel_kit::install`), which sets fonts, spacing and
//! the theme in the `egui::Context`; plugins never install or restyle anything. Colors
//! come from `kit::Theme::colors(ui.ctx())`, sizes from `kit::tokens`. The helpers below
//! ([`header`], [`section`], [`property`], [`empty_state`]) are thin wrappers over kit
//! components, kept so plugin code ports mechanically.

mod declarative;

pub use archipel_kit as kit;
pub use archipel_plugin_api::ViewFrame;
pub use declarative::render_node;
pub use egui;

use archipel_kit::Theme;
use archipel_kit::ui::{PropertyGrid, SeparatorComponent, StatusText, TitledSeparator};
use egui::{Color32, RichText, Ui};

/// The egui `Ui` inside a frame rendered by the egui host, if any.
pub fn ui<'f>(frame: &'f mut ViewFrame<'_>) -> Option<&'f mut Ui> {
    frame.downcast_mut::<Ui>()
}

/// A view implemented with egui. Plugins can keep one per view and call
/// [`render`] from `Plugin::render_view`.
pub trait EguiView {
    fn ui(&mut self, ui: &mut Ui);
}

/// Renders `view` into `frame` when the host is egui. Returns whether it ran.
pub fn render(frame: &mut ViewFrame<'_>, view: &mut dyn EguiView) -> bool {
    match ui(frame) {
        Some(ui) => {
            view.ui(ui);
            true
        }
        None => false,
    }
}

/// Width for a text input: bounded in rows so the widgets after it stay visible, the
/// available width otherwise.
pub fn input_width(ui: &Ui, multiline: bool) -> f32 {
    if !multiline && ui.layout().is_horizontal() {
        (ui.available_width() * 0.6).clamp(120.0, 320.0)
    } else {
        ui.available_width()
    }
}

/// Muted secondary text color of the kit theme.
pub fn secondary_text(ui: &Ui) -> Color32 {
    Theme::colors(ui.ctx()).text_secondary
}

/// Tool window header row: title on the left, `actions` on the right (laid out right to
/// left), then a separator.
pub fn header(ui: &mut Ui, title: &str, actions: impl FnOnce(&mut Ui)) {
    ui.horizontal(|ui| {
        ui.heading(title);
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), actions);
    });
    ui.add(SeparatorComponent);
}

/// Group title with a hairline, as used for sections in tool windows.
pub fn section(ui: &mut Ui, title: &str) {
    ui.add(TitledSeparator::new(title));
}

/// Centered placeholder for an empty view.
pub fn empty_state(ui: &mut Ui, title: &str, hint: &str) {
    let mut state = StatusText::new(title);
    if !hint.is_empty() {
        state = state.hint(hint);
    }
    state.show(ui);
}

/// Key/value row with a dimmed key column.
pub fn property(ui: &mut Ui, key: &str, value: &str) {
    PropertyGrid::new(("plugin_property", key))
        .row(key, value)
        .show(ui);
}

/// Text in the muted secondary color.
pub fn muted(ui: &Ui, text: impl Into<String>) -> RichText {
    RichText::new(text).color(secondary_text(ui))
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Counter(u32);

    impl EguiView for Counter {
        fn ui(&mut self, ui: &mut Ui) {
            header(ui, "Counter", |_| {});
            section(ui, "Group");
            property(ui, "Key", "value");
            empty_state(ui, "Nothing", "");
            self.0 += 1;
        }
    }

    #[test]
    fn renders_into_egui_frames_only() {
        let ctx = egui::Context::default();
        kit::install(&ctx, Theme::dark());
        let mut counter = Counter(0);
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                assert!(render(&mut ViewFrame::new(ui), &mut counter));
            });
        });
        assert!(
            counter.0 >= 1,
            "the view ran (installing fonts may add a pass)"
        );
        let mut not_egui = 5_u8;
        assert!(!render(&mut ViewFrame::new(&mut not_egui), &mut counter));
    }
}
