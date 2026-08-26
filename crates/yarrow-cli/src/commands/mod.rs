mod check;
mod compile;
mod dump;
mod explain;
mod interpret;
mod run;

pub use check::check_file;
pub use compile::compile_file;
pub use dump::dump_file;
pub use explain::explain_code;
pub use interpret::interpret_file;
pub use run::run_file;

use yarrow_core::RunResult;

/// Print a supported entry return value the same way for `run` and `interpret`.
pub(crate) fn print_run_result(result: RunResult) {
    match result {
        RunResult::Void => {}
        RunResult::Int(v) => println!("{v}"),
        RunResult::Bool(b) => println!("{b}"),
        RunResult::Float(f) => println!("{f}"),
        RunResult::Str(s) => println!("{s}"),
    }
}
