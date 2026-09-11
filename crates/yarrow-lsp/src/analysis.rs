//! Run `Session::check_source` / `check_project` and map core diagnostics to LSP.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use tower_lsp_server::ls_types::{
    Diagnostic, DiagnosticRelatedInformation, DiagnosticSeverity, Location, NumberOrString, Range,
    Uri,
};
use yarrow_core::{
    DEFAULT_ERROR_LIMIT, Diagnostic as CoreDiagnostic, DiagnosticBatch, ProjectOptions,
    ProjectRoot, Session, SessionDiagnostics, Severity, SourceFile, Span,
};

use crate::config::LspConfig;
use crate::position::{PositionEncoding, PositionMap};

/// Convert a document URI into a filesystem path for `CompileOptions::source_path`.
pub fn uri_to_source_path(uri: &Uri) -> String {
    uri.to_file_path()
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_else(|| uri.as_str().to_string())
}

/// Build a `file://` URI for an on-disk path when possible.
pub fn path_to_uri(path: &Path) -> Option<Uri> {
    Uri::from_file_path(path)
}

/// Stable key for matching open buffers to project roots.
pub fn path_key(path: &Path) -> String {
    std::fs::canonicalize(path)
        .unwrap_or_else(|_| {
            if path.is_absolute() {
                path.to_path_buf()
            } else {
                std::env::current_dir()
                    .map(|cwd| cwd.join(path))
                    .unwrap_or_else(|_| path.to_path_buf())
            }
        })
        .to_string_lossy()
        .into_owned()
}

/// Check `text` and build LSP diagnostics (errors and warnings).
pub fn check_document(
    uri: &Uri,
    text: &str,
    encoding: PositionEncoding,
    config: &LspConfig,
) -> Vec<Diagnostic> {
    let path = uri_to_source_path(uri);
    let session = config.session(path);
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

/// One root's diagnostics after a project check (or setup failure).
#[derive(Debug, Clone)]
pub struct RootDiagnostics {
    pub uri: Uri,
    pub path: PathBuf,
    pub items: Vec<Diagnostic>,
}

/// Check every configured project root via [`Session::check_project`].
///
/// `overlays` maps [`path_key`] → open-buffer text. Roots without an overlay
/// are read from disk. Missing / unreadable roots yield `E383` on that root's
/// URI (no hang).
pub fn check_project_roots(
    encoding: PositionEncoding,
    config: &LspConfig,
    overlays: &HashMap<String, String>,
) -> Vec<RootDiagnostics> {
    if config.project_roots.is_empty() {
        return Vec::new();
    }

    let mut loaded: Vec<(PathBuf, Uri, ProjectRoot)> = Vec::new();
    let mut out: Vec<RootDiagnostics> = Vec::new();

    for root_path in &config.project_roots {
        let Some(uri) = path_to_uri(root_path) else {
            continue;
        };
        let key = path_key(root_path);
        let display = root_path.to_string_lossy().into_owned();

        let source = if let Some(text) = overlays.get(&key) {
            text.clone()
        } else {
            match std::fs::read_to_string(root_path) {
                Ok(s) => s,
                Err(e) => {
                    out.push(e383_root(
                        uri,
                        root_path.clone(),
                        &display,
                        &e.to_string(),
                        encoding,
                    ));
                    continue;
                }
            }
        };

        loaded.push((
            root_path.clone(),
            uri,
            ProjectRoot {
                path: display,
                source,
            },
        ));
    }

    if loaded.is_empty() {
        return out;
    }

    let options = ProjectOptions {
        roots: loaded.iter().map(|(_, _, r)| r.clone()).collect(),
        module_search_paths: config.search_paths.clone(),
        require_main: true,
        entry_name: config.entry_name.clone(),
        error_limit: DEFAULT_ERROR_LIMIT,
    };

    match Session::check_project(&options) {
        Ok(checked) => {
            for prog in checked.roots {
                let path = PathBuf::from(&prog.file.path);
                let Some(uri) = path_to_uri(&path) else {
                    continue;
                };
                let map = PositionMap::from_file(&prog.file, encoding);
                out.push(RootDiagnostics {
                    uri: uri.clone(),
                    path,
                    items: batch_to_lsp(&uri, &prog.warnings, &map),
                });
            }
            // Keep any E383 entries for roots that failed to load alongside
            // successful checks on the remaining roots.
            out
        }
        Err(SessionDiagnostics { file, batch }) => {
            let path = PathBuf::from(&file.path);
            if let Some(uri) = path_to_uri(&path) {
                let map = PositionMap::from_file(&file, encoding);
                out.push(RootDiagnostics {
                    uri: uri.clone(),
                    path,
                    items: batch_to_lsp(&uri, &batch, &map),
                });
            }
            // Clear stale diagnostics on roots that did not produce a batch.
            for (root_path, uri, _) in &loaded {
                let key = path_key(root_path);
                if out.iter().any(|r| path_key(&r.path) == key) {
                    continue;
                }
                out.push(RootDiagnostics {
                    uri: uri.clone(),
                    path: root_path.clone(),
                    items: Vec::new(),
                });
            }
            out
        }
    }
}

fn e383_root(
    uri: Uri,
    path: PathBuf,
    display: &str,
    err: &str,
    encoding: PositionEncoding,
) -> RootDiagnostics {
    let file = SourceFile::new(display.to_string(), String::new());
    let map = PositionMap::from_file(&file, encoding);
    let mut batch = DiagnosticBatch::new();
    batch.push(
        CoreDiagnostic::error(
            "E383",
            format!("project root not found or unreadable: {display} ({err})"),
        )
        .with_path(display.to_string())
        .with_primary(Span::default(), "")
        .with_help("pass an existing `.yar` path in projectRoots, or open the buffer"),
    );
    RootDiagnostics {
        uri: uri.clone(),
        path,
        items: batch_to_lsp(&uri, &batch, &map),
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
