//! The settings page, rendered from the plugin's `SettingsPage` schema with kit components.

use archipel_plugin_api::{PluginHost, SettingKind, SettingsPage, Value};
use archipel_plugin_ui::kit::components::{Dropdown, SectionHeader, TextField, Toggle};
use archipel_plugin_ui::{muted, section};
use egui::Ui;
use std::collections::HashMap;

/// Edit buffers of number fields, so half-typed input (`1.`, `-`) survives a frame.
#[derive(Debug, Default)]
pub struct SettingsView {
    numbers: HashMap<String, String>,
}

impl SettingsView {
    pub fn show(&mut self, ui: &mut Ui, pages: &[SettingsPage], host: &mut dyn PluginHost) {
        let inset = archipel_plugin_ui::kit::tokens::size::CONTENT_INSET as i8;
        egui::Frame::new().inner_margin(inset).show(ui, |ui| {
            egui::ScrollArea::vertical()
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    ui.set_max_width(560.0);
                    for page in pages {
                        if pages.len() > 1 {
                            section(ui, &page.title);
                        } else {
                            ui.heading(&page.title);
                            ui.add(SectionHeader::new("Settings"));
                        }
                        for field in &page.fields {
                            self.field(ui, field, host);
                            if let Some(description) = &field.description {
                                ui.label(muted(ui, description.as_str()));
                            }
                            ui.add_space(6.0);
                        }
                    }
                });
        });
    }

    fn field(
        &mut self,
        ui: &mut Ui,
        field: &archipel_plugin_api::SettingField,
        host: &mut dyn PluginHost,
    ) {
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
        match &field.kind {
            SettingKind::Text { .. } => {
                let mut text = current.as_str().unwrap_or_default().to_owned();
                if ui
                    .add(TextField::new(&mut text).label(field.label.as_str()))
                    .changed()
                {
                    write(Value::from(text), host);
                }
            }
            SettingKind::Bool { .. } => {
                let mut flag = current.as_bool().unwrap_or_default();
                if ui
                    .add(Toggle::new(&mut flag, field.label.as_str()))
                    .changed()
                {
                    write(Value::from(flag), host);
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
                let response = ui.add(
                    TextField::new(buffer)
                        .label(field.label.as_str())
                        .error(error.as_deref()),
                );
                if response.changed()
                    && let (Ok(n), None) = (parsed, &error)
                {
                    write(Value::from(n), host);
                }
            }
            SettingKind::Choice { options, .. } => {
                let mut choice = current.as_str().unwrap_or_default().to_owned();
                ui.label(field.label.as_str());
                let out = ui.add(
                    Dropdown::new(
                        ("setting", &field.key),
                        &mut choice,
                        options.iter().cloned(),
                    )
                    .label(field.label.as_str()),
                );
                if out.changed() {
                    write(Value::from(choice), host);
                }
            }
        }
    }
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

    #[test]
    fn numbers_format_without_a_trailing_zero() {
        assert_eq!(format_number(3.0), "3");
        assert_eq!(format_number(0.5), "0.5");
    }
}
