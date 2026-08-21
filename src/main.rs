//! Minimal Yarrow driver: tokenize, parse, compile and run a `.yar` file.
//!
//! The program's `main` must return a single value, which is printed. Modules
//! required from user files are resolved relative to the source file's
//! directory (`"a.b"` looks for `a/b.yar` there).

use std::process::ExitCode;

use yarrow_core::{
    ColorChoice, CompileError, Compiler, Diagnostic, Parser, RunResult, SourceFile, Tokenizer,
    render,
};

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
    let file = SourceFile::new(path, source.clone());
    let color = ColorChoice::Auto;

    let tokens = match Tokenizer::new(source.clone()).tokenize() {
        Ok(t) => t,
        Err(e) => {
            let diag = Diagnostic::error(e.code, e.message)
                .with_path(path)
                .with_primary(yarrow_core::Span::from_location(e.location), "");
            eprint!("{}", render(&diag, &file, color));
            return ExitCode::from(1);
        }
    };
    let program = match Parser::new(tokens).parse() {
        Ok(p) => p,
        Err(e) => {
            let diag = e.into_diagnostic().with_path(path);
            eprint!("{}", render(&diag, &file, color));
            return ExitCode::from(1);
        }
    };

    let mut compiler = match Compiler::new() {
        Ok(c) => c,
        Err(e) => {
            print_compile_error(&e, &file, color);
            return ExitCode::from(1);
        }
    };
    compiler.set_source_path(path);
    if let Some(dir) = std::path::Path::new(path).parent()
        && !dir.as_os_str().is_empty()
    {
        compiler.add_module_search_path(dir);
    }

    if let Err(e) = compiler.compile(&program) {
        print_compile_error(&e, &file, color);
        return ExitCode::from(1);
    }
    match compiler.run_main() {
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
            print_compile_error(&e, &file, color);
            ExitCode::from(1)
        }
    }
}

fn print_compile_error(err: &CompileError, file: &SourceFile, color: ColorChoice) {
    let mut diag = (*err.diagnostic).clone();
    if diag.path.is_empty() {
        diag.path = file.path.clone();
    }
    eprint!("{}", render(&diag, file, color));
}
