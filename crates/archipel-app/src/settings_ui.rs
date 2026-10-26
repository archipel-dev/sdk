//! The settings page, rendered from the plugin's `SettingsPage` schema with kit components.

use archipel_plugin_api::{PluginHost, SettingField, SettingKind, SettingsPage, Value};
use archipel_plugin_ui::kit::openapi::ui::ValidationInfo;
use archipel_plugin_ui::kit::tokens::size;
use archipel_plugin_ui::kit::ui::TitledSeparator;
use archipel_plugin_ui::kit::ui::dsl::{Panel, panel};
use egui::Ui;
use std::collections::HashMap;

/// Width of a page's form, as Android Studio's settings pages (the fields do not stretch
/// across a wide window).
const PAGE_WIDTH: f32 = 560.0;

/// Edit buffers of fractional number fields, so half-typed input (`1.`, `-`) survives a frame.
#[derive(Debug, Default)]
pub struct SettingsView {
    numbers: HashMap<String, String>,
}

impl SettingsView {
    /// Every page as an Android Studio settings page: its title with a line, then a form
    /// with a checkbox per switch and a labelled row per value, each with its description
    /// as a comment.
    pub fn show(&mut self, ui: &mut Ui, pages: &[SettingsPage], host: &mut dyn PluginHost) {
        let inset = size::CONTENT_INSET as i8;
        egui::Frame::new().inner_margin(inset).show(ui, |ui| {
            ui.set_max_width(PAGE_WIDTH);
            for page in pages {
                ui.add(TitledSeparator::new(page.title.as_str()));
                panel(ui, |p| {
                    p.indent(|p| {
                        for field in &page.fields {
                            self.field(p, field, host);
                        }
                    });
                });
            }
        });
    }

    fn field(&mut self, panel: &mut Panel<'_>, field: &SettingField, host: &mut dyn PluginHost) {
        let current = host
            .setting(&field.key)
            .unwrap_or_else(|| field.kind.default_value());
        let write = |value: Value, host: &mut dyn PluginHost| {
            if let Err(error) = host.set_setting(&field.key, value) {
                host.notify(archipel_plugin_api::Notification::error(
                    "Setting not saved",
                    error.to_string(),
                ));
            }
        };
        let label = format!("{}:", field.label);
        let description = field.description.as_deref();
        match &field.kind {
            SettingKind::Bool { .. } => {
                let mut flag = current.as_bool().unwrap_or_default();
                let mut changed = false;
                panel.unlabeled_row(|r| {
                    let cell = r.check_box(&mut flag, field.label.as_str());
                    if let Some(description) = description {
                        cell.comment(description);
                    }
                    cell.on_response(|response| changed = response.changed());
                });
                if changed {
                    write(Value::from(flag), host);
                }
            }
            SettingKind::Text { .. } => {
                let mut text = current.as_str().unwrap_or_default().to_owned();
                let mut changed = false;
                panel.row(&label, |r| {
                    r.text_field(&mut text)
                        .on_response(|response| changed = response.changed());
                    if let Some(description) = description {
                        r.row_comment(description);
                    }
                });
                if changed {
                    write(Value::from(text), host);
                }
            }
            SettingKind::Number { min, max, .. }
                if whole(*min) && whole(*max) && whole_value(&current) =>
            {
                // Whole numbers (a page size, a timeout) get an integer field with its range.
                let mut n = current.as_f64().unwrap_or_default() as i64;
                let low = min.map_or(i64::MIN, |m| m as i64);
                let high = max.map_or(i64::MAX, |m| m as i64);
                let mut changed = false;
                panel.row(&label, |r| {
                    r.int_spinner_with(&mut n, |spinner| spinner.range(low..=high))
                        .on_response(|response| changed = response.changed());
                    if let Some(description) = description {
                        r.row_comment(description);
                    }
                });
                if changed {
                    write(Value::from(n), host);
                }
            }
            SettingKind::Number { min, max, .. } => {
                let buffer = self
                    .numbers
                    .entry(field.key.clone())
                    .or_insert_with(|| current.as_f64().map(format_number).unwrap_or_default());
                let parsed = buffer.trim().parse::<f64>();
                let error = match &parsed {
                    Ok(n) if min.is_some_and(|m| *n < m) || max.is_some_and(|m| *n > m) => {
                        Some(format!(
                            "Between {} and {}",
                            min.map_or("-∞".into(), format_number),
                            max.map_or("∞".into(), format_number)
                        ))
                    }
                    Ok(_) => None,
                    Err(_) => Some("Enter a number".to_owned()),
                };
                let validation = error.as_deref().map(ValidationInfo::error);
                let mut changed = false;
                panel.row(&label, |r| {
                    r.text_field(buffer)
                        .width(size::NUMBER_FIELD_WIDTH)
                        .validation(validation.as_ref())
                        .on_response(|response| changed = response.changed());
                    if let Some(description) = description {
                        r.row_comment(description);
                    }
                });
                if changed && let (Ok(n), None) = (parsed, &error) {
                    write(Value::from(n), host);
                }
            }
            SettingKind::Choice { options, .. } => {
                let mut choice = current.as_str().unwrap_or_default().to_owned();
                let mut changed = false;
                panel.row(&label, |r| {
                    r.combo_box(
                        ("setting", &field.key),
                        &mut choice,
                        options.iter().cloned(),
                    )
                    .on_response(|response| changed = response.changed());
                    if let Some(description) = description {
                        r.row_comment(description);
                    }
                });
                if changed {
                    write(Value::from(choice), host);
                }
            }
        }
    }
}

/// A bound that is absent or a whole number.
fn whole(bound: Option<f64>) -> bool {
    bound.is_none_or(|b| b.fract() == 0.0)
}

fn whole_value(value: &Value) -> bool {
    value
        .as_f64()
        .is_some_and(|n| n.fract() == 0.0 && n.abs() < 9e15)
}

fn format_number(n: f64) -> String {
    if n.fract() == 0.0 && n.abs() < 1e15 {
        format!("{n:.0}")
    } else {
        n.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use archipel_plugin_api::testing::RecordingHost;
    use archipel_plugin_testing::kit_testing::{self, Queryable};
    use archipel_plugin_ui::kit::Theme;

    fn page() -> SettingsPage {
        SettingsPage::new("db", "Database")
            .field(
                SettingField::new(
                    "confirm",
                    "Confirm changes",
                    SettingKind::Bool { default: true },
                )
                .description("Show the query and the documents it affects before it runs."),
            )
            .field(
                SettingField::new(
                    "page",
                    "Page size",
                    SettingKind::Number {
                        default: 50.0,
                        min: Some(1.0),
                        max: Some(1000.0),
                    },
                )
                .description("Documents per page in tables."),
            )
            .field(SettingField::new(
                "theme",
                "Result view",
                SettingKind::Choice {
                    options: vec!["Table".into(), "Tree".into()],
                    default: "Table".into(),
                },
            ))
            .field(SettingField::new(
                "app",
                "Application name",
                SettingKind::Text {
                    default: String::new(),
                },
            ))
    }

    fn harness(theme: Theme) -> kit_testing::Harness<'static, (SettingsView, RecordingHost)> {
        let pages = vec![page()];
        kit_testing::harness_state_sized(
            theme,
            egui::vec2(640.0, 340.0),
            (SettingsView::default(), RecordingHost::new("test")),
            move |ui, (view, host)| view.show(ui, &pages, host),
        )
    }

    #[test]
    fn pages_are_forms_and_write_what_is_changed() {
        let mut harness = harness(Theme::dark());
        // Switches are checkboxes with their description, values rows in one column.
        let page = harness.get_by_label("Page size").rect();
        let name = harness.get_by_label("Application name").rect();
        assert!((page.left() - name.left()).abs() < 0.5, "{page:?} {name:?}");
        harness.get_by_label("Confirm changes").click();
        harness.run();
        assert_eq!(
            harness.state().1.setting("confirm"),
            Some(Value::from(false))
        );
    }

    #[test]
    fn numbers_format_without_a_trailing_zero() {
        assert_eq!(format_number(3.0), "3");
        assert_eq!(format_number(0.5), "0.5");
    }
}
