//! Build `textDocument/documentSymbol` outlines from a parsed [`Program`].

use tower_lsp_server::ls_types::{DocumentSymbol, Range, SymbolKind};
use yarrow_core::parser::ast::{Function, Stmt, StmtKind};
use yarrow_core::{Program, SourceFile, Span};

use crate::position::{PositionEncoding, PositionMap};

/// Parse `text` and return hierarchical document symbols, or `None` on parse failure.
pub fn document_symbols(
    path: &str,
    text: &str,
    encoding: PositionEncoding,
) -> Option<Vec<DocumentSymbol>> {
    let opts = yarrow_core::CompileOptions::new(path.to_string());
    let session = yarrow_core::Session::new(opts);
    let (file, program) = session.parse_source(text.to_string()).ok()?;
    Some(program_symbols(&file, &program, encoding))
}

fn program_symbols(
    file: &SourceFile,
    program: &Program,
    encoding: PositionEncoding,
) -> Vec<DocumentSymbol> {
    let map = PositionMap::from_file(file, encoding);
    let source = file.source.as_str();
    program
        .items
        .iter()
        .filter_map(|item| stmt_symbol(source, &map, item))
        .collect()
}

fn stmt_symbol(source: &str, map: &PositionMap<'_>, stmt: &Stmt) -> Option<DocumentSymbol> {
    match &stmt.kind {
        StmtKind::Function(f) => Some(function_symbol(source, map, f, stmt.span)),
        StmtKind::Struct(s) => {
            let children = s
                .fields
                .iter()
                .filter_map(|field| {
                    name_only_symbol(source, map, stmt.span, &field.name, SymbolKind::FIELD)
                })
                .collect::<Vec<_>>();
            Some(named_symbol(
                source,
                map,
                stmt.span,
                s.name.clone(),
                SymbolKind::STRUCT,
                children,
            ))
        }
        StmtKind::Enum(e) => {
            let children = e
                .members
                .iter()
                .filter_map(|member| {
                    name_only_symbol(
                        source,
                        map,
                        stmt.span,
                        &member.name,
                        SymbolKind::ENUM_MEMBER,
                    )
                })
                .collect::<Vec<_>>();
            Some(named_symbol(
                source,
                map,
                stmt.span,
                e.name.clone(),
                SymbolKind::ENUM,
                children,
            ))
        }
        StmtKind::Union(u) => Some(named_symbol(
            source,
            map,
            stmt.span,
            u.name.clone(),
            SymbolKind::CLASS,
            Vec::new(),
        )),
        StmtKind::Error(err) => {
            let children = err
                .members
                .iter()
                .filter_map(|member| {
                    name_only_symbol(source, map, stmt.span, member, SymbolKind::ENUM_MEMBER)
                })
                .collect::<Vec<_>>();
            Some(named_symbol(
                source,
                map,
                stmt.span,
                err.name.clone(),
                SymbolKind::CLASS,
                children,
            ))
        }
        StmtKind::Implement(imp) => {
            let children = imp
                .functions
                .iter()
                .filter_map(|f| {
                    name_only_symbol(source, map, stmt.span, &f.name, SymbolKind::METHOD)
                })
                .collect::<Vec<_>>();
            Some(named_symbol(
                source,
                map,
                stmt.span,
                imp.target.clone(),
                SymbolKind::NAMESPACE,
                children,
            ))
        }
        StmtKind::Require { path, alias } => {
            let name = alias
                .clone()
                .unwrap_or_else(|| path.rsplit('.').next().unwrap_or(path).to_string());
            Some(named_symbol(
                source,
                map,
                stmt.span,
                name,
                SymbolKind::MODULE,
                Vec::new(),
            ))
        }
        _ => None,
    }
}

fn function_symbol(
    source: &str,
    map: &PositionMap<'_>,
    function: &Function,
    span: Span,
) -> DocumentSymbol {
    let children = function
        .body
        .iter()
        .filter_map(|inner| match &inner.kind {
            StmtKind::Function(nested) => Some(function_symbol(source, map, nested, inner.span)),
            _ => None,
        })
        .collect();
    named_symbol(
        source,
        map,
        span,
        function.name.clone(),
        SymbolKind::FUNCTION,
        children,
    )
}

fn named_symbol(
    source: &str,
    map: &PositionMap<'_>,
    span: Span,
    name: String,
    kind: SymbolKind,
    children: Vec<DocumentSymbol>,
) -> DocumentSymbol {
    let range = map.range(span);
    let selection_range = name_selection(source, map, span, &name).unwrap_or(range);
    make_symbol(name, kind, range, selection_range, children)
}

fn name_only_symbol(
    source: &str,
    map: &PositionMap<'_>,
    parent: Span,
    name: &str,
    kind: SymbolKind,
) -> Option<DocumentSymbol> {
    let selection = name_selection(source, map, parent, name)?;
    Some(make_symbol(
        name.to_string(),
        kind,
        selection,
        selection,
        Vec::new(),
    ))
}

fn name_selection(source: &str, map: &PositionMap<'_>, span: Span, name: &str) -> Option<Range> {
    if name.is_empty() || span.lo > span.hi || span.hi > source.len() {
        return None;
    }
    let slice = &source[span.lo..span.hi];
    let rel = slice.find(name)?;
    let lo = span.lo + rel;
    let hi = lo + name.len();
    Some(map.range(Span::new(lo, hi)))
}

#[allow(deprecated)]
fn make_symbol(
    name: String,
    kind: SymbolKind,
    range: Range,
    selection_range: Range,
    children: Vec<DocumentSymbol>,
) -> DocumentSymbol {
    DocumentSymbol {
        name,
        detail: None,
        kind,
        tags: None,
        deprecated: None,
        range,
        selection_range,
        children: if children.is_empty() {
            None
        } else {
            Some(children)
        },
    }
}
