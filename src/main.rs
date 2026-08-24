use std::process::ExitCode;

fn main() -> ExitCode {
    yarrow_cli::run(std::env::args_os())
}
