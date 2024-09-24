use crate::command::Command;

pub struct Cli {
    pub arguments: Vec<String>,
    pub commands: Vec<Command>,
}

impl Cli {
    pub fn new(arguments: Vec<String>, commands: Vec<Command>) -> Self {
        Self {
            arguments,
            commands,
        }
    }

    pub fn print_help(command: String) {
        println!("Yarrow v0.0.1");
    }
}
