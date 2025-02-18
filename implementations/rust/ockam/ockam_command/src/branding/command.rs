use crate::Result;
use once_cell::sync::Lazy;
use std::fmt::{Debug, Formatter};

pub(crate) fn name(name: &str) -> &'static str {
    CUSTOM_COMMANDS.name(name)
}

pub(crate) fn hide(name: &str) -> bool {
    CUSTOM_COMMANDS.hide(name)
}

static CUSTOM_COMMANDS: Lazy<Commands> =
    Lazy::new(|| Commands::from_env().expect("Failed to load custom commands"));

pub(crate) struct Commands {
    commands: Vec<Command>,
}

pub(crate) struct Command {
    name: String,
    custom_name: String,
}

impl Debug for Command {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Command")
            .field("name", &self.name)
            .field("custom_name", &self.custom_name)
            .finish()
    }
}

impl Commands {
    fn new(commands: &str) -> Result<Self> {
        let commands = commands
            .split(',')
            .filter_map(|c| {
                if c.is_empty() {
                    return None;
                }
                let mut parts = c.split('=');
                let name = match parts.next() {
                    Some(name) => name,
                    None => return None,
                };
                let custom_name = parts.next().unwrap_or(name);
                Some(Command {
                    name: name.to_string(),
                    custom_name: custom_name.to_string(),
                })
            })
            .collect();
        Ok(Self { commands })
    }

    pub fn from_env() -> Result<Self> {
        Self::new(super::BrandingCompileEnvVars::commands())
    }

    pub fn hide(&self, command_name: &str) -> bool {
        if self.commands.is_empty() {
            return false;
        }
        !self.commands.iter().any(|c| c.name == command_name)
    }

    pub fn name(&self, command_name: &str) -> &'static str {
        if self.commands.is_empty() {
            return Box::leak(command_name.to_string().into_boxed_str());
        }
        self.commands
            .iter()
            .find(|c| c.name == command_name)
            .map(|c| Box::leak(c.custom_name.clone().into_boxed_str()))
            .unwrap_or(Box::leak(command_name.to_string().into_boxed_str()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hide() {
        let commands = Commands::new("node create=host create,project,enroll").unwrap();
        assert!(!commands.hide("node create"));
        assert!(!commands.hide("project"));
        assert!(!commands.hide("enroll"));
        assert!(commands.hide("command4"));

        let commands = Commands::new("").unwrap();
        assert!(!commands.hide("command1"));
    }

    #[test]
    fn test_commands() {
        let commands = Commands::new("node create=host create,project,enroll").unwrap();
        assert_eq!(commands.name("node create"), "host create");
        assert_eq!(commands.name("project"), "project");
        assert_eq!(commands.name("enroll"), "enroll");
        assert_eq!(commands.name("command4"), "command4");

        let commands = Commands::new("").unwrap();
        assert_eq!(commands.name("command1"), "command1");
    }
}
