//! The command palette (Ctrl+Shift+A), built from kit components.

use archipel_plugin_ui::kit::parts::Slot;
use archipel_plugin_ui::kit::ui::{ColoredListCellRenderer, JBTextField};
use archipel_plugin_ui::kit::{Theme, tokens};
use egui::{Align2, Key, Order};

/// One row of the palette.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PaletteEntry {
    pub id: String,
    pub label: String,
    pub category: String,
    pub shortcut: Option<String>,
}

/// State of the palette overlay.
#[derive(Debug, Default)]
pub struct Palette {
    open: bool,
    query: String,
    selected: usize,
    just_opened: bool,
}

impl Palette {
    pub fn is_open(&self) -> bool {
        self.open
    }

    pub fn open(&mut self) {
        self.open = true;
        self.query.clear();
        self.selected = 0;
        self.just_opened = true;
    }

    pub fn close(&mut self) {
        self.open = false;
    }

    pub fn toggle(&mut self) {
        if self.open {
            self.close();
        } else {
            self.open();
        }
    }

    /// Draws the palette; returns the id of the command the user picked.
    pub fn show(&mut self, ctx: &egui::Context, entries: &[PaletteEntry]) -> Option<String> {
        if !self.open {
            return None;
        }
        // The click that opened the palette (a toolbar button, a menu item) is still in this
        // frame's input: it is not a click outside.
        let opening = self.just_opened;
        let matches = filter(entries, &self.query);
        let (up, down, enter, escape) = ctx.input_mut(|input| {
            (
                input.consume_key(egui::Modifiers::NONE, Key::ArrowUp),
                input.consume_key(egui::Modifiers::NONE, Key::ArrowDown),
                input.consume_key(egui::Modifiers::NONE, Key::Enter),
                input.consume_key(egui::Modifiers::NONE, Key::Escape),
            )
        });
        if escape {
            self.close();
            return None;
        }
        if down && !matches.is_empty() {
            self.selected = (self.selected + 1) % matches.len();
        }
        if up && !matches.is_empty() {
            self.selected = (self.selected + matches.len() - 1) % matches.len();
        }
        self.selected = self.selected.min(matches.len().saturating_sub(1));

        let colors = Theme::colors(ctx);
        let mut picked = enter.then(|| matches.get(self.selected).copied()).flatten();
        let area = egui::Area::new(egui::Id::new("archipel-command-palette"))
            .order(Order::Foreground)
            .anchor(Align2::CENTER_TOP, [0.0, 56.0])
            .show(ctx, |ui| {
                egui::Frame::new()
                    .fill(colors.popup)
                    .stroke(egui::Stroke::new(1.0_f32, colors.popup_border))
                    .corner_radius(tokens::radius::POPUP)
                    .inner_margin(tokens::size::DIALOG_PADDING as i8)
                    .show(ui, |ui| {
                        ui.set_width(520.0);
                        let field = ui.add(
                            JBTextField::search(&mut self.query).placeholder("Run a command…"),
                        );
                        if self.just_opened {
                            field.request_focus();
                            self.just_opened = false;
                        }
                        if field.changed() {
                            self.selected = 0;
                        }
                        ui.add_space(6.0);
                        egui::ScrollArea::vertical()
                            .max_height(320.0)
                            .auto_shrink([false, true])
                            .show(ui, |ui| {
                                for (row, index) in matches.iter().copied().enumerate() {
                                    let entry = &entries[index];
                                    let mut line =
                                        ColoredListCellRenderer::new(entry.label.as_str())
                                            .secondary(entry.category.as_str())
                                            .selected(row == self.selected);
                                    if let Some(shortcut) = &entry.shortcut {
                                        line = line.trailing(Slot::text(shortcut.as_str()));
                                    }
                                    let out = line.show(ui);
                                    if row == self.selected {
                                        out.response.scroll_to_me(None);
                                    }
                                    if out.response.clicked() {
                                        picked = Some(index);
                                    }
                                }
                                if matches.is_empty() {
                                    ui.label("No matching commands");
                                }
                            });
                    });
            });
        let clicked_outside = !opening
            && ctx.input(|input| {
                input.pointer.any_pressed()
                    && input
                        .pointer
                        .interact_pos()
                        .is_some_and(|pos| !area.response.rect.contains(pos))
            });
        if clicked_outside {
            self.close();
            return None;
        }
        picked.map(|index| {
            self.close();
            entries[index].id.clone()
        })
    }
}

/// Indices of the entries matching `query`: every whitespace-separated term must occur
/// (case-insensitive) in the category or label; label prefixes rank first.
pub fn filter(entries: &[PaletteEntry], query: &str) -> Vec<usize> {
    let terms: Vec<String> = query.split_whitespace().map(str::to_lowercase).collect();
    let mut hits: Vec<(bool, usize)> = entries
        .iter()
        .enumerate()
        .filter_map(|(index, entry)| {
            let label = entry.label.to_lowercase();
            let haystack = format!("{} {label}", entry.category.to_lowercase());
            terms
                .iter()
                .all(|term| haystack.contains(term.as_str()))
                .then(|| {
                    let prefix = terms.first().is_some_and(|t| label.starts_with(t.as_str()));
                    (!prefix, index)
                })
        })
        .collect();
    hits.sort();
    hits.into_iter().map(|(_, index)| index).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(id: &str, label: &str, category: &str) -> PaletteEntry {
        PaletteEntry {
            id: id.into(),
            label: label.into(),
            category: category.into(),
            shortcut: None,
        }
    }

    #[test]
    fn filtering_matches_all_terms_and_ranks_prefixes_first() {
        let entries = vec![
            entry("a", "Open Settings", "App"),
            entry("b", "Say Hello", "Tools"),
            entry("c", "Hello Settings", "Tools"),
        ];
        assert_eq!(filter(&entries, ""), [0, 1, 2]);
        assert_eq!(filter(&entries, "hello"), [2, 1]);
        assert_eq!(filter(&entries, "tools set"), [2]);
        assert!(filter(&entries, "zzz").is_empty());
    }
}
