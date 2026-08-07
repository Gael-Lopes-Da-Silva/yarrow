//! Minimal Yarrow driver: tokenize, parse, compile and run a `.yar` file.
//!
//! The program's `main` must return a single value, which is printed. Modules
//! required from user files are resolved relative to the source file's
//! directory (`"a.b"` looks for `a/b.yar` there).

use std::process::ExitCode;

use yarrow_core::{Compiler, Parser, Tokenizer};

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

    let tokens = match Tokenizer::new(source).tokenize() {
        Ok(t) => t,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::from(1);
        }
    };
    let program = match Parser::new(tokens).parse() {
        Ok(p) => p,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::from(1);
        }
    };

    let mut compiler = match Compiler::new() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::from(1);
        }
    };
    if let Some(dir) = std::path::Path::new(path).parent()
        && !dir.as_os_str().is_empty()
    {
        compiler.add_module_search_path(dir);
    }

    if let Err(e) = compiler.compile(&program) {
        eprintln!("{e}");
        return ExitCode::from(1);
    }
    match compiler.run_main() {
        Ok(code) => {
            println!("{code}");
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("{e}");
            ExitCode::from(1)
        }
    }
}
