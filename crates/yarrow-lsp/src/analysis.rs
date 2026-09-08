//! Run `Session::check_source` and map core diagnostics to LSP.

use tower_lsp_server::ls_types::{
    Diagnostic, DiagnosticRelatedInformation, DiagnosticSeverity, Location, NumberOrString, Range,
    Uri,
};
use yarrow_core::{
    CompileOptions, Diagnostic as CoreDiagnostic, DiagnosticBatch, Session, SessionDiagnostics,
    Severity,
};

use crate::position::{PositionEncoding, PositionMap};

/// Convert a document URI into a filesystem path for `CompileOptions::source_path`.
pub fn uri_to_source_path(uri: &Uri) -> String {
    uri.to_file_path()
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_else(|| uri.as_str().to_string())
}

/// Check `text` and build LSP diagnostics (errors and warnings).
pub fn check_document(uri: &Uri, text: &str, encoding: PositionEncoding) -> Vec<Diagnostic> {
    let path = uri_to_source_path(uri);
    let opts = CompileOptions::new(path);
    let session = Session::new(opts);
    match session.check_source(text.to_string()) {
        Ok(checked) => {
            let map = PositionMap::from_file(&checked.file, encoding);
            batch_to_lsp(uri, &checked.warnings, &map)
        }
        Err(SessionDiagnostics { file, batch }) => {
            let map = PositionMap::from_file(&file, encoding);
            batch_to_lsp(uri, &batch, &map)
        }
    }
}

fn batch_to_lsp(uri: &Uri, batch: &DiagnosticBatch, map: &PositionMap<'_>) -> Vec<Diagnostic> {
    batch.iter().map(|d| core_to_lsp(uri, d, map)).collect()
}

fn core_to_lsp(uri: &Uri, diag: &CoreDiagnostic, map: &PositionMap<'_>) -> Diagnostic {
    let (primary, secondaries): (Vec<_>, Vec<_>) = diag.labels.iter().partition(|l| l.primary);

    let range = primary
        .first()
        .or(secondaries.first())
        .map(|l| map.range(l.span))
        .unwrap_or_else(|| Range::new(map.position(0), map.position(0)));

    let related = if secondaries.is_empty() {
        None
    } else {
        Some(
            secondaries
                .into_iter()
                .map(|label| DiagnosticRelatedInformation {
                    location: Location::new(uri.clone(), map.range(label.span)),
                    message: if label.message.is_empty() {
                        diag.message.clone()
                    } else {
                        label.message.clone()
                    },
                })
                .collect(),
        )
    };

    let mut message = diag.message.clone();
    for note in &diag.notes {
        message.push_str("\nnote: ");
        message.push_str(note);
    }
    for help in &diag.helps {
        message.push_str("\nhelp: ");
        message.push_str(help);
    }

    Diagnostic {
        range,
        severity: Some(severity_to_lsp(diag.severity)),
        code: Some(NumberOrString::String(diag.code.clone())),
        source: Some("yarrow".into()),
        message,
        related_information: related,
        ..Diagnostic::default()
    }
}

fn severity_to_lsp(severity: Severity) -> DiagnosticSeverity {
    match severity {
        Severity::Error => DiagnosticSeverity::ERROR,
        Severity::Warning => DiagnosticSeverity::WARNING,
        Severity::Note => DiagnosticSeverity::INFORMATION,
        Severity::Help => DiagnosticSeverity::HINT,
    }
}
