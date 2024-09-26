mod commands;
mod flags;

fn main() {
    let matches = clap::Command::new("")
        .version("0.0.1")
        .arg_required_else_help(true)
        .subcommand(clap::Command::new("run").about("Run the current project using the interpreter"))
        .subcommand(
            clap::Command::new("build")
                .about("Build the current project using the compiler")
                .arg(
                    clap::Arg::new("optimization")
                        .short('O')
                        .long("optimization")
                        .value_name("number")
                        .action(clap::ArgAction::Set)
                        .help("Change the level of optimization between 0 and 3")
                        .global(true)
                        .required(false),
                )
                .arg(
                    clap::Arg::new("quiet")
                        .short('q')
                        .long("quiet")
                        .action(clap::ArgAction::SetTrue)
                        .help("Do not print any debug information when compiling")
                        .global(true)
                        .required(false),
                )
                .arg(
                    clap::Arg::new("verbose")
                        .short('v')
                        .long("verbose")
                        .action(clap::ArgAction::SetTrue)
                        .help("Print more information when compiling")
                        .global(true)
                        .required(false),
                )
                .subcommand(clap::Command::new("run").about("")),
        )
        .subcommand(clap::Command::new("aliases").about(""))
        .subcommand(clap::Command::new("init").about(""))
        .subcommand(clap::Command::new("new").about(""))
        .subcommand(clap::Command::new("add").about(""))
        .subcommand(clap::Command::new("update").about(""))
        .subcommand(clap::Command::new("remove").about(""))
        .subcommand(clap::Command::new("fmt").about(""))
        .subcommand(clap::Command::new("lsp").about(""))
        .get_matches();

    match matches.subcommand() {
        Some(("run", matches)) => {
            match matches.subcommand() {
                _ => {
                    commands::run();
                }
            }
        }
        Some(("build", matches)) => {
            if let Some(optimization) = matches.get_one::<String>("optimization") {
                flags::optimization(optimization.to_string());
            }

            if matches.get_flag("quiet") {
                flags::quiet();
            }

            if matches.get_flag("verbose") {
                flags::verbose();
            }


            match matches.subcommand() {
                Some(("run", matches)) => {
                    commands::build::run();
                }
                _ => {
                    commands::build();
                }
            }
        }
        _ => unreachable!(),
    }
}
