//! CLI-owned diagnostic rendering helpers.
//!
//! `yarrow-core` owns the low-level `render()` formatting, but the CLI owns
//! *policy*: filling missing paths and printing the batch cap message.

use yarrow_core::{ColorChoice, Diagnostic, DiagnosticBatch, SourceFile};

/// Render a single diagnostic with rustc-like layout.
///
/// This is a thin wrapper around `yarrow_core::render` that guarantees
/// `diag.path` is set (so callers don't have to).
pub fn render_diag(diag: &Diagnostic, file: &SourceFile, color: ColorChoice) -> String {
    let mut diag = diag.clone();
    if diag.path.is_empty() {
        diag.path = file.path.clone();
    }
    yarrow_core::render(&diag, file, color)
}

/// Render a diagnostic batch (compile/check errors), including the
/// "aborting due to N previous errors (limit M)" message when the batch hit
/// its configured cap.
pub fn render_batch(batch: &DiagnosticBatch, file: &SourceFile, color: ColorChoice) -> String {
    let mut out = String::new();
    for diag in batch.iter() {
        out.push_str(&render_diag(diag, file, color));
    }

    if batch.is_at_limit() {
        out.push_str(&format!(
            "error: aborting due to {} previous errors (limit {})\n",
            batch.len(),
            batch.limit()
        ));
    }

    out
}
