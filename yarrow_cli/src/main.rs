fn main() {
    let matches = clap::Command::new("")
        .version("0.0.1")
        .arg_required_else_help(true)
        .subcommand(clap::Command::new("run").about(""))
        .subcommand(
            clap::Command::new("build")
                .about("")
                .arg(
                    clap::Arg::new("optimization")
                        .short('O')
                        .long("optimization")
                        .value_name("number")
                        .action(clap::ArgAction::Set)
                        .help("")
                        .global(true)
                        .required(false),
                )
                .arg(
                    clap::Arg::new("quiet")
                        .short('q')
                        .long("quiet")
                        .action(clap::ArgAction::SetTrue)
                        .help("")
                        .global(true)
                        .required(false),
                )
                .arg(
                    clap::Arg::new("verbose")
                        .short('v')
                        .long("verbose")
                        .action(clap::ArgAction::SetTrue)
                        .help("")
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
            if matches.get_flag("quiet") {
                todo!("Implement quiet mode in run");
            }

            if matches.get_flag("verbose") {
                todo!("Implement verbose mode in run")
            }

            match matches.subcommand() {
                _ => {
                    todo!("Implement run command");
                }
            }
        }
        Some(("build", matches)) => {
            if let Some(optimization) = matches.get_one::<String>("optimization") {
                todo!("Implement optimization level in build")
            }

            match matches.subcommand() {
                Some(("run", matches)) => {
                    todo!("Implement run command in build")
                }
                _ => {
                    todo!("Implement build command")
                }
            }
        }
        _ => unreachable!(),
    }
}
