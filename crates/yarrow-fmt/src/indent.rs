//! Indentation and `end` alignment from `docs/STYLE_GUIDE.md`.
//!
//! Stage 4: one tab per nesting level; `end` / `else` / `do` aligned with the
//! opener; leading spaces rewritten to tabs. Stage 8: continuation lines of
//! wrapped stack phrases paint one level deeper than the phrase start.

use yarrow_core::parser::ast::{
    EnumDecl, ErrorDecl, Expr, Function, Implement, MatchCase, MatchCaseKind, Stmt, StmtKind,
    StructDecl, UnionDecl,
};
use yarrow_core::{SourceFile, Span};

use crate::FormatOptions;
use crate::ir::FormatIr;
use crate::phrase::paint_expr_run_depths;

/// Rewrite leading indentation on each line using AST nesting depth.
///
/// Blank lines stay blank (no indent). Non-empty lines get exactly
/// `depth` leading tabs and no leading spaces. Wrapped phrase continuations
/// use `depth + 1` when Stage 8 layout splits a call or long phrase.
///
/// Idempotent when composed with source hygiene on accepted inputs.
pub fn apply_indent(ir: &FormatIr, options: &FormatOptions) -> String {
    let file = &ir.file;
    let line_count = file.line_count();
    if line_count == 0 {
        return String::from("\n");
    }

    let max_width = options.max_width.max(1);
    let mut levels: Vec<Option<usize>> = vec![None; line_count + 1];
    for item in &ir.program.items {
        paint_stmt(item, 0, max_width, file, &mut levels);
    }
    fill_unpainted_lines(file, &mut levels);

    let mut out = String::with_capacity(file.source.len());
    for (line_no, depth) in levels.iter().enumerate().take(line_count + 1).skip(1) {
        let raw = file.line_text(line_no);
        let content = trim_leading_indent(raw);
        if content.is_empty() {
            out.push('\n');
            continue;
        }
        let depth = depth.unwrap_or(0);
        for _ in 0..depth {
            out.push('\t');
        }
        out.push_str(content);
        out.push('\n');
    }
    out
}

fn trim_leading_indent(line: &str) -> &str {
    line.trim_start_matches([' ', '\t'])
}

fn fill_unpainted_lines(file: &SourceFile, levels: &mut [Option<usize>]) {
    let n = levels.len() - 1;
    // Own-line comments and similar: adopt the next painted line's depth
    // (leading attachment). Fall back to the previous painted depth.
    let mut next = vec![None; levels.len()];
    let mut seen = None;
    for line in (1..=n).rev() {
        if let Some(d) = levels[line] {
            seen = Some(d);
        }
        next[line] = seen;
    }
    let mut prev = None;
    for line in 1..=n {
        if let Some(d) = levels[line] {
            prev = Some(d);
            continue;
        }
        let raw = file.line_text(line);
        if trim_leading_indent(raw).is_empty() {
            continue;
        }
        levels[line] = next[line].or(prev);
    }
}

fn set_level(levels: &mut [Option<usize>], line: usize, depth: usize) {
    if line == 0 || line >= levels.len() {
        return;
    }
    levels[line] = Some(depth);
}

fn end_line(file: &SourceFile, span: Span) -> usize {
    if span.hi == 0 {
        return span.line;
    }
    file.location(span.hi.saturating_sub(1)).line
}

fn lines_in_span(file: &SourceFile, span: Span) -> std::ops::RangeInclusive<usize> {
    let start = span.line.max(1);
    let end = end_line(file, span).max(start);
    start..=end
}

fn first_word(line: &str) -> Option<&str> {
    trim_leading_indent(line).split_whitespace().next()
}

fn line_starts_with_word(line: &str, word: &str) -> bool {
    first_word(line) == Some(word)
}

fn line_has_word(line: &str, word: &str) -> bool {
    trim_leading_indent(line)
        .split_whitespace()
        .any(|w| w == word)
}

fn find_word_line(
    file: &SourceFile,
    from: usize,
    to: usize,
    word: &str,
    levels_len: usize,
) -> Option<usize> {
    if from == 0 || to == 0 || from >= levels_len {
        return None;
    }
    let to = to.min(levels_len - 1);
    if from > to {
        return None;
    }
    (from..=to).find(|&line| line_starts_with_word(file.line_text(line), word))
}

fn expr_start_line(expr: &Expr) -> Option<usize> {
    match expr {
        Expr::Seq(elems) => elems.first().map(|(_, s)| s.line),
        _ => expr.span_hint().map(|s| s.line),
    }
}

fn paint_span_lines(file: &SourceFile, span: Span, depth: usize, levels: &mut [Option<usize>]) {
    for line in lines_in_span(file, span) {
        set_level(levels, line, depth);
    }
}

fn paint_stmt(
    stmt: &Stmt,
    depth: usize,
    max_width: usize,
    file: &SourceFile,
    levels: &mut [Option<usize>],
) {
    match &stmt.kind {
        StmtKind::Function(func) => paint_function(func, stmt.span, depth, max_width, file, levels),
        StmtKind::If {
            then_branch,
            else_branch,
            ..
        } => paint_if(
            stmt.span,
            then_branch,
            else_branch,
            depth,
            max_width,
            file,
            levels,
        ),
        StmtKind::Match {
            cases, else_branch, ..
        } => paint_match(
            stmt.span,
            cases,
            else_branch,
            depth,
            max_width,
            file,
            levels,
        ),
        StmtKind::For { body, .. } => {
            paint_simple_block(stmt.span, body, depth, max_width, file, levels)
        }
        StmtKind::Defer { body } => {
            paint_simple_block(stmt.span, body, depth, max_width, file, levels)
        }
        StmtKind::Unsafe { body } => {
            paint_simple_block(stmt.span, body, depth, max_width, file, levels)
        }
        StmtKind::Handle { body, fallback } => {
            paint_simple_block(stmt.span, body, depth, max_width, file, levels);
            if fallback.is_some() {
                let start = stmt.span.line.max(1);
                let end = end_line(file, stmt.span);
                if start != end {
                    let from = start.saturating_add(1);
                    let to = end.saturating_sub(1).min(levels.len().saturating_sub(1));
                    if from <= to {
                        for line in from..=to {
                            if line_has_word(file.line_text(line), "fallback") {
                                set_level(levels, line, depth + 1);
                                break;
                            }
                        }
                    }
                }
            }
        }
        StmtKind::Struct(decl) => paint_struct(decl, stmt.span, depth, file, levels),
        StmtKind::Implement(impls) => {
            paint_implement(impls, stmt.span, depth, max_width, file, levels)
        }
        StmtKind::Enum(decl) => paint_enum(decl, stmt.span, depth, file, levels),
        StmtKind::Union(decl) => paint_union(decl, stmt.span, depth, file, levels),
        StmtKind::Error(decl) => paint_error(decl, stmt.span, depth, file, levels),
        StmtKind::Require { .. }
        | StmtKind::Expr(_)
        | StmtKind::VarDecl { .. }
        | StmtKind::Set { .. }
        | StmtKind::Move { .. }
        | StmtKind::Return { .. }
        | StmtKind::Fallback { .. } => {
            paint_span_lines(file, stmt.span, depth, levels);
        }
    }
}

/// Paint a statement list, grouping consecutive `Expr` stmts so wrapped
/// call-phrase continuations get `depth + 1`.
fn paint_body(
    stmts: &[Stmt],
    depth: usize,
    max_width: usize,
    file: &SourceFile,
    levels: &mut [Option<usize>],
) {
    let mut i = 0;
    while i < stmts.len() {
        if matches!(stmts[i].kind, StmtKind::Expr(_)) {
            let start = i;
            i += 1;
            while i < stmts.len() && matches!(stmts[i].kind, StmtKind::Expr(_)) {
                i += 1;
            }
            let run = &stmts[start..i];
            for (line, line_depth) in paint_expr_run_depths(run, depth, max_width, file) {
                set_level(levels, line, line_depth);
            }
        } else {
            paint_stmt(&stmts[i], depth, max_width, file, levels);
            i += 1;
        }
    }
}

fn paint_function(
    func: &Function,
    span: Span,
    depth: usize,
    max_width: usize,
    file: &SourceFile,
    levels: &mut [Option<usize>],
) {
    let start = span.line.max(1);
    let end = end_line(file, span);
    set_level(levels, start, depth);
    set_level(levels, end, depth);

    for param in &func.params {
        set_level(levels, param.ty.location.line, depth + 1);
    }

    let body_lo = func.body.first().map(|s| s.span.line).unwrap_or(end).max(1);
    // `do` on its own line sits with the opener; same-line `name function do`
    // is already covered by `start`.
    if let Some(do_line) = find_word_line(file, start, body_lo, "do", levels.len()) {
        set_level(levels, do_line, depth);
    }

    paint_body(&func.body, depth + 1, max_width, file, levels);
}

fn paint_if(
    span: Span,
    then_branch: &[Stmt],
    else_branch: &[Stmt],
    depth: usize,
    max_width: usize,
    file: &SourceFile,
    levels: &mut [Option<usize>],
) {
    let start = span.line.max(1);
    let end = end_line(file, span);
    set_level(levels, start, depth);
    set_level(levels, end, depth);

    let after_then = then_branch
        .last()
        .map(|s| end_line(file, s.span).saturating_add(1))
        .unwrap_or(start.saturating_add(1));
    let else_limit = else_branch
        .first()
        .map(|s| s.span.line.saturating_sub(1))
        .unwrap_or(end.saturating_sub(1));
    if let Some(else_line) = find_word_line(
        file,
        after_then,
        else_limit.max(after_then),
        "else",
        levels.len(),
    ) {
        set_level(levels, else_line, depth);
    } else if let Some(else_line) = find_word_line(
        file,
        after_then,
        end.saturating_sub(1),
        "else",
        levels.len(),
    ) {
        // Empty else body: `else` sits just before `end`.
        set_level(levels, else_line, depth);
    }

    paint_body(then_branch, depth + 1, max_width, file, levels);
    paint_body(else_branch, depth + 1, max_width, file, levels);
}

fn paint_match(
    span: Span,
    cases: &[MatchCase],
    else_branch: &[Stmt],
    depth: usize,
    max_width: usize,
    file: &SourceFile,
    levels: &mut [Option<usize>],
) {
    let start = span.line.max(1);
    let end = end_line(file, span);
    set_level(levels, start, depth);
    set_level(levels, end, depth);

    let mut cursor = start.saturating_add(1);
    for case in cases {
        cursor = paint_match_case(case, depth + 1, max_width, cursor, end, file, levels);
    }

    let after_cases = cases
        .last()
        .and_then(|c| c.body.last().map(|s| end_line(file, s.span)))
        .unwrap_or(cursor.saturating_sub(1));
    let else_search_from = after_cases.saturating_add(1);
    if let Some(else_line) = find_word_line(
        file,
        else_search_from,
        end.saturating_sub(1),
        "else",
        levels.len(),
    ) {
        set_level(levels, else_line, depth + 1);
        paint_body(else_branch, depth + 2, max_width, file, levels);
        let after_else_body = else_branch
            .last()
            .map(|s| end_line(file, s.span).saturating_add(1))
            .unwrap_or(else_line.saturating_add(1));
        if let Some(else_end) = find_word_line(
            file,
            after_else_body,
            end.saturating_sub(1),
            "end",
            levels.len(),
        ) {
            set_level(levels, else_end, depth + 1);
        }
    }
}

fn paint_match_case(
    case: &MatchCase,
    depth: usize,
    max_width: usize,
    search_from: usize,
    match_end: usize,
    file: &SourceFile,
    levels: &mut [Option<usize>],
) -> usize {
    let header = match &case.kind {
        MatchCaseKind::Condition(expr) => expr_start_line(expr).unwrap_or(search_from),
        MatchCaseKind::Type(ty) => ty.location.line.max(1),
    };
    set_level(levels, header, depth);

    paint_body(&case.body, depth + 1, max_width, file, levels);

    let after_body = case
        .body
        .last()
        .map(|s| end_line(file, s.span).saturating_add(1))
        .unwrap_or(header.saturating_add(1));
    let case_end = find_word_line(
        file,
        after_body,
        match_end.saturating_sub(1),
        "end",
        levels.len(),
    )
    .unwrap_or(after_body);
    set_level(levels, case_end, depth);
    case_end.saturating_add(1)
}

fn paint_simple_block(
    span: Span,
    body: &[Stmt],
    depth: usize,
    max_width: usize,
    file: &SourceFile,
    levels: &mut [Option<usize>],
) {
    let start = span.line.max(1);
    let end = end_line(file, span);
    set_level(levels, start, depth);
    set_level(levels, end, depth);
    // One-line forms (`defer … end`, `call handle … fallback end`) keep the
    // whole phrase at `depth`; body spans must not deepen that line.
    if start == end {
        return;
    }
    paint_body(body, depth + 1, max_width, file, levels);
    // Re-assert opener / `end` after body paint (shared-line fallback, etc.).
    set_level(levels, start, depth);
    set_level(levels, end, depth);
}

fn paint_struct(
    decl: &StructDecl,
    span: Span,
    depth: usize,
    file: &SourceFile,
    levels: &mut [Option<usize>],
) {
    let start = span.line.max(1);
    let end = end_line(file, span);
    set_level(levels, start, depth);
    set_level(levels, end, depth);
    for field in &decl.fields {
        set_level(levels, field.ty.location.line, depth + 1);
    }
}

fn paint_implement(
    impls: &Implement,
    span: Span,
    depth: usize,
    max_width: usize,
    file: &SourceFile,
    levels: &mut [Option<usize>],
) {
    let start = span.line.max(1);
    let end = end_line(file, span);
    set_level(levels, start, depth);
    set_level(levels, end, depth);

    let mut search_from = start.saturating_add(1);
    for func in &impls.functions {
        let header = find_named_function_line(file, search_from, end.saturating_sub(1), &func.name)
            .unwrap_or(search_from);
        let method_end = infer_function_end_line(func, header, end.saturating_sub(1), file);
        let lo = file.line_start_offset(header);
        let hi = file.line_start_offset(method_end) + file.line_text(method_end).len();
        let method_span = Span::at(lo, hi, header, 1);
        paint_function(func, method_span, depth + 1, max_width, file, levels);
        search_from = method_end.saturating_add(1);
    }
}

fn find_named_function_line(
    file: &SourceFile,
    from: usize,
    to: usize,
    name: &str,
) -> Option<usize> {
    let to = to.min(file.line_count());
    if from == 0 || from > to {
        return None;
    }
    (from..=to).find(|&line| {
        let text = trim_leading_indent(file.line_text(line));
        text.split_whitespace().next() == Some(name)
            && text.split_whitespace().any(|w| w == "function")
    })
}

fn infer_function_end_line(
    func: &Function,
    header: usize,
    limit: usize,
    file: &SourceFile,
) -> usize {
    let after_body = func
        .body
        .last()
        .map(|s| end_line(file, s.span).saturating_add(1))
        .unwrap_or_else(|| {
            func.params
                .last()
                .map(|p| p.ty.location.line.saturating_add(1))
                .unwrap_or(header.saturating_add(1))
        });
    find_word_line(file, after_body, limit, "end", file.line_count() + 1).unwrap_or(after_body)
}

fn paint_enum(
    decl: &EnumDecl,
    span: Span,
    depth: usize,
    file: &SourceFile,
    levels: &mut [Option<usize>],
) {
    let _ = decl;
    paint_interior_members(span, depth, file, levels);
}

fn paint_union(
    decl: &UnionDecl,
    span: Span,
    depth: usize,
    file: &SourceFile,
    levels: &mut [Option<usize>],
) {
    let start = span.line.max(1);
    let end = end_line(file, span);
    set_level(levels, start, depth);
    set_level(levels, end, depth);
    for ty in &decl.types {
        set_level(levels, ty.location.line, depth + 1);
    }
}

fn paint_error(
    decl: &ErrorDecl,
    span: Span,
    depth: usize,
    file: &SourceFile,
    levels: &mut [Option<usize>],
) {
    let _ = decl;
    paint_interior_members(span, depth, file, levels);
}

fn paint_interior_members(
    span: Span,
    depth: usize,
    file: &SourceFile,
    levels: &mut [Option<usize>],
) {
    let start = span.line.max(1);
    let end = end_line(file, span);
    set_level(levels, start, depth);
    set_level(levels, end, depth);
    for line in (start + 1)..end {
        if !trim_leading_indent(file.line_text(line)).is_empty() {
            set_level(levels, line, depth + 1);
        }
    }
}
