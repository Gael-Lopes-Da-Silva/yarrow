use std::process::ExitCode;

mod cli;
mod command;

use crate::cli::Cli;
use crate::command::Command;

fn main() -> ExitCode {
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
        return ExitCode::from(0);
    }

    for argument in cli.arguments {
        match argument.to_lowercase().as_str() {
            "run" => {}
            "build" => {}
            "help" => {}
            _ => {}
        }
    }

    return ExitCode::from(0);
}
