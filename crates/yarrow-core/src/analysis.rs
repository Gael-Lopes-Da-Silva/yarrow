//! Typed-at-span probes after a successful check (Stage 30).
//!
//! Collected during lowering without re-running analysis. Used by Session /
//! `CheckedProgram` and by `yarrow-lsp` typed hover.

use crate::diagnostics::Span;

/// Binding or function info at a byte offset in the checked root file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypeProbe {
    /// Binding or function name.
    pub name: String,
    /// Tightest span for this site (usually the identifier).
    pub span: Span,
    /// Resolved type string (`i32`, `list<i32>`, …) when this is a binding.
    pub ty: Option<String>,
    /// Function / method signature (and stack-effect summary) when applicable.
    pub signature: Option<String>,
}

#[derive(Debug, Clone)]
struct TypeSite {
    span: Span,
    name: String,
    ty: Option<String>,
    signature: Option<String>,
}

/// Index of typed sites in the root file, built during [`crate::Session::check_source`].
#[derive(Debug, Clone, Default)]
pub struct TypeIndex {
    sites: Vec<TypeSite>,
}

impl TypeIndex {
    pub(crate) fn clear(&mut self) {
        self.sites.clear();
    }

    pub(crate) fn push_binding(
        &mut self,
        span: Span,
        name: impl Into<String>,
        ty: impl Into<String>,
    ) {
        self.sites.push(TypeSite {
            span,
            name: name.into(),
            ty: Some(ty.into()),
            signature: None,
        });
    }

    pub(crate) fn push_signature(
        &mut self,
        span: Span,
        name: impl Into<String>,
        signature: impl Into<String>,
    ) {
        self.sites.push(TypeSite {
            span,
            name: name.into(),
            ty: None,
            signature: Some(signature.into()),
        });
    }

    /// Innermost site whose span contains `offset`, or `None` on a miss.
    pub fn type_at(&self, offset: usize) -> Option<TypeProbe> {
        self.sites
            .iter()
            .filter(|s| s.span.lo <= offset && offset < s.span.hi)
            .min_by_key(|s| s.span.len())
            .map(TypeSite::to_probe)
    }

    /// All recorded sites in the root file (for inlay hints and similar).
    pub fn probes(&self) -> impl Iterator<Item = TypeProbe> + '_ {
        self.sites.iter().map(TypeSite::to_probe)
    }
}

impl TypeSite {
    fn to_probe(&self) -> TypeProbe {
        TypeProbe {
            name: self.name.clone(),
            span: self.span,
            ty: self.ty.clone(),
            signature: self.signature.clone(),
        }
    }
}
