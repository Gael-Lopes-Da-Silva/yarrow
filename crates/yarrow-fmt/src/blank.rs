//! Blank-line rules from `docs/STYLE_GUIDE.md`.
//!
//! Stage 5: single blank between top-level items; at most one consecutive
//! blank anywhere; no blank immediately after block openers (`do` / `if` /
//! `else` / `case` / `for` / `defer` / `unsafe`) or immediately before the
//! matching `end`. Interior logical grouping is left alone (not inserted).

use yarrow_core::parser::ast::{
    EnumDecl, ErrorDecl, Expr, Function, Implement, MatchCase, MatchCaseKind, Stmt, StmtKind,
    StructDecl, UnionDecl,
};
use yarrow_core::{SourceFile, Span};

use crate::ir::FormatIr;

/// Normalize blank lines according to the style guide.
///
/// Expects source already hygiene-cleaned and indent-rewritten so line
/// structure matches [`FormatIr::file`]. Idempotent when composed with
/// source hygiene on accepted inputs.
pub fn apply_blank_lines(ir: &FormatIr) -> String {
    let file = &ir.file;
    let line_count = file.line_count();
    if line_count == 0 {
        return String::from("\n");
    }

    let lines: Vec<&str> = (1..=line_count).map(|n| file.line_text(n)).collect();

    let mut no_blank_after = vec![false; line_count + 1];
    let mut no_blank_before = vec![false; line_count + 1];
    for item in &ir.program.items {
        mark_stmt(item, file, &mut no_blank_after, &mut no_blank_before);
    }

    let groups = toplevel_groups(&ir.program.items, file);
    rebuild(&lines, &groups, &no_blank_after, &no_blank_before)
}

#[derive(Debug, Clone, Copy)]
struct Group {
    /// First line of the item itself (1-based), not including leading comments.
    start: usize,
    end: usize,
}

fn is_blank(line: &str) -> bool {
    line.trim_start_matches([' ', '\t']).is_empty()
}

fn is_comment_line(line: &str) -> bool {
    line.trim_start_matches([' ', '\t']).starts_with('#')
}

fn trim_indent(line: &str) -> &str {
    line.trim_start_matches([' ', '\t'])
}

fn first_word(line: &str) -> Option<&str> {
    trim_indent(line).split_whitespace().next()
}

fn line_starts_with_word(line: &str, word: &str) -> bool {
    first_word(line) == Some(word)
}

fn line_has_word(line: &str, word: &str) -> bool {
    trim_indent(line).split_whitespace().any(|w| w == word)
}

fn end_line(file: &SourceFile, span: Span) -> usize {
    if span.hi == 0 {
        return span.line;
    }
    file.location(span.hi.saturating_sub(1)).line
}

fn find_word_line(file: &SourceFile, from: usize, to: usize, word: &str) -> Option<usize> {
    let last = file.line_count();
    if from == 0 || to == 0 || from > last {
        return None;
    }
    let to = to.min(last);
    if from > to {
        return None;
    }
    (from..=to).find(|&line| line_starts_with_word(file.line_text(line), word))
}

fn find_line_with_word(file: &SourceFile, from: usize, to: usize, word: &str) -> Option<usize> {
    let last = file.line_count();
    if from == 0 || to == 0 || from > last {
        return None;
    }
    let to = to.min(last);
    if from > to {
        return None;
    }
    (from..=to).find(|&line| line_has_word(file.line_text(line), word))
}

fn expr_start_line(expr: &Expr) -> Option<usize> {
    match expr {
        Expr::Seq(elems) => elems.first().map(|(_, s)| s.line),
        _ => expr.span_hint().map(|s| s.line),
    }
}

fn is_toplevel_item(kind: &StmtKind) -> bool {
    matches!(
        kind,
        StmtKind::Require { .. }
            | StmtKind::Function(_)
            | StmtKind::Struct(_)
            | StmtKind::Implement(_)
            | StmtKind::Enum(_)
            | StmtKind::Union(_)
            | StmtKind::Error(_)
    )
}

fn is_require(kind: &StmtKind) -> bool {
    matches!(kind, StmtKind::Require { .. })
}

fn toplevel_groups(items: &[Stmt], file: &SourceFile) -> Vec<Group> {
    let mut groups = Vec::new();
    let mut i = 0;
    while i < items.len() {
        if !is_toplevel_item(&items[i].kind) {
            i += 1;
            continue;
        }
        let start = items[i].span.line.max(1);
        let mut end = end_line(file, items[i].span);
        if is_require(&items[i].kind) {
            i += 1;
            while i < items.len() && is_require(&items[i].kind) {
                end = end_line(file, items[i].span);
                i += 1;
            }
        } else {
            i += 1;
        }
        groups.push(Group { start, end });
    }
    groups
}

/// Leading own-line comments immediately above `start` (no blank between).
fn leading_comment_start(lines: &[&str], start: usize) -> usize {
    let mut s = start;
    while s > 1 {
        let prev = lines[s - 2];
        if is_comment_line(prev) {
            s -= 1;
        } else {
            break;
        }
    }
    s
}

fn push_blank_if_needed(out: &mut Vec<&str>) {
    if !out.is_empty() && !out.last().is_some_and(|l| is_blank(l)) {
        out.push("");
    }
}

fn trim_trailing_blanks(out: &mut Vec<&str>) {
    while out.last().is_some_and(|l| is_blank(l)) {
        out.pop();
    }
}

/// Emit lines `from..=to` (1-based), collapsing blanks and honoring bans.
fn emit_region<'a>(
    out: &mut Vec<&'a str>,
    lines: &[&'a str],
    from: usize,
    to: usize,
    no_blank_after: &[bool],
    no_blank_before: &[bool],
) {
    if from == 0 || from > to {
        return;
    }
    let mut i = from;
    while i <= to {
        let line = lines[i - 1];
        if !is_blank(line) {
            out.push(line);
            i += 1;
            continue;
        }

        let mut j = i;
        while j <= to && is_blank(lines[j - 1]) {
            j += 1;
        }

        // Ban uses the previous content line number (`i - 1`) and the next
        // content line number (`j`), which may sit just past `to`.
        let prev_forbidden = i > 1 && no_blank_after[i - 1];
        let next_forbidden = j <= lines.len() && no_blank_before[j];
        let last_was_blank = out.last().is_some_and(|l| is_blank(l));

        if !prev_forbidden && !next_forbidden && !last_was_blank && !out.is_empty() {
            out.push("");
        }
        i = j;
    }
}

fn rebuild(
    lines: &[&str],
    groups: &[Group],
    no_blank_after: &[bool],
    no_blank_before: &[bool],
) -> String {
    let n = lines.len();
    let mut out: Vec<&str> = Vec::with_capacity(n + groups.len());

    if groups.is_empty() {
        emit_region(&mut out, lines, 1, n, no_blank_after, no_blank_before);
    } else {
        let first_lead = leading_comment_start(lines, groups[0].start);
        if first_lead > 1 {
            emit_region(
                &mut out,
                lines,
                1,
                first_lead - 1,
                no_blank_after,
                no_blank_before,
            );
        }

        // Track how far we have emitted so overlapping spans (parser bugs)
        // cannot reprint earlier lines.
        let mut emitted_through = first_lead.saturating_sub(1);

        for (gi, group) in groups.iter().enumerate() {
            let lead = leading_comment_start(lines, group.start).max(emitted_through + 1);
            let end = group.end.max(lead.saturating_sub(1));
            if lead > end {
                continue;
            }

            if gi == 0 {
                if first_lead > 1 {
                    push_blank_if_needed(&mut out);
                }
            } else {
                trim_trailing_blanks(&mut out);
                push_blank_if_needed(&mut out);
            }

            emit_region(&mut out, lines, lead, end, no_blank_after, no_blank_before);
            emitted_through = emitted_through.max(end);

            let next_lead = groups
                .get(gi + 1)
                .map(|g| leading_comment_start(lines, g.start))
                .unwrap_or(n + 1);
            let gap_lo = end + 1;
            let gap_hi = next_lead.saturating_sub(1);
            if gap_lo <= gap_hi && gap_hi <= n {
                for line_no in gap_lo..=gap_hi {
                    if line_no <= emitted_through {
                        continue;
                    }
                    if !is_blank(lines[line_no - 1]) {
                        emit_region(
                            &mut out,
                            lines,
                            line_no,
                            line_no,
                            no_blank_after,
                            no_blank_before,
                        );
                        emitted_through = emitted_through.max(line_no);
                    }
                }
            }
        }

        let last_end = emitted_through;
        if last_end < n {
            let mut first_content = None;
            for line_no in (last_end + 1)..=n {
                if !is_blank(lines[line_no - 1]) {
                    first_content = Some(line_no);
                    break;
                }
            }
            if let Some(start) = first_content {
                trim_trailing_blanks(&mut out);
                push_blank_if_needed(&mut out);
                emit_region(&mut out, lines, start, n, no_blank_after, no_blank_before);
            }
        }
    }

    trim_trailing_blanks(&mut out);

    let mut buf = String::new();
    for (i, line) in out.iter().enumerate() {
        if i > 0 {
            buf.push('\n');
        }
        buf.push_str(line);
    }
    buf.push('\n');
    buf
}

fn mark_line(flags: &mut [bool], line: usize) {
    if line > 0 && line < flags.len() {
        flags[line] = true;
    }
}

fn mark_stmt(stmt: &Stmt, file: &SourceFile, after: &mut [bool], before: &mut [bool]) {
    match &stmt.kind {
        StmtKind::Function(func) => mark_function(func, stmt.span, file, after, before),
        StmtKind::If {
            then_branch,
            else_branch,
            ..
        } => mark_if(stmt.span, then_branch, else_branch, file, after, before),
        StmtKind::Match {
            cases, else_branch, ..
        } => mark_match(stmt.span, cases, else_branch, file, after, before),
        StmtKind::For { body, .. } => mark_simple_block(stmt.span, body, file, after, before),
        StmtKind::Defer { body } => mark_simple_block(stmt.span, body, file, after, before),
        StmtKind::Unsafe { body } => mark_simple_block(stmt.span, body, file, after, before),
        StmtKind::Handle { body, .. } => {
            mark_line(before, end_line(file, stmt.span));
            for s in body {
                mark_stmt(s, file, after, before);
            }
        }
        StmtKind::Struct(decl) => mark_struct(decl, stmt.span, file, after, before),
        StmtKind::Implement(impls) => mark_implement(impls, stmt.span, file, after, before),
        StmtKind::Enum(decl) => mark_enum(decl, stmt.span, file, after, before),
        StmtKind::Union(decl) => mark_union(decl, stmt.span, file, after, before),
        StmtKind::Error(decl) => mark_error(decl, stmt.span, file, after, before),
        StmtKind::Require { .. }
        | StmtKind::Expr(_)
        | StmtKind::VarDecl { .. }
        | StmtKind::Set { .. }
        | StmtKind::Move { .. }
        | StmtKind::Return { .. }
        | StmtKind::Fallback { .. } => {}
    }
}

fn mark_function(
    func: &Function,
    span: Span,
    file: &SourceFile,
    after: &mut [bool],
    before: &mut [bool],
) {
    let start = span.line.max(1);
    let end = end_line(file, span);
    mark_line(before, end);

    let body_lo = func.body.first().map(|s| s.span.line).unwrap_or(end).max(1);
    // `name function do` keeps `do` on the header line; own-line `do` is later.
    let do_search_hi = body_lo.max(start);
    if let Some(do_line) = find_line_with_word(file, start, do_search_hi, "do")
        .or_else(|| find_word_line(file, start, body_lo.max(start), "do"))
    {
        mark_line(after, do_line);
    }

    for stmt in &func.body {
        mark_stmt(stmt, file, after, before);
    }
}

fn mark_if(
    span: Span,
    then_branch: &[Stmt],
    else_branch: &[Stmt],
    file: &SourceFile,
    after: &mut [bool],
    before: &mut [bool],
) {
    let start = span.line.max(1);
    let end = end_line(file, span);
    mark_line(after, start);
    mark_line(before, end);

    let after_then = then_branch
        .last()
        .map(|s| end_line(file, s.span).saturating_add(1))
        .unwrap_or(start.saturating_add(1));
    let else_limit = else_branch
        .first()
        .map(|s| s.span.line.saturating_sub(1))
        .unwrap_or(end.saturating_sub(1));
    if let Some(else_line) = find_word_line(file, after_then, else_limit.max(after_then), "else")
        .or_else(|| find_word_line(file, after_then, end.saturating_sub(1), "else"))
    {
        mark_line(after, else_line);
    }

    for stmt in then_branch {
        mark_stmt(stmt, file, after, before);
    }
    for stmt in else_branch {
        mark_stmt(stmt, file, after, before);
    }
}

fn mark_match(
    span: Span,
    cases: &[MatchCase],
    else_branch: &[Stmt],
    file: &SourceFile,
    after: &mut [bool],
    before: &mut [bool],
) {
    let start = span.line.max(1);
    let end = end_line(file, span);
    mark_line(after, start);
    mark_line(before, end);

    let mut cursor = start.saturating_add(1);
    for case in cases {
        cursor = mark_match_case(case, cursor, end, file, after, before);
    }

    let after_cases = cases
        .last()
        .and_then(|c| c.body.last().map(|s| end_line(file, s.span)))
        .unwrap_or(cursor.saturating_sub(1));
    if let Some(else_line) = find_word_line(
        file,
        after_cases.saturating_add(1),
        end.saturating_sub(1),
        "else",
    ) {
        mark_line(after, else_line);
        for stmt in else_branch {
            mark_stmt(stmt, file, after, before);
        }
        let after_else_body = else_branch
            .last()
            .map(|s| end_line(file, s.span).saturating_add(1))
            .unwrap_or(else_line.saturating_add(1));
        if let Some(else_end) = find_word_line(file, after_else_body, end.saturating_sub(1), "end")
        {
            mark_line(before, else_end);
        }
    }
}

fn mark_match_case(
    case: &MatchCase,
    search_from: usize,
    match_end: usize,
    file: &SourceFile,
    after: &mut [bool],
    before: &mut [bool],
) -> usize {
    let header = match &case.kind {
        MatchCaseKind::Condition(expr) => expr_start_line(expr).unwrap_or(search_from),
        MatchCaseKind::Type(ty) => ty.location.line.max(1),
    };
    mark_line(after, header);

    for stmt in &case.body {
        mark_stmt(stmt, file, after, before);
    }

    let after_body = case
        .body
        .last()
        .map(|s| end_line(file, s.span).saturating_add(1))
        .unwrap_or(header.saturating_add(1));
    let case_end =
        find_word_line(file, after_body, match_end.saturating_sub(1), "end").unwrap_or(after_body);
    mark_line(before, case_end);
    case_end.saturating_add(1)
}

fn mark_simple_block(
    span: Span,
    body: &[Stmt],
    file: &SourceFile,
    after: &mut [bool],
    before: &mut [bool],
) {
    let start = span.line.max(1);
    let end = end_line(file, span);
    mark_line(after, start);
    mark_line(before, end);
    for stmt in body {
        mark_stmt(stmt, file, after, before);
    }
}

fn mark_struct(
    decl: &StructDecl,
    span: Span,
    file: &SourceFile,
    after: &mut [bool],
    before: &mut [bool],
) {
    let _ = (decl, after);
    mark_line(before, end_line(file, span));
}

fn mark_implement(
    impls: &Implement,
    span: Span,
    file: &SourceFile,
    after: &mut [bool],
    before: &mut [bool],
) {
    let start = span.line.max(1);
    let end = end_line(file, span);
    mark_line(before, end);

    let mut search_from = start.saturating_add(1);
    for func in &impls.functions {
        let header = find_named_function_line(file, search_from, end.saturating_sub(1), &func.name)
            .unwrap_or(search_from);
        let method_end = infer_function_end_line(func, header, end.saturating_sub(1), file);
        let lo = file.line_start_offset(header);
        let hi = file.line_start_offset(method_end) + file.line_text(method_end).len();
        let method_span = Span::at(lo, hi, header, 1);
        mark_function(func, method_span, file, after, before);
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
        let text = trim_indent(file.line_text(line));
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
    find_word_line(file, after_body, limit, "end").unwrap_or(after_body)
}

fn mark_enum(
    decl: &EnumDecl,
    span: Span,
    file: &SourceFile,
    after: &mut [bool],
    before: &mut [bool],
) {
    let _ = (decl, after);
    mark_line(before, end_line(file, span));
}

fn mark_union(
    decl: &UnionDecl,
    span: Span,
    file: &SourceFile,
    after: &mut [bool],
    before: &mut [bool],
) {
    let _ = (decl, after);
    mark_line(before, end_line(file, span));
}

fn mark_error(
    decl: &ErrorDecl,
    span: Span,
    file: &SourceFile,
    after: &mut [bool],
    before: &mut [bool],
) {
    let _ = (decl, after);
    mark_line(before, end_line(file, span));
}
