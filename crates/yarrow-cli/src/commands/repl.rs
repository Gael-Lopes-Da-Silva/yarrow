//! Interactive interpreter (`yarrow repl`).
//!
//! Line-oriented loop over [`yarrow_core::EvalContext`]. Snippets without a
//! top-level `function` / `require` are wrapped as `main` bodies. A leftover
//! stack value (W403) is re-checked with `end with <type>` so expressions
//! print as [`yarrow_core::RunResult`].

use std::io::{self, BufRead, Write};
use std::process::ExitCode;

use yarrow_core::{
    CheckedProgram, ColorChoice, CompileOptions, DiagnosticBatch, EvalContext, ExecutionMode,
    Session,
};

use crate::args::GlobalArgs;
use crate::commands::print_run_result;
use crate::diagnostics::render_batch;

/// Start an interactive interpret loop until EOF / `exit` / `quit`.
pub fn run_repl(global: &GlobalArgs) -> ExitCode {
    let color = global.color.to_core();

    let mut opts = CompileOptions::new("<repl>");
    for p in &global.search_paths {
        opts.module_search_paths.push(p.clone());
    }
    opts.error_limit = global.error_limit;
    opts.mode = ExecutionMode::Interpret;

    let session = Session::new(opts);
    let mut ctx = EvalContext::new();
    for p in &global.search_paths {
        ctx.add_module_search_path(p.clone());
    }

    if !global.quiet {
        eprintln!("yarrow repl (interpret). Type `exit` or Ctrl-D to quit.");
    }

    let stdin = io::stdin();
    let mut lines = stdin.lock().lines();

    loop {
        if !global.quiet {
            eprint!(">>> ");
            let _ = io::stderr().flush();
        }

        let Some(read) = lines.next() else {
            if !global.quiet {
                eprintln!();
            }
            break;
        };

        let line = match read {
            Ok(line) => line,
            Err(e) => {
                eprintln!("error: failed to read stdin: {e}");
                return ExitCode::from(2);
            }
        };

        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if trimmed == "exit" || trimmed == "quit" {
            break;
        }

        eval_line(&session, &mut ctx, trimmed, color, global.progress());
        let _ = io::stdout().flush();
    }

    ExitCode::SUCCESS
}

fn eval_line(
    session: &Session,
    ctx: &mut EvalContext,
    line: &str,
    color: ColorChoice,
    progress: bool,
) {
    let mut source = if looks_like_program(line) {
        format!("{line}\n")
    } else {
        wrap_as_main(line, None)
    };

    if progress {
        eprintln!("repl source:\n{source}");
    }

    let mut checked = match session.check_source(source.clone()) {
        Ok(checked) => checked,
        Err(first) => {
            // Wrapped body failed; try the line as a raw unit (e.g. pasted main).
            if !looks_like_program(line) {
                match session.check_source(format!("{line}\n")) {
                    Ok(checked) => checked,
                    Err(_) => {
                        eprint!("{}", render_batch(&first.batch, &first.file, color));
                        return;
                    }
                }
            } else {
                eprint!("{}", render_batch(&first.batch, &first.file, color));
                return;
            }
        }
    };

    if let Some(ty) = unused_stack_type(&checked.warnings) {
        source = wrap_as_main(line, Some(&ty));
        if progress {
            eprintln!("repl promote end with {ty}:\n{source}");
        }
        checked = match session.check_source(source) {
            Ok(checked) => checked,
            Err(diags) => {
                eprint!("{}", render_batch(&diags.batch, &diags.file, color));
                return;
            }
        };
    }

    if !checked.warnings.is_empty() {
        eprint!("{}", render_batch(&checked.warnings, &checked.file, color));
    }

    run_checked(session, ctx, checked, color);
}

fn run_checked(
    session: &Session,
    ctx: &mut EvalContext,
    checked: CheckedProgram,
    color: ColorChoice,
) {
    if let Err(e) = ctx.load_program(&checked.program) {
        let batch = interpret_error_batch(e, session.options.error_limit);
        eprint!("{}", render_batch(&batch, &checked.file, color));
        return;
    }

    match ctx.run_entry(&session.options.entry_name) {
        Ok(result) => print_run_result(result),
        Err(e) => {
            let batch = interpret_error_batch(e, session.options.error_limit);
            eprint!("{}", render_batch(&batch, &checked.file, color));
        }
    }
}

fn interpret_error_batch(err: yarrow_core::InterpretError, error_limit: usize) -> DiagnosticBatch {
    let compile_err = err.into_compile_error();
    let mut batch = DiagnosticBatch::with_limit(error_limit);
    batch.push((*compile_err.diagnostic).clone());
    batch
}

fn wrap_as_main(body: &str, ret: Option<&str>) -> String {
    match ret {
        Some(ty) => format!("main function do\n\t{body}\nend with {ty}\n"),
        None => format!("main function do\n\t{body}\nend\n"),
    }
}

fn looks_like_program(src: &str) -> bool {
    src.split_whitespace()
        .any(|w| w == "function" || w == "require")
}

fn unused_stack_type(warnings: &DiagnosticBatch) -> Option<String> {
    const PREFIX: &str = "unused value of type ";
    const SUFFIX: &str = " left on the stack";
    for diag in warnings.iter() {
        if diag.code != "W403" {
            continue;
        }
        if let Some(rest) = diag.message.strip_prefix(PREFIX)
            && let Some(ty) = rest.strip_suffix(SUFFIX)
            && !ty.is_empty()
        {
            return Some(ty.to_string());
        }
    }
    None
}
