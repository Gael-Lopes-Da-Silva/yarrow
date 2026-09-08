//! Resolve `require` dotted paths to on-disk `.yar` files when available.
//!
//! Std modules are embedded in `yarrow-core` at compile time. For navigation we
//! prefer real `lib/std/**` paths from the repo / adjacent crate layout; if no
//! file exists, callers skip the jump (no virtual URI scheme in v1).

use std::path::{Path, PathBuf};

use yarrow_core::parser::ast::StmtKind;
use yarrow_core::{CompileOptions, Session};

/// On-disk module target for go-to-definition.
#[derive(Debug, Clone)]
pub struct ModuleTarget {
    pub path: PathBuf,
    /// Item import (`"std.math.sqrt" require`) binds only this top-level name.
    pub item: Option<String>,
}

/// Resolve a `require` path string to a file under search roots / `lib/std`.
pub fn resolve_require_file(
    source_path: &str,
    require_path: &str,
    search_paths: &[PathBuf],
) -> Option<ModuleTarget> {
    let (module_path, item) = classify_require(source_path, require_path, search_paths)?;
    let path = locate_module_file(source_path, &module_path, search_paths)?;
    Some(ModuleTarget { path, item })
}

/// Parent-first item import vs whole-module (mirrors core `resolve_require`).
fn classify_require(
    source_path: &str,
    require_path: &str,
    search_paths: &[PathBuf],
) -> Option<(String, Option<String>)> {
    if let Some((parent, last)) = require_path.rsplit_once('.')
        && locate_module_file(source_path, parent, search_paths).is_some()
        && module_has_top_level_function(source_path, parent, last, search_paths)
    {
        return Some((parent.to_string(), Some(last.to_string())));
    }
    if locate_module_file(source_path, require_path, search_paths).is_some() {
        return Some((require_path.to_string(), None));
    }
    None
}

fn module_has_top_level_function(
    source_path: &str,
    module_path: &str,
    name: &str,
    search_paths: &[PathBuf],
) -> bool {
    let Some(path) = locate_module_file(source_path, module_path, search_paths) else {
        return false;
    };
    let Ok(text) = std::fs::read_to_string(&path) else {
        return false;
    };
    let opts = CompileOptions::new(path.to_string_lossy().into_owned());
    let session = Session::new(opts);
    let Ok((_, program)) = session.parse_source(text) else {
        return false;
    };
    program.items.iter().any(|item| {
        matches!(&item.kind, StmtKind::Function(f) if f.name == name)
            || matches!(&item.kind, StmtKind::Implement(imp) if imp.functions.iter().any(|f| f.name == name))
    })
}

fn locate_module_file(
    source_path: &str,
    module_path: &str,
    search_paths: &[PathBuf],
) -> Option<PathBuf> {
    if let Some(rest) = module_path.strip_prefix("std.") {
        let rel = PathBuf::from(rest.replace('.', "/")).with_extension("yar");
        for root in std_lib_roots(source_path) {
            let candidate = root.join(&rel);
            if candidate.is_file() {
                return canonical_or_self(candidate);
            }
        }
    }

    let rel = PathBuf::from(module_path.replace('.', "/")).with_extension("yar");
    if let Some(parent) = Path::new(source_path).parent() {
        let candidate = parent.join(&rel);
        if candidate.is_file() {
            return canonical_or_self(candidate);
        }
    }
    for root in search_paths {
        let candidate = root.join(&rel);
        if candidate.is_file() {
            return canonical_or_self(candidate);
        }
    }
    None
}

fn std_lib_roots(source_path: &str) -> Vec<PathBuf> {
    let mut roots = Vec::new();
    let lsp_manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    roots.push(lsp_manifest.join("../yarrow-core/lib/std"));

    let mut cur = Path::new(source_path).parent();
    while let Some(dir) = cur {
        let nested = dir.join("crates/yarrow-core/lib/std");
        if nested.is_dir() {
            roots.push(nested);
        }
        let flat = dir.join("lib/std");
        if flat.is_dir() {
            roots.push(flat);
        }
        cur = dir.parent();
    }
    roots
}

fn canonical_or_self(path: PathBuf) -> Option<PathBuf> {
    Some(path.canonicalize().unwrap_or(path))
}

/// Byte span of a top-level function / method named `item` in `path`, if any.
pub fn item_name_span(path: &Path, item: &str) -> Option<yarrow_core::Span> {
    let text = std::fs::read_to_string(path).ok()?;
    let path_str = path.to_string_lossy().into_owned();
    let opts = CompileOptions::new(path_str.clone());
    let session = Session::new(opts);
    let (file, program) = session.parse_source(text).ok()?;
    let source = file.source.as_str();
    for stmt in &program.items {
        match &stmt.kind {
            StmtKind::Function(f) if f.name == item => {
                return name_span_in(source, stmt.span, item);
            }
            StmtKind::Implement(imp) => {
                for f in &imp.functions {
                    if f.name == item {
                        return name_span_in(source, stmt.span, item);
                    }
                }
            }
            _ => {}
        }
    }
    None
}

fn name_span_in(source: &str, span: yarrow_core::Span, name: &str) -> Option<yarrow_core::Span> {
    if name.is_empty() || span.lo > span.hi || span.hi > source.len() {
        return None;
    }
    let slice = &source[span.lo..span.hi];
    let rel = slice.find(name)?;
    let lo = span.lo + rel;
    let hi = lo + name.len();
    Some(yarrow_core::Span::new(lo, hi))
}
