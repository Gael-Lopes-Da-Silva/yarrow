use crate::command::Command;

pub struct Cli {
    pub arguments: Vec<String>,
    pub commands: Vec<Command>,
}

impl Cli {
    pub fn new(arguments: Vec<String>) -> Self {
        Self {
            arguments,
            commands: Self::get_commands(),
        }
    }

    pub fn get_commands() -> Vec<Command> {
        vec![
            Command::new("run".to_string(), "".to_string(), vec![]),
            Command::new(
                "build".to_string(),
                "".to_string(),
                vec![Command::new("run".to_string(), "".to_string(), vec![])],
            ),
            Command::new("help".to_string(), "".to_string(), vec![]),
        ]
    }
}
