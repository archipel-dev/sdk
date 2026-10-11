//! egui renderer for declarative [`ViewNode`] trees, built from kit components.

use crate::{empty_state, muted, property, section};
use archipel_kit::components::{
    Button, Cell, Checkbox, Column, Dropdown, ListRow, Separator, Table, TextField, Tree, TreeRow,
    VirtualList,
};
use archipel_plugin_api::{TreeNode, Value, ViewEvent, ViewNode};
use egui::{Id, RichText, Ui};
use std::collections::HashSet;

const COLLECTION_HEIGHT: f32 = 240.0;
const DEFAULT_COLUMN_WIDTH: f32 = 140.0;

/// Renders `node`; `scope` keeps widget ids unique per view. Returns the
/// interactions of this frame.
pub fn render_node(ui: &mut Ui, scope: &str, node: &ViewNode) -> Vec<ViewEvent> {
    let mut events = Vec::new();
    node_ui(ui, scope, node, &mut events);
    events
}

fn node_ui(ui: &mut Ui, scope: &str, node: &ViewNode, events: &mut Vec<ViewEvent>) {
    let id = |widget: &str| Id::new(("plugin_view", scope, widget.to_owned()));
    match node {
        ViewNode::Column { children } => {
            ui.vertical(|ui| {
                for child in children {
                    node_ui(ui, scope, child, events);
                }
            });
        }
        ViewNode::Row { children } => {
            ui.horizontal(|ui| {
                for child in children {
                    node_ui(ui, scope, child, events);
                }
            });
        }
        ViewNode::Heading { text } => {
            ui.heading(text);
        }
        ViewNode::Label { text, muted: dim } => {
            if *dim {
                ui.label(muted(ui, text));
            } else {
                ui.label(text);
            }
        }
        ViewNode::Section { text } => section(ui, text),
        ViewNode::Property { key, value } => property(ui, key, value),
        ViewNode::Separator => {
            ui.add(Separator);
        }
        ViewNode::Button { id: button, label } => {
            ui.push_id(id(button), |ui| {
                if ui.add(Button::secondary(label.as_str())).clicked() {
                    events.push(ViewEvent::Clicked { id: button.clone() });
                }
            });
        }
        ViewNode::TextInput {
            id: input,
            value,
            hint,
            ..
        } => {
            // The kit's field is single-line; a multiline request renders as one line.
            let mut text = value.clone();
            ui.push_id(id(input), |ui| {
                let mut field = TextField::new(&mut text).width(crate::input_width(ui, false));
                if !hint.is_empty() {
                    field = field.label(hint.as_str());
                }
                if ui.add(field).changed() {
                    events.push(ViewEvent::Changed {
                        id: input.clone(),
                        value: Value::from(text.clone()),
                    });
                }
            });
        }
        ViewNode::Checkbox {
            id: checkbox,
            label,
            checked,
        } => {
            let mut value = *checked;
            ui.push_id(id(checkbox), |ui| {
                if ui.add(Checkbox::new(&mut value, label.as_str())).changed() {
                    events.push(ViewEvent::Changed {
                        id: checkbox.clone(),
                        value: Value::from(value),
                    });
                }
            });
        }
        ViewNode::Choice {
            id: choice,
            options,
            selected,
        } => {
            let mut value = selected.clone();
            ui.add(Dropdown::new(id(choice), &mut value, options.iter().cloned()).label(choice));
            if value != *selected {
                events.push(ViewEvent::Changed {
                    id: choice.clone(),
                    value: Value::from(value),
                });
            }
        }
        ViewNode::List {
            id: list,
            items,
            selected,
        } => {
            let mut current = selected
                .as_deref()
                .and_then(|sel| items.iter().position(|item| item.id == sel));
            let out = VirtualList::new(id(list), items.len(), &mut current)
                .name(list.as_str())
                .height(COLLECTION_HEIGHT)
                .show(ui, |index| {
                    let item = &items[index];
                    let row = ListRow::new(item.label.as_str());
                    match &item.detail {
                        Some(detail) => row.secondary(detail.as_str()),
                        None => row,
                    }
                });
            if out.selection_changed
                && let Some(item) = current.and_then(|index| items.get(index))
            {
                events.push(ViewEvent::Selected {
                    id: list.clone(),
                    item: Value::from(item.id.clone()),
                });
            }
        }
        ViewNode::Table {
            id: table,
            columns,
            rows,
            selected,
        } => {
            let table_id = id(table);
            // Column widths are the user's to drag, so they live in the context.
            let mut widths = ui
                .data_mut(|data| data.get_temp::<Vec<f32>>(table_id))
                .unwrap_or_default();
            widths.resize(columns.len(), DEFAULT_COLUMN_WIDTH);
            let mut cols: Vec<Column> = columns
                .iter()
                .zip(&widths)
                .map(|(title, width)| Column::new(title.as_str(), *width))
                .collect();
            let mut current = *selected;
            let out = Table::new(table_id, &mut cols, rows.len(), &mut current)
                .name(table.as_str())
                .height(COLLECTION_HEIGHT)
                .show(ui, |row, column| {
                    Cell::new(
                        rows.get(row)
                            .and_then(|cells| cells.get(column))
                            .map_or("", String::as_str),
                    )
                });
            let widths: Vec<f32> = cols.iter().map(|column| column.width).collect();
            ui.data_mut(|data| data.insert_temp(table_id, widths));
            if out.selection_changed
                && let Some(index) = current
            {
                events.push(ViewEvent::Selected {
                    id: table.clone(),
                    item: Value::from(index),
                });
            }
        }
        ViewNode::Tree {
            id: tree,
            nodes,
            selected,
        } => tree_ui(ui, id(tree), tree, nodes, selected.as_deref(), events),
        ViewNode::Code { text } => {
            ui.label(RichText::new(text).monospace());
        }
        ViewNode::Empty { title, hint } => empty_state(ui, title, hint),
    }
}

fn tree_ui(
    ui: &mut Ui,
    tree_id: Id,
    tree: &str,
    nodes: &[TreeNode],
    selected: Option<&str>,
    events: &mut Vec<ViewEvent>,
) {
    // Expansion is view state the plugin does not model, so it lives in the context.
    let expanded_key = tree_id.with("expanded");
    let mut expanded = ui
        .data_mut(|data| data.get_temp::<HashSet<String>>(expanded_key))
        .unwrap_or_default();
    let mut rows = Vec::new();
    let mut ids: Vec<&str> = Vec::new();
    flatten(nodes, 0, &expanded, &mut rows, &mut ids);
    let mut current = selected.and_then(|sel| ids.iter().position(|id| *id == sel));
    let out = Tree::new(tree_id, &rows, &mut current)
        .name(tree)
        .height(COLLECTION_HEIGHT)
        .show(ui);
    if let Some(index) = out.toggled
        && let Some(id) = ids.get(index)
        && !expanded.remove(*id)
    {
        expanded.insert((*id).to_owned());
    }
    ui.data_mut(|data| data.insert_temp(expanded_key, expanded));
    if out.selection_changed
        && let Some(id) = current.and_then(|index| ids.get(index))
    {
        events.push(ViewEvent::Selected {
            id: tree.to_owned(),
            item: Value::from((*id).to_owned()),
        });
    }
}

/// The visible rows of `nodes` (children of expanded nodes), with the node id of each.
fn flatten<'n>(
    nodes: &'n [TreeNode],
    depth: usize,
    expanded: &HashSet<String>,
    rows: &mut Vec<TreeRow>,
    ids: &mut Vec<&'n str>,
) {
    for node in nodes {
        let open = expanded.contains(&node.id);
        let mut row = TreeRow::new(node.label.as_str(), depth);
        if !node.children.is_empty() {
            row = row.parent(open);
        }
        rows.push(row);
        ids.push(node.id.as_str());
        if open {
            flatten(&node.children, depth + 1, expanded, rows, ids);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use archipel_plugin_api::ListItem;

    fn sample() -> ViewNode {
        ViewNode::Column {
            children: vec![
                ViewNode::Heading { text: "H".into() },
                ViewNode::Row {
                    children: vec![
                        ViewNode::Button {
                            id: "b".into(),
                            label: "Go".into(),
                        },
                        ViewNode::Checkbox {
                            id: "c".into(),
                            label: "On".into(),
                            checked: true,
                        },
                    ],
                },
                ViewNode::TextInput {
                    id: "t".into(),
                    value: "v".into(),
                    hint: "Name".into(),
                    multiline: false,
                },
                ViewNode::Choice {
                    id: "ch".into(),
                    options: vec!["a".into()],
                    selected: "a".into(),
                },
                ViewNode::List {
                    id: "l".into(),
                    items: vec![ListItem {
                        id: "1".into(),
                        label: "One".into(),
                        detail: None,
                    }],
                    selected: Some("1".into()),
                },
                ViewNode::Table {
                    id: "tb".into(),
                    columns: vec!["A".into()],
                    rows: vec![vec!["x".into()]],
                    selected: None,
                },
                ViewNode::Tree {
                    id: "tr".into(),
                    nodes: vec![TreeNode {
                        id: "r".into(),
                        label: "Root".into(),
                        children: vec![TreeNode {
                            id: "c".into(),
                            label: "Child".into(),
                            children: vec![],
                        }],
                    }],
                    selected: None,
                },
                ViewNode::Code { text: "fn".into() },
                ViewNode::Property {
                    key: "k".into(),
                    value: "v".into(),
                },
                ViewNode::Section { text: "s".into() },
                ViewNode::Label {
                    text: "l".into(),
                    muted: true,
                },
                ViewNode::Separator,
                ViewNode::Empty {
                    title: "e".into(),
                    hint: String::new(),
                },
            ],
        }
    }

    #[test]
    fn renders_every_node_kind_headlessly() {
        let node = sample();
        let ctx = egui::Context::default();
        archipel_kit::install(&ctx, archipel_kit::Theme::dark());
        let mut events = Vec::new();
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                events = render_node(ui, "test", &node);
            });
        });
        assert!(events.is_empty(), "no input, no events");
    }

    #[test]
    fn renders_without_an_installed_kit() {
        let ctx = egui::Context::default();
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                render_node(ui, "bare", &sample());
            });
        });
    }

    #[test]
    fn empty_collections_do_not_panic() {
        let node = ViewNode::Column {
            children: vec![
                ViewNode::Table {
                    id: "t".into(),
                    columns: vec![],
                    rows: vec![vec!["stray".into()]],
                    selected: Some(9),
                },
                ViewNode::List {
                    id: "l".into(),
                    items: vec![],
                    selected: Some("gone".into()),
                },
                ViewNode::Choice {
                    id: "c".into(),
                    options: vec![],
                    selected: String::new(),
                },
            ],
        };
        let ctx = egui::Context::default();
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                render_node(ui, "empty", &node);
            });
        });
    }
}
