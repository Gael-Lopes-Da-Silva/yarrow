//! Stage 37 gate: interpreter corpus toward JIT parity (unsafe / pointers /
//! move / fs / remaining `valid/**` except the grammar tour).
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

fn interpret_source_ok(label: &str, source: &str) {
    let opts = CompileOptions::new(format!("<{label}>"));
    Session::new(opts)
        .interpret_source(source.to_string())
        .unwrap_or_else(|d| {
            eprintln!("{}", render_batch(&d.batch, &d.file, ColorChoice::Never));
            panic!("{label} must interpret successfully");
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

    // Stage 32 gate
    for rel in [
        "docs/examples/valid/06_structs_and_enums.yar",
        "docs/examples/valid/07_unions.yar",
        "docs/examples/valid/10_errors.yar",
        "docs/examples/valid/13_containers.yar",
    ] {
        interpret_ok(repo, rel);
    }

    // Stage 36: regions + defer
    interpret_ok(repo, "docs/examples/valid/09_regions_and_defer.yar");

    // Stage 36: struct field `set` (minimal adjacent fixture)
    interpret_source_ok(
        "field_set",
        r#"
Point struct
	i32 x public
	i32 y public
end

main function do
	{x 5 y 20} point mutable Point
	10 point.x set
	point.x
end with i32
"#,
    );

    // Stage 37: ownership move, unsafe/pointers, io/string, fs
    for rel in [
        "docs/examples/valid/08_ownership_borrow_move.yar",
        "docs/examples/valid/11_unsafe_pointers.yar",
        "docs/examples/valid/14_io_and_string.yar",
        "docs/examples/valid/15_fs.yar",
    ] {
        interpret_ok(repo, rel);
    }

    println!("ok: interpret Stage 21 + Stage 32 + Stage 36 + Stage 37 fixtures");
}
