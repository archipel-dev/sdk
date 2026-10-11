use std::{collections::BTreeMap, sync::Arc};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CommandSpec {
    pub id: String,
    pub label: String,
    pub category: String,
    pub shortcut: Option<String>,
    pub params: String,
}

#[derive(Debug, Error, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "code", content = "message")]
pub enum CommandError {
    #[error("unknown command: {0}")]
    Unknown(String),
    #[error("command is disabled: {0}")]
    Disabled(String),
    #[error("invalid parameters: {0}")]
    InvalidParams(String),
    #[error("command failed: {0}")]
    Failed(String),
}

pub type CommandResult = Result<Value, CommandError>;
type Handler<C> = Arc<dyn Fn(&mut C, Value) -> CommandResult + Send + Sync>;
type Enabled<C> = Arc<dyn Fn(&C) -> bool + Send + Sync>;

pub struct Command<C> {
    pub spec: CommandSpec,
    enabled: Enabled<C>,
    handler: Handler<C>,
}

impl<C> Command<C> {
    pub fn new(
        spec: CommandSpec,
        enabled: impl Fn(&C) -> bool + Send + Sync + 'static,
        handler: impl Fn(&mut C, Value) -> CommandResult + Send + Sync + 'static,
    ) -> Self {
        Self {
            spec,
            enabled: Arc::new(enabled),
            handler: Arc::new(handler),
        }
    }
}

#[derive(Default)]
pub struct CommandRegistry<C> {
    commands: BTreeMap<String, Command<C>>,
}

impl<C> CommandRegistry<C> {
    pub fn register(&mut self, command: Command<C>) -> Result<(), CommandError> {
        // A duplicate must not replace the existing handler.
        match self.commands.entry(command.spec.id.clone()) {
            std::collections::btree_map::Entry::Occupied(entry) => Err(CommandError::Failed(
                format!("duplicate command id: {}", entry.key()),
            )),
            std::collections::btree_map::Entry::Vacant(entry) => {
                entry.insert(command);
                Ok(())
            }
        }
    }

    /// Removes a command (used when a plugin is disabled). Returns whether it existed.
    pub fn unregister(&mut self, id: &str) -> bool {
        self.commands.remove(id).is_some()
    }

    pub fn contains(&self, id: &str) -> bool {
        self.commands.contains_key(id)
    }

    pub fn specs(&self) -> Vec<CommandSpec> {
        self.commands.values().map(|c| c.spec.clone()).collect()
    }

    pub fn is_enabled(&self, id: &str, context: &C) -> bool {
        self.commands.get(id).is_some_and(|c| (c.enabled)(context))
    }

    pub fn execute(&self, id: &str, context: &mut C, params: Value) -> CommandResult {
        let command = self
            .commands
            .get(id)
            .ok_or_else(|| CommandError::Unknown(id.into()))?;
        if !(command.enabled)(context) {
            return Err(CommandError::Disabled(id.into()));
        }
        (command.handler)(context, params)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_discovers_executes_and_rejects_duplicates() {
        let mut registry = CommandRegistry::<u32>::default();
        let spec = CommandSpec {
            id: "counter.increment".into(),
            label: "Increment".into(),
            category: "Test".into(),
            shortcut: None,
            params: "{}".into(),
        };
        registry
            .register(Command::new(
                spec.clone(),
                |_| true,
                |value, _| {
                    *value += 1;
                    Ok(Value::from(*value))
                },
            ))
            .unwrap();
        assert_eq!(registry.specs(), vec![spec.clone()]);
        let mut value = 0;
        assert_eq!(
            registry.execute(&spec.id, &mut value, Value::Null).unwrap(),
            1
        );
        assert!(
            registry
                .register(Command::new(spec.clone(), |_| true, |_, _| Ok(Value::Null)))
                .is_err()
        );
        // The original handler survives a rejected duplicate.
        assert_eq!(
            registry.execute(&spec.id, &mut value, Value::Null).unwrap(),
            2
        );
        assert!(registry.unregister(&spec.id));
        assert!(!registry.contains(&spec.id));
        assert!(!registry.unregister(&spec.id));
    }
}
