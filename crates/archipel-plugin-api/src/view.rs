//! Declarative views: a plain-data widget tree that script and WASM plugins
//! return from `render_view`. The host renders it and reports
//! [`ViewEvent`]s back. Rust plugins can use it too, or draw with egui.

use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ViewNode {
    /// Children stacked vertically.
    Column {
        #[serde(default)]
        children: Vec<ViewNode>,
    },
    /// Children laid out horizontally.
    Row {
        #[serde(default)]
        children: Vec<ViewNode>,
    },
    Heading {
        text: String,
    },
    Label {
        text: String,
        #[serde(default)]
        muted: bool,
    },
    /// Small uppercase group title.
    Section {
        text: String,
    },
    /// Key/value row.
    Property {
        key: String,
        value: String,
    },
    Separator,
    Button {
        id: String,
        label: String,
    },
    TextInput {
        id: String,
        #[serde(default)]
        value: String,
        #[serde(default)]
        hint: String,
        #[serde(default)]
        multiline: bool,
    },
    Checkbox {
        id: String,
        label: String,
        #[serde(default)]
        checked: bool,
    },
    /// Single-choice drop-down.
    Choice {
        id: String,
        options: Vec<String>,
        #[serde(default)]
        selected: String,
    },
    List {
        id: String,
        #[serde(default)]
        items: Vec<ListItem>,
        #[serde(default)]
        selected: Option<String>,
    },
    Table {
        id: String,
        columns: Vec<String>,
        #[serde(default)]
        rows: Vec<Vec<String>>,
        #[serde(default)]
        selected: Option<usize>,
    },
    Tree {
        id: String,
        #[serde(default)]
        nodes: Vec<TreeNode>,
        #[serde(default)]
        selected: Option<String>,
    },
    /// Monospace, selectable text.
    Code {
        text: String,
    },
    /// Centered placeholder.
    Empty {
        title: String,
        #[serde(default)]
        hint: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ListItem {
    pub id: String,
    pub label: String,
    #[serde(default)]
    pub detail: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TreeNode {
    pub id: String,
    pub label: String,
    #[serde(default)]
    pub children: Vec<TreeNode>,
}

/// User interaction with a declarative view.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ViewEvent {
    Clicked {
        id: String,
    },
    /// New value of a text input (string), checkbox (bool) or choice (string).
    Changed {
        id: String,
        value: Value,
    },
    /// Selected list item id, table row index or tree node id.
    Selected {
        id: String,
        item: Value,
    },
}

impl ViewEvent {
    pub fn id(&self) -> &str {
        match self {
            ViewEvent::Clicked { id }
            | ViewEvent::Changed { id, .. }
            | ViewEvent::Selected { id, .. } => id,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn view_trees_round_trip_through_json() {
        let json = serde_json::json!({
            "type": "column",
            "children": [
                {"type": "heading", "text": "Requests"},
                {"type": "text_input", "id": "url", "value": "https://example.com"},
                {"type": "list", "id": "history", "items": [{"id": "1", "label": "GET /"}]},
                {"type": "separator"}
            ]
        });
        let node: ViewNode = serde_json::from_value(json.clone()).unwrap();
        let ViewNode::Column { children } = &node else {
            panic!("column")
        };
        assert_eq!(children.len(), 4);
        assert!(serde_json::from_value::<ViewNode>(serde_json::json!({"type": "nope"})).is_err());
        let event = ViewEvent::Changed {
            id: "url".into(),
            value: Value::from("x"),
        };
        assert_eq!(event.id(), "url");
    }
}
