mod cli;
mod command;

fn main() {
    let cli = cli::Cli::new(std::env::args().collect());

    for command in cli.commands {
        if cli.arguments.contains(&command.name) {
        }
    }
}
