mod cli;
mod command;

use crate::cli::Cli;
use crate::command::Command;

fn main() {
    let cli = Cli::new(
        std::env::args().collect(),
        vec![
            Command::new("run".to_string(), "".to_string(), vec![]),
            Command::new(
                "build".to_string(),
                "".to_string(),
                vec![Command::new("run".to_string(), "".to_string(), vec![])],
            ),
            Command::new("help".to_string(), "".to_string(), vec![]),
        ],
    );

    if cli.arguments.len() <= 0 {
        Cli::print_help("".to_string());
    }

    for argument in cli.arguments {
        match argument {
            _ => Cli::print_help("".to_string()),
        }
    }
}
