//! Stage 40 gate: ICE-tagged session failures (`E999`).
//!
//! ```bash
//! cargo run -p yarrow_core --example check_ice
//! ```

use yarrow_core::{
    ColorChoice, CompileOptions, ICE_CODE, Session, SessionFailureKind, explain_code, render_batch,
};

fn main() {
    let opts = CompileOptions::new("ice-gate.yar");
    let session = Session::new(opts);
    let diags = session.debug_trigger_ice("stage 40 gate: deliberate ICE");

    assert!(
        diags.is_ice(),
        "debug_trigger_ice must tag the batch as ICE"
    );
    assert_eq!(diags.failure_kind(), SessionFailureKind::Ice);
    assert!(
        diags.batch.iter().any(|d| d.code == ICE_CODE && d.is_ice()),
        "expected diagnostic code {ICE_CODE}"
    );

    let entry = explain_code(ICE_CODE).expect("E999 must be in the explain catalog");
    assert_eq!(entry.code, ICE_CODE);

    // Ordinary invalid programs stay User (exit 1), never Ice.
    let bad = Session::new(CompileOptions::new("no-main.yar"))
        .check_source("1 pop\n".into())
        .expect_err("missing main must fail");
    assert!(
        !bad.is_ice(),
        "user diagnostics must not be marked ICE; got {}",
        render_batch(&bad.batch, &bad.file, ColorChoice::Never)
    );
    assert_eq!(bad.failure_kind(), SessionFailureKind::User);

    println!("ok: E999 ICE path + user diagnostics stay non-ICE");
}
