fn main() {
    let matches = clap::Command::new("")
        .version("0.0.1")
        .arg_required_else_help(true)
        .subcommand(
            clap::Command::new("run").about("").arg(
                clap::Arg::new("quiet")
                    .short('q')
                    .long("quiet")
                    .action(clap::ArgAction::SetTrue)
                    .required(false),
            ),
        )
        .subcommand(
            clap::Command::new("build")
                .about("")
                .arg(
                    clap::Arg::new("optimization")
                        .short('O')
                        .long("optimization")
                        .value_name("number")
                        .action(clap::ArgAction::Set)
                        .required(false),
                )
                .subcommand(
                    clap::Command::new("run").about("").arg(
                        clap::Arg::new("optimization")
                            .short('O')
                            .long("optimization")
                            .value_name("number")
                            .action(clap::ArgAction::Set)
                            .required(false),
                    ),
                ),
        )
        .subcommand(clap::Command::new("aliases").about(""))
        .subcommand(clap::Command::new("init").about(""))
        .subcommand(clap::Command::new("new").about(""))
        .subcommand(clap::Command::new("add").about(""))
        .subcommand(clap::Command::new("update").about(""))
        .subcommand(clap::Command::new("remove").about(""))
        .subcommand(clap::Command::new("lsp").about(""))
        .subcommand(clap::Command::new("fmt").about(""))
        .get_matches();

    if let Some(matches) = matches.subcommand_matches("run") {
        if matches.get_flag("quiet") {
            println!("it's quiest here");
        }
    }

    if let Some(matches) = matches.subcommand_matches("build") {
        if let Some(optimization) = matches.get_one::<String>("optimization") {
            println!("{}", optimization);
        }
    }
}
