//! Typed-at-span and definition / require probes after a successful check
//! (Stages 30 / 35).
//!
//! Collected during lowering without re-running analysis. Used by Session /
//! `CheckedProgram` and by `yarrow-lsp` hover / goto-definition.

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

/// Kind of definition / navigation probe (Stage 35).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DefKind {
    /// Local binding, function, or type in the root file.
    Definition,
    /// `require` import (alias, item name, or path string).
    Require,
}

/// Definition or require target at a byte offset in the checked root file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DefProbe {
    /// Binding / alias / item name (or last path segment for path-only hits).
    pub name: String,
    /// Span of the definition name in the root file (or require site name).
    pub def_span: Span,
    /// Root source path for [`DefKind::Definition`], or resolved module dotted
    /// path for [`DefKind::Require`].
    pub path: String,
    /// On-disk `.yar` when the loader resolved a file; `None` for embedded-only
    /// std (or other sources with no search-path hit).
    pub file_path: Option<String>,
    pub kind: DefKind,
}

#[derive(Debug, Clone)]
struct DefSite {
    /// Span that answers `definition_at` (use or declaration / require site).
    hit_span: Span,
    name: String,
    def_span: Span,
    path: String,
    file_path: Option<String>,
    kind: DefKind,
}

/// Index of definition / require sites in the root file (Stage 35).
#[derive(Debug, Clone, Default)]
pub struct DefIndex {
    sites: Vec<DefSite>,
}

impl DefIndex {
    pub(crate) fn clear(&mut self) {
        self.sites.clear();
    }

    pub(crate) fn push_definition(
        &mut self,
        hit_span: Span,
        name: impl Into<String>,
        def_span: Span,
        path: impl Into<String>,
    ) {
        self.sites.push(DefSite {
            hit_span,
            name: name.into(),
            def_span,
            path: path.into(),
            file_path: None,
            kind: DefKind::Definition,
        });
    }

    pub(crate) fn push_require(
        &mut self,
        hit_span: Span,
        name: impl Into<String>,
        def_span: Span,
        module_path: impl Into<String>,
        file_path: Option<String>,
    ) {
        self.sites.push(DefSite {
            hit_span,
            name: name.into(),
            def_span,
            path: module_path.into(),
            file_path,
            kind: DefKind::Require,
        });
    }

    /// Innermost site whose hit span contains `offset`, or `None` on a miss.
    pub fn definition_at(&self, offset: usize) -> Option<DefProbe> {
        self.sites
            .iter()
            .filter(|s| s.hit_span.lo <= offset && offset < s.hit_span.hi)
            .min_by_key(|s| s.hit_span.len())
            .map(DefSite::to_probe)
    }
}

impl DefSite {
    fn to_probe(&self) -> DefProbe {
        DefProbe {
            name: self.name.clone(),
            def_span: self.def_span,
            path: self.path.clone(),
            file_path: self.file_path.clone(),
            kind: self.kind,
        }
    }
}
