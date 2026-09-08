//! Same-file `textDocument/definition` via AST declarations + token fallback.

use tower_lsp_server::ls_types::{GotoDefinitionResponse, Location, Position, Uri};
use yarrow_core::parser::ast::{Function, Stmt, StmtKind};
use yarrow_core::{Program, SourceFile, Span, TokenKind, Tokenizer};

use crate::position::{PositionEncoding, PositionMap};

/// Resolve definition at `position` in `text`, or `None` if unresolved / parse failure.
pub fn goto_definition(
    uri: &Uri,
    path: &str,
    text: &str,
    encoding: PositionEncoding,
    position: Position,
) -> Option<GotoDefinitionResponse> {
    let opts = yarrow_core::CompileOptions::new(path.to_string());
    let session = yarrow_core::Session::new(opts);
    let (file, program) = session.parse_source(text.to_string()).ok()?;
    let map = PositionMap::from_file(&file, encoding);
    let offset = map.offset(position)?;
    let name = identifier_at(&file.source, offset)?;
    let decls = collect_decls(&file, &program);
    let decl = resolve(&decls, &name, offset)?;
    let range = map.range(decl.name_span);
    Some(GotoDefinitionResponse::Scalar(Location::new(
        uri.clone(),
        range,
    )))
}

#[derive(Debug, Clone)]
struct Decl {
    name: String,
    name_span: Span,
    /// Region where this declaration is visible.
    scope: Span,
}

fn identifier_at(source: &str, offset: usize) -> Option<String> {
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
    Some(token.lexeme.clone())
}

fn collect_decls(file: &SourceFile, program: &Program) -> Vec<Decl> {
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
            push_named(source, stmt.span, scope, &f.name, out);
            collect_function_body(source, f, stmt.span, out);
        }
        StmtKind::Struct(s) => {
            push_named(source, stmt.span, scope, &s.name, out);
            for field in &s.fields {
                push_named(source, stmt.span, stmt.span, &field.name, out);
            }
        }
        StmtKind::Enum(e) => {
            push_named(source, stmt.span, scope, &e.name, out);
            for member in &e.members {
                push_named(source, stmt.span, stmt.span, &member.name, out);
            }
        }
        StmtKind::Union(u) => {
            push_named(source, stmt.span, scope, &u.name, out);
        }
        StmtKind::Error(err) => {
            push_named(source, stmt.span, scope, &err.name, out);
            for member in &err.members {
                push_named(source, stmt.span, stmt.span, member, out);
            }
        }
        StmtKind::Implement(imp) => {
            push_named(source, stmt.span, scope, &imp.target, out);
            for f in &imp.functions {
                push_named(source, stmt.span, scope, &f.name, out);
                collect_function_body(source, f, stmt.span, out);
            }
        }
        StmtKind::Require { path, alias } => {
            let name = alias
                .as_deref()
                .unwrap_or_else(|| path.rsplit('.').next().unwrap_or(path));
            push_named(source, stmt.span, scope, name, out);
        }
        StmtKind::VarDecl { name, .. } => {
            push_named(source, stmt.span, scope, name, out);
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

fn push_named(source: &str, item_span: Span, scope: Span, name: &str, out: &mut Vec<Decl>) {
    if let Some(name_span) = name_span_in(source, item_span, name) {
        out.push(Decl {
            name: name.to_string(),
            name_span,
            scope,
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

fn resolve<'a>(decls: &'a [Decl], name: &str, offset: usize) -> Option<&'a Decl> {
    decls
        .iter()
        .filter(|d| d.name == name && d.scope.lo <= offset && offset < d.scope.hi)
        .min_by_key(|d| d.scope.len())
}
