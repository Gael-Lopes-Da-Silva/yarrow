//! Stage 15 gate: range format expands to top-level item; matches full-doc slice; idempotent.

use std::path::PathBuf;
use yarrow_fmt::{ByteRange, FormatOptions, format_range, format_source};

fn main() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let path = root.join("fixtures/stage15_range.yar");
    let source = std::fs::read_to_string(&path).expect("read fixture");
    let options = FormatOptions::default();

    let full = format_source(&source, &options).expect("full format");

    // Select a byte inside the messy `lookup` body (after "lookup function").
    let needle = "lookup function";
    let lookup_at = source.find(needle).expect("lookup");
    let inside = lookup_at + needle.len() + 5;
    let edit =
        format_range(&source, ByteRange::new(inside, inside + 1), &options).expect("range format");

    assert!(
        edit.expanded,
        "expected expansion from a one-byte selection inside lookup"
    );
    assert!(
        edit.range.start <= lookup_at,
        "cover should start at or before lookup"
    );
    let cover = &source[edit.range.start..edit.range.end];
    assert!(
        cover.contains("lookup function"),
        "cover must include lookup"
    );
    assert!(
        !cover.contains("main function"),
        "cover must not swallow main: {cover:?}"
    );
    assert!(
        !cover.contains("AppError error"),
        "cover must not swallow AppError: {cover:?}"
    );

    let applied = edit.apply(&source);
    assert!(
        applied.contains(edit.new_text.trim_end_matches('\n')),
        "applied buffer should contain new_text"
    );

    // new_text should appear as a contiguous region in the full-document format.
    assert!(
        full.contains(edit.new_text.trim_end_matches('\n')) || full.contains(&edit.new_text),
        "full format should contain range new_text\nfull:\n{full}\nnew:\n{}",
        edit.new_text
    );

    // Second call on the applied buffer over the rewritten span is a no-op.
    let rewritten_start = edit.range.start;
    let rewritten_end = edit.range.start + edit.new_text.len();
    let second = format_range(
        &applied,
        ByteRange::new(rewritten_start, rewritten_end.min(applied.len())),
        &options,
    )
    .expect("second range");
    assert!(
        second.is_noop(&applied),
        "second range format should be a no-op\nrange={:?}\nnew={:?}\nold={:?}",
        second.range,
        second.new_text,
        &applied[second.range.start..second.range.end]
    );

    // Parse failure surfaces as FormatError::Parse (not a partial edit).
    let bad = format_range("not valid {{{", ByteRange::new(0, 3), &options);
    assert!(bad.is_err(), "expected parse error");

    eprintln!("stage15_gate ok");
}
