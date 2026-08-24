//! Implementation of the `dump` subcommand.

use std::path::Path;
use std::process::ExitCode;

use yarrow_core::{CompileOptions, Session, SourceFile, Token};

use crate::args::{EmitKind, GlobalArgs};
use crate::diagnostics::render_batch;

/// Print tokens, AST, or Cranelift IR for `file` on stdout.
pub fn dump_file(file: &Path, emit: EmitKind, global: &GlobalArgs) -> ExitCode {
    let path = file.to_string_lossy().into_owned();
    let color = global.color.to_core();

    let source = match std::fs::read_to_string(file) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("error: cannot read {path}: {e}");
            return ExitCode::from(2);
        }
    };

    if global.verbose {
        let kind = match emit {
            EmitKind::Tokens => "tokens",
            EmitKind::Ast => "ast",
            EmitKind::Ir => "ir",
        };
        eprintln!("dumping {kind} from {path}");
    }

    let mut opts = CompileOptions::new(path);
    for p in &global.search_paths {
        opts.module_search_paths.push(p.clone());
    }
    opts.error_limit = global.error_limit;
    opts.require_main = false;

    let session = Session::new(opts);
    match emit {
        EmitKind::Tokens => match session.tokenize_source(source) {
            Ok((file, tokens)) => {
                print!("{}", format_tokens(&file, &tokens));
                ExitCode::SUCCESS
            }
            Err(diags) => {
                eprint!("{}", render_batch(&diags.batch, &diags.file, color));
                ExitCode::from(1)
            }
        },
        EmitKind::Ast => match session.parse_source(source) {
            Ok((_file, program)) => {
                println!("{program:#?}");
                ExitCode::SUCCESS
            }
            Err(diags) => {
                eprint!("{}", render_batch(&diags.batch, &diags.file, color));
                ExitCode::from(1)
            }
        },
        EmitKind::Ir => match session.compile_source(source) {
            Ok(artifact) => {
                print!("{}", artifact.emit_ir());
                ExitCode::SUCCESS
            }
            Err(diags) => {
                eprint!("{}", render_batch(&diags.batch, &diags.file, color));
                ExitCode::from(1)
            }
        },
    }
}

fn format_tokens(file: &SourceFile, tokens: &[Token]) -> String {
    let mut out = String::new();
    for token in tokens {
        let span = token.span();
        let start = file.span_start(span);
        let end = file.span_end(span);
        out.push_str(&format!(
            "{}:{}-{}:{} {:?}\t{:?}\n",
            start.line, start.column, end.line, end.column, token.kind, token.lexeme
        ));
    }
    out
}
