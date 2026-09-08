//! Top-level `require` sorting from `docs/STYLE_GUIDE.md` (Stage 10).
//!
//! When enabled, consecutive top-level requires are reordered: `"std.…"` first,
//! then other paths, alphabetically within each group. Function-local requires
//! are left alone.

use yarrow_core::parser::ast::{Stmt, StmtKind};

/// Whether `path` is a standard-library module (`std` or `std.…`).
pub fn is_std_path(path: &str) -> bool {
    path == "std" || path.starts_with("std.")
}

fn require_path(stmt: &Stmt) -> Option<&str> {
    match &stmt.kind {
        StmtKind::Require { path, .. } => Some(path.as_str()),
        _ => None,
    }
}

/// Sort key: `(0, path)` for std, `(1, path)` otherwise.
pub fn require_sort_key(stmt: &Stmt) -> (u8, &str) {
    let path = require_path(stmt).unwrap_or("");
    let group = if is_std_path(path) { 0 } else { 1 };
    (group, path)
}

/// Sort each consecutive top-level require run in place (std first, then local;
/// alphabetical within a group). Non-require items keep their relative order.
pub fn sort_toplevel_requires(items: &[Stmt]) -> Vec<Stmt> {
    let mut out = Vec::with_capacity(items.len());
    let mut i = 0;
    while i < items.len() {
        if matches!(items[i].kind, StmtKind::Require { .. }) {
            let start = i;
            i += 1;
            while i < items.len() && matches!(items[i].kind, StmtKind::Require { .. }) {
                i += 1;
            }
            let mut group: Vec<Stmt> = items[start..i].to_vec();
            group.sort_by(|a, b| require_sort_key(a).cmp(&require_sort_key(b)));
            out.extend(group);
        } else {
            out.push(items[i].clone());
            i += 1;
        }
    }
    out
}

/// True when both statements are requires and the pair crosses from std to
/// non-std (style guide blank between those groups).
pub fn require_group_blank(prev: &Stmt, cur: &Stmt) -> bool {
    match (&prev.kind, &cur.kind) {
        (StmtKind::Require { path: a, .. }, StmtKind::Require { path: b, .. }) => {
            is_std_path(a) && !is_std_path(b)
        }
        _ => false,
    }
}
