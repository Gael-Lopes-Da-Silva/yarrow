//! Top-level file layout reorder from `docs/STYLE_GUIDE.md` (Stage 13).
//!
//! When enabled, top-level items are reordered: requires, types (with matching
//! `implement` blocks immediately after), private helpers, public API, then
//! `main`. Visibility comes from AST flags / keywords, not names. Stable within
//! each category. Function-local items are never moved.

use yarrow_core::parser::ast::{Stmt, StmtKind, Visibility};

/// Style-guide top-level category for layout reorder.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LayoutKind {
    Require,
    Type,
    Implement,
    PrivateFn,
    PublicFn,
    Main,
    Other,
}

/// Classify a top-level statement for file layout.
pub fn layout_kind(stmt: &Stmt) -> LayoutKind {
    match &stmt.kind {
        StmtKind::Require { .. } => LayoutKind::Require,
        StmtKind::Struct(_) | StmtKind::Enum(_) | StmtKind::Union(_) | StmtKind::Error(_) => {
            LayoutKind::Type
        }
        StmtKind::Implement(_) => LayoutKind::Implement,
        StmtKind::Function(func) => {
            if func.name == "main" {
                LayoutKind::Main
            } else if matches!(func.visibility, Some(Visibility::Public)) {
                LayoutKind::PublicFn
            } else {
                // `None` (bare `name function`) and `Private` are private helpers.
                LayoutKind::PrivateFn
            }
        }
        _ => LayoutKind::Other,
    }
}

fn type_name(stmt: &Stmt) -> Option<&str> {
    match &stmt.kind {
        StmtKind::Struct(d) => Some(d.name.as_str()),
        StmtKind::Enum(d) => Some(d.name.as_str()),
        StmtKind::Union(d) => Some(d.name.as_str()),
        StmtKind::Error(d) => Some(d.name.as_str()),
        _ => None,
    }
}

fn implement_target(stmt: &Stmt) -> Option<&str> {
    match &stmt.kind {
        StmtKind::Implement(d) => Some(d.target.as_str()),
        _ => None,
    }
}

/// Indices of `items` in style-guide file-layout order (stable within groups).
///
/// Types keep their relative order; each type is followed by `implement` blocks
/// targeting that name (stable among those). Orphan implements follow all types.
/// `Other` sits after public API and before `main`.
pub fn reorder_toplevel_indices(items: &[Stmt]) -> Vec<usize> {
    let mut requires = Vec::new();
    let mut types: Vec<(usize, &str)> = Vec::new();
    let mut implements: Vec<(usize, &str)> = Vec::new();
    let mut private_fns = Vec::new();
    let mut public_fns = Vec::new();
    let mut mains = Vec::new();
    let mut others = Vec::new();

    for (idx, stmt) in items.iter().enumerate() {
        match layout_kind(stmt) {
            LayoutKind::Require => requires.push(idx),
            LayoutKind::Type => {
                if let Some(name) = type_name(stmt) {
                    types.push((idx, name));
                } else {
                    others.push(idx);
                }
            }
            LayoutKind::Implement => {
                if let Some(target) = implement_target(stmt) {
                    implements.push((idx, target));
                } else {
                    others.push(idx);
                }
            }
            LayoutKind::PrivateFn => private_fns.push(idx),
            LayoutKind::PublicFn => public_fns.push(idx),
            LayoutKind::Main => mains.push(idx),
            LayoutKind::Other => others.push(idx),
        }
    }

    let mut out = Vec::with_capacity(items.len());
    out.extend(requires);

    let mut used_impl = vec![false; implements.len()];
    for &(type_idx, name) in &types {
        out.push(type_idx);
        for (j, &(impl_idx, target)) in implements.iter().enumerate() {
            if !used_impl[j] && target == name {
                out.push(impl_idx);
                used_impl[j] = true;
            }
        }
    }
    for (j, &(impl_idx, _)) in implements.iter().enumerate() {
        if !used_impl[j] {
            out.push(impl_idx);
        }
    }

    out.extend(private_fns);
    out.extend(public_fns);
    out.extend(others);
    out.extend(mains);
    out
}

/// Reorder a cloned top-level item list (AST-only helper; printer attaches comments).
pub fn reorder_toplevel_items(items: &[Stmt]) -> Vec<Stmt> {
    reorder_toplevel_indices(items)
        .into_iter()
        .map(|i| items[i].clone())
        .collect()
}
