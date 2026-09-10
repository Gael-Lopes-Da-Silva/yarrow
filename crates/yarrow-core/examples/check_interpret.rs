//! Stage 32 gate: interpreter corpus toward JIT parity.
//!
//! ```bash
//! cargo run -p yarrow_core --example check_interpret
//! ```

use std::path::PathBuf;

use yarrow_core::{ColorChoice, CompileOptions, Session, render_batch};

fn interpret_ok(repo: &std::path::Path, rel: &str) {
    let path = repo.join(rel);
    let source = std::fs::read_to_string(&path).unwrap_or_else(|e| {
        panic!("read {rel}: {e}");
    });
    let opts = CompileOptions::new(path.display().to_string());
    Session::new(opts)
        .interpret_source(source)
        .unwrap_or_else(|d| {
            eprintln!("{}", render_batch(&d.batch, &d.file, ColorChoice::Never));
            panic!("{rel} must interpret successfully");
        });
}

fn main() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let repo = manifest_dir
        .parent()
        .and_then(|p| p.parent())
        .expect("crates/yarrow-core → repo root");

    // Stage 21 corpus
    for rel in [
        "docs/examples/valid/01_hello.yar",
        "docs/examples/valid/02_arithmetic_and_stack.yar",
        "docs/examples/valid/03_variables_and_typeof.yar",
        "docs/examples/valid/04_functions.yar",
        "docs/examples/valid/05_control_flow.yar",
        "docs/examples/valid/12_modules.yar",
    ] {
        interpret_ok(repo, rel);
    }

    // Stage 32 gate (stdout matches JIT: green / hello! / missing / containers ok)
    for rel in [
        "docs/examples/valid/06_structs_and_enums.yar",
        "docs/examples/valid/07_unions.yar",
        "docs/examples/valid/10_errors.yar",
        "docs/examples/valid/13_containers.yar",
    ] {
        interpret_ok(repo, rel);
    }

    println!("ok: interpret Stage 21 + Stage 32 fixtures");
}
