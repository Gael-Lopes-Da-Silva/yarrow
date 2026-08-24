//! Minimal Yarrow driver: tokenize, parse, compile and run a `.yar` file.
//!
//! The program's `main` must return a single value, which is printed. Modules
//! required from user files are resolved relative to the source file's
//! directory (`"a.b"` looks for `a/b.yar` there).

use std::process::ExitCode;

use yarrow_core::{ColorChoice, CompileOptions, RunResult, Session, render_batch};

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!("usage: yarrow <file.yar>");
        return ExitCode::from(2);
    }
    let path = &args[1];
    let source = match std::fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("cannot read {path}: {e}");
            return ExitCode::from(2);
        }
    };
    let color = ColorChoice::Auto;
    let session = Session::new(CompileOptions::new(path));
    let mut artifact = match session.compile_source(source) {
        Ok(artifact) => artifact,
        Err(diags) => {
            let file = diags.file;
            let batch = diags.batch;
            eprint!("{}", render_batch(&batch, &file, color));
            return ExitCode::from(1);
        }
    };

    match artifact.run_main() {
        Ok(result) => {
            match result {
                RunResult::Void => {}
                RunResult::Int(v) => println!("{v}"),
                RunResult::Bool(b) => println!("{b}"),
                RunResult::Float(f) => println!("{f}"),
                RunResult::Str(s) => println!("{s}"),
            }
            ExitCode::SUCCESS
        }
        Err(e) => {
            let mut diag = (*e.diagnostic).clone();
            if diag.path.is_empty() {
                diag.path = artifact.file.path.clone();
            }
            eprint!("{}", yarrow_core::render(&diag, &artifact.file, color));
            ExitCode::from(1)
        }
    }
}
