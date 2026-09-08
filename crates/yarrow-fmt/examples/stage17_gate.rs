//! Stage 17 gate: best-effort hygiene on broken buffers; full format on valid.

use std::path::PathBuf;

use yarrow_fmt::{FormatOptions, apply_source_hygiene, format_source, format_source_best_effort};

fn main() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let broken_path = root.join("fixtures/stage17_broken.yar");
    let broken = std::fs::read_to_string(&broken_path).expect("read broken fixture");
    let options = FormatOptions::default();

    assert!(
        format_source(&broken, &options).is_err(),
        "strict format_source must fail closed on broken input"
    );

    let out = format_source_best_effort(&broken, &options).expect("best_effort");
    assert!(out.best_effort, "expected best_effort flag");
    assert_eq!(
        out.text,
        apply_source_hygiene(&broken),
        "incomplete parse must apply hygiene only"
    );
    assert!(
        out.text.contains("x 0 == if"),
        "broken region must not be deleted: {:?}",
        out.text
    );
    assert!(!out.text.contains('\r'), "CRLF must normalize to LF");
    assert!(out.text.ends_with('\n'), "must end with a final newline");
    for line in out.text.lines() {
        assert_eq!(
            line,
            line.trim_end_matches([' ', '\t']),
            "no trailing whitespace: {line:?}"
        );
    }
    let again = format_source_best_effort(&out.text, &options).expect("idempotent best_effort");
    assert_eq!(
        again.text, out.text,
        "hygiene on broken input is idempotent"
    );
    assert!(again.best_effort);

    // Valid file still fully formats (same as strict path).
    let valid_path = root.join("fixtures/stage15_range.yar");
    let valid = std::fs::read_to_string(&valid_path).expect("read valid fixture");
    let strict = format_source(&valid, &options).expect("strict valid");
    let tolerant = format_source_best_effort(&valid, &options).expect("best_effort valid");
    assert!(!tolerant.best_effort);
    assert_eq!(tolerant.text, strict);
    let second = format_source(&strict, &options).expect("idempotent valid");
    assert_eq!(second, strict);

    println!("stage17_gate ok");
}
