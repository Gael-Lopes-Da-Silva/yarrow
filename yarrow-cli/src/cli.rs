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

    pub fn print_help(argument: String) {
        println!("{}", argument);
        match argument {
            _ => {}
        }
    }

    pub fn print_error(message: String) {

    }

    pub fn print_info(message: String) {

    }
}
