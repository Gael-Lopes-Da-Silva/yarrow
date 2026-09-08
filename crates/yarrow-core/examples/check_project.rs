//! Stage 28 gate: multi-root `check_project` on `docs/examples/project/`.
//!
//! ```bash
//! cargo run -p yarrow_core --example check_project
//! ```

use std::path::PathBuf;

use yarrow_core::{
    ColorChoice, CompileOptions, ProjectOptions, Session, check_project, render_batch,
};

fn main() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let repo = manifest_dir
        .parent()
        .and_then(|p| p.parent())
        .expect("crates/yarrow-core → repo root");
    let project = repo.join("docs/examples/project");
    let root_a = project.join("root_a.yar");
    let root_b = project.join("root_b.yar");

    let opts = ProjectOptions::from_root_paths([&root_a, &root_b]).unwrap_or_else(|d| {
        eprintln!("{}", render_batch(&d.batch, &d.file, ColorChoice::Never));
        std::process::exit(1);
    });

    let checked = check_project(&opts).unwrap_or_else(|d| {
        eprintln!("{}", render_batch(&d.batch, &d.file, ColorChoice::Never));
        std::process::exit(1);
    });

    assert_eq!(checked.roots.len(), 2);
    assert!(
        checked.graph.modules.iter().any(|m| m == "shared.util"),
        "expected shared.util in module graph, got {:?}",
        checked.graph.modules
    );

    // Cycle fixture → E382
    let cycle = repo.join("docs/examples/invalid/16_require_cycle.yar");
    let source = std::fs::read_to_string(&cycle).expect("cycle fixture");
    let mut copts = CompileOptions::new(cycle.display().to_string());
    copts.mode = yarrow_core::ExecutionMode::Check;
    let err = Session::new(copts)
        .check_source(source)
        .expect_err("cycle must fail");
    let codes: Vec<_> = err.batch.iter().map(|d| d.code.as_str()).collect();
    assert!(codes.contains(&"E382"), "expected E382, got {codes:?}");

    // Missing root → E383
    let missing = ProjectOptions::from_root_paths([project.join("no_such_root.yar")]);
    let err = missing.expect_err("missing root");
    assert_eq!(
        err.batch.iter().next().map(|d| d.code.as_str()),
        Some("E383")
    );

    println!(
        "ok: {} roots, {} modules, cycle=E382, missing=E383",
        checked.roots.len(),
        checked.graph.modules.len()
    );
}
