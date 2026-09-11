//! Same-file + `require` cross-file `textDocument/definition`.
//!
//! Prefers core [`CheckedProgram::definition_at`] (Stage 22) when check succeeds;
//! falls back to the AST / require walk on miss or check failure.

use tower_lsp_server::ls_types::{GotoDefinitionResponse, Location, Position, Range, Uri};
use yarrow_core::parser::ast::{Function, Stmt, StmtKind};
use yarrow_core::{DefKind, Program, SourceFile, Span, TokenKind, Tokenizer};

use crate::config::LspConfig;
use crate::modules::{self, item_name_span};
use crate::position::{PositionEncoding, PositionMap};

/// Resolve definition at `position` in `text`, or `None` if unresolved / parse failure.
pub fn goto_definition(
    uri: &Uri,
    path: &str,
    text: &str,
    encoding: PositionEncoding,
    position: Position,
    config: &LspConfig,
) -> Option<GotoDefinitionResponse> {
    let session = config.session(path);
    let map_file = SourceFile::new(path.to_string(), text.to_string());
    let map = PositionMap::from_file(&map_file, encoding);
    let offset = map.offset(position)?;

    if let Ok(checked) = session.check_source(text.to_string())
        && let Some(probe) = checked.definition_at(offset)
    {
        let probe_map = PositionMap::from_file(&checked.file, encoding);
        if let Some(resp) = location_from_probe(&probe, &probe_map, uri, path, encoding, config) {
            return Some(resp);
        }
    }

    let (file, program) = session.parse_source(text.to_string()).ok()?;
    let map = PositionMap::from_file(&file, encoding);
    let offset = map.offset(position)?;
    let name = identifier_at(&file.source, offset)?;
    let decls = collect_decls(&file, &program);
    let decl = resolve(&decls, &name, offset)?;

    if let Some(require_path) = &decl.require_path
        && let Some(target) =
            modules::resolve_require_file(path, require_path, &config.search_paths)
        && let Some(target_uri) = Uri::from_file_path(&target.path)
    {
        let range = match &target.item {
            Some(item) => std::fs::read_to_string(&target.path)
                .ok()
                .and_then(|target_text| {
                    let target_file =
                        SourceFile::new(target.path.to_string_lossy().into_owned(), target_text);
                    let target_map = PositionMap::from_file(&target_file, encoding);
                    item_name_span(&target.path, item).map(|span| target_map.range(span))
                })
                .unwrap_or_else(|| Range::new(Position::new(0, 0), Position::new(0, 0))),
            None => Range::new(Position::new(0, 0), Position::new(0, 0)),
        };
        return Some(GotoDefinitionResponse::Scalar(Location::new(
            target_uri, range,
        )));
    }

    let range = map.range(decl.name_span);
    Some(GotoDefinitionResponse::Scalar(Location::new(
        uri.clone(),
        range,
    )))
}

fn location_from_probe(
    probe: &yarrow_core::DefProbe,
    map: &PositionMap,
    uri: &Uri,
    source_path: &str,
    encoding: PositionEncoding,
    config: &LspConfig,
) -> Option<GotoDefinitionResponse> {
    match probe.kind {
        DefKind::Definition => {
            let range = map.range(probe.def_span);
            Some(GotoDefinitionResponse::Scalar(Location::new(
                uri.clone(),
                range,
            )))
        }
        DefKind::Require => {
            let target_path = probe
                .file_path
                .as_ref()
                .map(std::path::PathBuf::from)
                .or_else(|| {
                    modules::resolve_require_file(source_path, &probe.path, &config.search_paths)
                        .map(|t| t.path)
                })?;
            let target_uri = Uri::from_file_path(&target_path)?;
            let range = std::fs::read_to_string(&target_path)
                .ok()
                .and_then(|target_text| {
                    let target_file =
                        SourceFile::new(target_path.to_string_lossy().into_owned(), target_text);
                    let target_map = PositionMap::from_file(&target_file, encoding);
                    item_name_span(&target_path, &probe.name).map(|span| target_map.range(span))
                })
                .unwrap_or_else(|| Range::new(Position::new(0, 0), Position::new(0, 0)));
            Some(GotoDefinitionResponse::Scalar(Location::new(
                target_uri, range,
            )))
        }
    }
}

/// Binding kind for semantic highlighting / navigation helpers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DeclKind {
    Function,
    Variable,
    Type,
    Property,
    /// Module / require alias (highlighted as variable).
    Module,
}

#[derive(Debug, Clone)]
pub(crate) struct Decl {
    pub name: String,
    pub name_span: Span,
    /// Region where this declaration is visible.
    pub scope: Span,
    pub kind: DeclKind,
    /// When set, this decl is a `require` binding (`path` string from the AST).
    pub require_path: Option<String>,
}

pub(crate) fn identifier_at(source: &str, offset: usize) -> Option<String> {
    identifier_span_at(source, offset).map(|(name, _)| name)
}

/// Identifier lexeme and byte span at `offset` (or at the exclusive end of the word).
pub(crate) fn identifier_span_at(source: &str, offset: usize) -> Option<(String, Span)> {
    let mut tokenizer = Tokenizer::new(source.to_string());
    let tokens = tokenizer.tokenize().ok()?;
    let token = tokens
        .iter()
        .find(|t| {
            t.kind == TokenKind::Identifier && t.location.offset <= offset && offset < t.end_offset
        })
        .or_else(|| {
            // Cursor often sits at the exclusive end of the word.
            tokens.iter().find(|t| {
                t.kind == TokenKind::Identifier
                    && offset == t.end_offset
                    && t.location.offset < offset
            })
        })?;
    Some((
        token.lexeme.clone(),
        Span::new(token.location.offset, token.end_offset),
    ))
}

pub(crate) fn collect_decls(file: &SourceFile, program: &Program) -> Vec<Decl> {
    let source = file.source.as_str();
    let file_scope = Span::new(0, source.len());
    let mut out = Vec::new();
    for item in &program.items {
        collect_stmt(source, item, file_scope, &mut out);
    }
    out
}

fn collect_stmt(source: &str, stmt: &Stmt, scope: Span, out: &mut Vec<Decl>) {
    match &stmt.kind {
        StmtKind::Function(f) => {
            push_named(
                source,
                stmt.span,
                scope,
                &f.name,
                DeclKind::Function,
                None,
                out,
            );
            collect_function_body(source, f, stmt.span, out);
        }
        StmtKind::Struct(s) => {
            push_named(source, stmt.span, scope, &s.name, DeclKind::Type, None, out);
            for field in &s.fields {
                push_named(
                    source,
                    stmt.span,
                    stmt.span,
                    &field.name,
                    DeclKind::Property,
                    None,
                    out,
                );
            }
        }
        StmtKind::Enum(e) => {
            push_named(source, stmt.span, scope, &e.name, DeclKind::Type, None, out);
            for member in &e.members {
                push_named(
                    source,
                    stmt.span,
                    stmt.span,
                    &member.name,
                    DeclKind::Property,
                    None,
                    out,
                );
            }
        }
        StmtKind::Union(u) => {
            push_named(source, stmt.span, scope, &u.name, DeclKind::Type, None, out);
        }
        StmtKind::Error(err) => {
            push_named(
                source,
                stmt.span,
                scope,
                &err.name,
                DeclKind::Type,
                None,
                out,
            );
            for member in &err.members {
                push_named(
                    source,
                    stmt.span,
                    stmt.span,
                    member,
                    DeclKind::Property,
                    None,
                    out,
                );
            }
        }
        StmtKind::Implement(imp) => {
            push_named(
                source,
                stmt.span,
                scope,
                &imp.target,
                DeclKind::Type,
                None,
                out,
            );
            for f in &imp.functions {
                push_named(
                    source,
                    stmt.span,
                    scope,
                    &f.name,
                    DeclKind::Function,
                    None,
                    out,
                );
                collect_function_body(source, f, stmt.span, out);
            }
        }
        StmtKind::Require { path, alias } => {
            let name = alias
                .as_deref()
                .unwrap_or_else(|| path.rsplit('.').next().unwrap_or(path));
            push_named(
                source,
                stmt.span,
                scope,
                name,
                DeclKind::Module,
                Some(path.clone()),
                out,
            );
        }
        StmtKind::VarDecl { name, .. } => {
            push_named(
                source,
                stmt.span,
                scope,
                name,
                DeclKind::Variable,
                None,
                out,
            );
        }
        StmtKind::If {
            then_branch,
            else_branch,
            ..
        } => {
            for s in then_branch {
                collect_stmt(source, s, scope, out);
            }
            for s in else_branch {
                collect_stmt(source, s, scope, out);
            }
        }
        StmtKind::For { body, .. } => {
            for s in body {
                collect_stmt(source, s, scope, out);
            }
        }
        StmtKind::Match {
            cases, else_branch, ..
        } => {
            for case in cases {
                for s in &case.body {
                    collect_stmt(source, s, scope, out);
                }
            }
            for s in else_branch {
                collect_stmt(source, s, scope, out);
            }
        }
        StmtKind::Defer { body } | StmtKind::Unsafe { body } => {
            for s in body {
                collect_stmt(source, s, scope, out);
            }
        }
        StmtKind::Handle { body, .. } => {
            for s in body {
                collect_stmt(source, s, scope, out);
            }
        }
        _ => {}
    }
}

fn collect_function_body(
    source: &str,
    function: &Function,
    function_span: Span,
    out: &mut Vec<Decl>,
) {
    for stmt in &function.body {
        collect_stmt(source, stmt, function_span, out);
    }
}

fn push_named(
    source: &str,
    item_span: Span,
    scope: Span,
    name: &str,
    kind: DeclKind,
    require_path: Option<String>,
    out: &mut Vec<Decl>,
) {
    if let Some(name_span) = name_span_in(source, item_span, name) {
        out.push(Decl {
            name: name.to_string(),
            name_span,
            scope,
            kind,
            require_path,
        });
    }
}

fn name_span_in(source: &str, span: Span, name: &str) -> Option<Span> {
    if name.is_empty() || span.lo > span.hi || span.hi > source.len() {
        return None;
    }
    let slice = &source[span.lo..span.hi];
    let rel = slice.find(name)?;
    let lo = span.lo + rel;
    let hi = lo + name.len();
    Some(Span::new(lo, hi))
}

pub(crate) fn resolve<'a>(decls: &'a [Decl], name: &str, offset: usize) -> Option<&'a Decl> {
    decls
        .iter()
        .filter(|d| d.name == name && d.scope.lo <= offset && offset < d.scope.hi)
        .min_by_key(|d| d.scope.len())
}
