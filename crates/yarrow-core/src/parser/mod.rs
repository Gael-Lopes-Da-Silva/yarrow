//! Recursive-descent parser for the Yarrow language.
//!
//! Yarrow is a stack-based language in which whitespace/newlines are not part
//! of the token stream. To reconstruct structure, the parser maintains an
//! *operand stack* while scanning a body of tokens. Each value or computed
//! expression is pushed onto the operand stack; binary operators pop two
//! operands and push back a combined node. Declarations and control-flow
//! keywords flush (drain) the operand stack and use its contents as their
//! payload.

pub mod ast;
pub mod literals;

pub use ast::*;

use crate::diagnostics::{DiagnosticBatch, Span};
use crate::tokenizer::token::Location;
use crate::tokenizer::token::Token;
use crate::tokenizer::token_kind::TokenKind;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseError {
    pub message: String,
    pub location: Location,
    pub code: String,
}

impl ParseError {
    pub fn new(message: impl Into<String>, location: Location, code: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            location,
            code: code.into(),
        }
    }

    pub fn into_diagnostic(self) -> crate::diagnostics::Diagnostic {
        crate::diagnostics::Diagnostic::error(self.code, self.message)
            .with_primary(Span::from_location(self.location), "")
    }
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "[{}] {} at line {}, column {}",
            self.code, self.message, self.location.line, self.location.column
        )
    }
}

impl std::error::Error for ParseError {}

impl From<crate::tokenizer::TokenizeError> for ParseError {
    fn from(e: crate::tokenizer::TokenizeError) -> Self {
        ParseError::new(e.message, e.location, e.code)
    }
}

type ParseResult<T> = Result<T, ParseError>;

pub struct Parser {
    tokens: Vec<Token>,
    current: usize,
    /// Syntax errors collected during recovery (Stage 10).
    errors: DiagnosticBatch,
}

impl Parser {
    pub fn new(tokens: Vec<Token>) -> Self {
        Self {
            tokens,
            current: 0,
            errors: DiagnosticBatch::new(),
        }
    }

    /// Parse a program. On syntax failure returns every recovered diagnostic
    /// (capped), not only the first.
    pub fn parse(&mut self) -> Result<Program, DiagnosticBatch> {
        let items = match self.body(&[TokenKind::Eof]) {
            Ok(items) => items,
            Err(e) => {
                self.record_error(e);
                Vec::new()
            }
        };
        if !self.errors.is_empty() {
            return Err(self.errors.take());
        }
        Ok(Program { items })
    }

    fn record_error(&mut self, err: ParseError) -> bool {
        self.errors.push(err.into_diagnostic())
    }

    // ------------------------------------------------------------------
    // Statement parsing
    // ------------------------------------------------------------------

    fn body(&mut self, stops: &[TokenKind]) -> ParseResult<Vec<Stmt>> {
        let mut stmts = Vec::new();
        let mut ops: Vec<Expr> = Vec::new();
        let mut op_spans: Vec<Span> = Vec::new();

        loop {
            if self.errors.is_at_limit() {
                break;
            }

            let kind = self.peek_kind();

            if stops.contains(&kind) {
                break;
            }

            match kind {
                TokenKind::End | TokenKind::Else | TokenKind::Case | TokenKind::Eof => break,
                _ => {
                    let start_pos = self.current;
                    if let Err(e) = self.body_step(kind, &mut stmts, &mut ops, &mut op_spans) {
                        if !self.record_error(e) {
                            break;
                        }
                        // Drop partial operand stack so the next statement does
                        // not inherit a corrupted postfix sequence.
                        ops.clear();
                        op_spans.clear();
                        if self.current == start_pos && self.peek_kind() != TokenKind::Eof {
                            self.advance();
                        }
                        self.synchronize(stops);
                    }
                }
            }
        }

        if let Some((expr, span)) = drain_ops(&mut ops, &mut op_spans) {
            stmts.push(Stmt::new(StmtKind::Expr(expr), span));
        }

        Ok(stmts)
    }

    /// Parse one statement or expression word inside a body.
    fn body_step(
        &mut self,
        kind: TokenKind,
        stmts: &mut Vec<Stmt>,
        ops: &mut Vec<Expr>,
        op_spans: &mut Vec<Span>,
    ) -> ParseResult<()> {
        match kind {
            TokenKind::Mutable | TokenKind::Const | TokenKind::Static => {
                let decl = self.parse_var_decl(ops, op_spans)?;
                stmts.push(decl);
            }

            TokenKind::Function => {
                let (name, stack_span) = self.pop_name(ops, op_spans)?;
                let start = self.peek_span();
                let func = self.parse_function(name, None, false)?;
                let end = self.prev_span();
                let span = stack_span.merge(start).merge(end);
                stmts.push(Stmt::new(StmtKind::Function(func), span));
            }

            TokenKind::Load | TokenKind::Store => {
                // `load`/`store` are keywords (the typed pointer words),
                // but `std.mem` also exposes functions with these names.
                // When the next token is a function declaration head, this
                // is a function name; otherwise it is the word.
                if self.peek_is_function_head(1) {
                    let span = self.peek_span();
                    ops.push(Expr::variable(self.peek_lexeme()));
                    note_op_span(op_spans, span);
                    self.advance();
                } else {
                    self.process_expr_word(ops, op_spans)?;
                }
            }

            TokenKind::Public | TokenKind::Private => {
                let vis = self.parse_visibility().unwrap();
                let stmt = self.parse_visible_decl(ops, op_spans, vis)?;
                stmts.push(stmt);
            }

            TokenKind::Unsafe => {
                if self.peek_next_kind() == TokenKind::Function {
                    // `name unsafe function`
                    let (name, stack_span) = self.pop_name(ops, op_spans)?;
                    let start = self.peek_span();
                    self.advance();
                    let func = self.parse_function(name, None, true)?;
                    let end = self.prev_span();
                    let span = stack_span.merge(start).merge(end);
                    stmts.push(Stmt::new(StmtKind::Function(func), span));
                } else {
                    // `unsafe ... end`: an unsafe block.
                    let start = self.peek_span();
                    self.advance();
                    let body = self.body(&[TokenKind::End])?;
                    self.expect(TokenKind::End, "expected 'end' after unsafe block")?;
                    let end = self.prev_span();
                    stmts.push(Stmt::new(StmtKind::Unsafe { body }, start.merge(end)));
                }
            }

            TokenKind::Struct => {
                let (name, stack_span) = self.pop_name(ops, op_spans)?;
                let start = self.peek_span();
                let decl = self.parse_struct(name, None)?;
                let end = self.prev_span();
                let span = stack_span.merge(start).merge(end);
                stmts.push(Stmt::new(StmtKind::Struct(decl), span));
            }

            TokenKind::Enum => {
                let (name, underlying, stack_span) = self.pop_enum_head(ops, op_spans)?;
                let start = self.peek_span();
                let decl = self.parse_enum(name, underlying)?;
                let end = self.prev_span();
                let span = stack_span.merge(start).merge(end);
                stmts.push(Stmt::new(StmtKind::Enum(decl), span));
            }

            TokenKind::Union => {
                let (name, stack_span) = self.pop_name(ops, op_spans)?;
                let start = self.peek_span();
                let decl = self.parse_union(name)?;
                let end = self.prev_span();
                let span = stack_span.merge(start).merge(end);
                stmts.push(Stmt::new(StmtKind::Union(decl), span));
            }

            TokenKind::Error => {
                // Soft keyword: `error.Name` is a path; `… error require`
                // is a module alias; otherwise `Name error … end`.
                match self.peek_next_kind() {
                    TokenKind::Dot => self.process_expr_word(ops, op_spans)?,
                    TokenKind::Require => {
                        let span = self.peek_span();
                        ops.push(Expr::variable("error"));
                        note_op_span(op_spans, span);
                        self.advance();
                    }
                    _ => {
                        let start = self.peek_span();
                        let (decl, stack_span) = self.parse_error_decl(ops, op_spans)?;
                        let end = self.prev_span();
                        let span = stack_span.merge(start).merge(end);
                        stmts.push(Stmt::new(StmtKind::Error(decl), span));
                    }
                }
            }

            TokenKind::Implement => {
                let (name, stack_span) = self.pop_name(ops, op_spans)?;
                let start = self.peek_span();
                let impls = self.parse_implement(name)?;
                let end = self.prev_span();
                let span = stack_span.merge(start).merge(end);
                stmts.push(Stmt::new(StmtKind::Implement(impls), span));
            }

            TokenKind::Require => {
                let req = self.parse_require(ops, op_spans)?;
                stmts.push(req);
            }

            TokenKind::Set => {
                let set = self.parse_set(ops, op_spans)?;
                stmts.push(set);
            }

            TokenKind::If => {
                let (condition, cond_span) = drain_ops(ops, op_spans)
                    .unwrap_or_else(|| (Expr::variable(""), Span::default()));
                let start = self.peek_span();
                self.advance();
                let then_branch = self.body(&[TokenKind::Else, TokenKind::End])?;
                let else_branch = if self.match_kind(TokenKind::Else) {
                    self.body(&[TokenKind::End])?
                } else {
                    Vec::new()
                };
                self.expect(TokenKind::End, "expected 'end' after if/else block")?;
                let end = self.prev_span();
                let span = cond_span.merge(start).merge(end);
                stmts.push(Stmt::new(
                    StmtKind::If {
                        condition,
                        then_branch,
                        else_branch,
                    },
                    span,
                ));
            }

            TokenKind::For => {
                let stmt = self.parse_for(ops, op_spans)?;
                stmts.push(stmt);
            }

            TokenKind::Match => {
                let stmt = self.parse_match(ops, op_spans)?;
                stmts.push(stmt);
            }

            TokenKind::Defer => {
                let start = self.peek_span();
                self.advance();
                let body = self.body(&[TokenKind::End])?;
                self.expect(TokenKind::End, "expected 'end' after defer block")?;
                let end = self.prev_span();
                stmts.push(Stmt::new(StmtKind::Defer { body }, start.merge(end)));
            }

            TokenKind::Handle => {
                if let Some((expr, span)) = drain_ops(ops, op_spans) {
                    stmts.push(Stmt::new(StmtKind::Expr(expr), span));
                }
                let start = self.peek_span();
                self.advance();
                let body = self.body(&[TokenKind::End])?;
                self.expect(TokenKind::End, "expected 'end' after handle block")?;
                let end = self.prev_span();
                let (body, fallback) = extract_fallback(body);
                stmts.push(Stmt::new(
                    StmtKind::Handle { body, fallback },
                    start.merge(end),
                ));
            }

            TokenKind::Move => {
                let start = self.peek_span();
                self.advance();
                let location = self.prev_span().start_location();
                let target_expr = ops.pop().ok_or_else(|| {
                    ParseError::new("'move' requires a target variable", location, "E220")
                })?;
                let target_span = op_spans.pop().unwrap_or_default();
                let target = match target_expr {
                    Expr::Variable { name } => name,
                    _ => {
                        return Err(ParseError::new(
                            "'move' requires a target variable",
                            location,
                            "E221",
                        ));
                    }
                };
                let (source, source_span) = drain_ops(ops, op_spans)
                    .unwrap_or_else(|| (Expr::variable(""), Span::default()));
                stmts.push(Stmt::new(
                    StmtKind::Move { target, source },
                    target_span.merge(source_span).merge(start),
                ));
            }

            TokenKind::Fallback => {
                let start = self.peek_span();
                self.advance();
                let (value, stack_span) = match drain_ops(ops, op_spans) {
                    Some((expr, span)) => (Some(expr), span),
                    None => (None, Span::default()),
                };
                stmts.push(Stmt::new(
                    StmtKind::Fallback { value },
                    stack_span.merge(start),
                ));
            }

            TokenKind::Return => {
                let drained = drain_ops(ops, op_spans);
                let start = self.peek_span();
                self.advance();
                if let Some((expr, expr_span)) = drained {
                    stmts.push(Stmt::new(StmtKind::Expr(expr), expr_span));
                }
                stmts.push(Stmt::new(StmtKind::Return { value: None }, start));
            }

            _ => self.process_expr_word(ops, op_spans)?,
        }
        Ok(())
    }

    /// Skip tokens until a likely statement / block boundary after a syntax error.
    fn synchronize(&mut self, stops: &[TokenKind]) {
        while self.peek_kind() != TokenKind::Eof {
            let kind = self.peek_kind();
            if stops.contains(&kind) {
                break;
            }
            if matches!(
                kind,
                TokenKind::End
                    | TokenKind::Else
                    | TokenKind::Case
                    | TokenKind::Function
                    | TokenKind::Struct
                    | TokenKind::Enum
                    | TokenKind::Union
                    | TokenKind::Implement
                    | TokenKind::Require
                    | TokenKind::If
                    | TokenKind::For
                    | TokenKind::Match
                    | TokenKind::Defer
                    | TokenKind::Handle
                    | TokenKind::Return
                    | TokenKind::Set
                    | TokenKind::Mutable
                    | TokenKind::Const
                    | TokenKind::Static
                    | TokenKind::Public
                    | TokenKind::Private
                    | TokenKind::Unsafe
                    | TokenKind::Error
            ) {
                break;
            }
            self.advance();
        }
    }

    fn parse_var_decl(
        &mut self,
        ops: &mut Vec<Expr>,
        op_spans: &mut Vec<Span>,
    ) -> ParseResult<Stmt> {
        let start = self.peek_span();
        let mutability = match self.peek_kind() {
            TokenKind::Mutable => Mutability::Mutable,
            TokenKind::Const => Mutability::Const,
            TokenKind::Static => Mutability::Static,
            _ => unreachable!(),
        };
        self.advance();

        let (name, name_span) = self.pop_name(ops, op_spans)?;
        let (value, value_span) = match drain_ops(ops, op_spans) {
            Some((expr, span)) => (Some(expr), span),
            None => (None, Span::default()),
        };
        let ty = self.parse_type()?;
        let end = self.prev_span();

        Ok(Stmt::new(
            StmtKind::VarDecl {
                name,
                mutability,
                ty,
                value,
            },
            name_span.merge(value_span).merge(start).merge(end),
        ))
    }

    fn parse_set(&mut self, ops: &mut Vec<Expr>, op_spans: &mut Vec<Span>) -> ParseResult<Stmt> {
        let start = self.peek_span();
        let location = self.peek_location();
        self.advance();

        if ops.is_empty() {
            return Err(ParseError::new(
                "'set' requires a target variable",
                location,
                "E202",
            ));
        }

        let (target, target_span) = pop_target(ops, op_spans).ok_or_else(|| {
            ParseError::new("'set' target must be a variable or field", location, "E203")
        })?;
        let (value, value_span) = match drain_ops(ops, op_spans) {
            Some((expr, span)) => (Some(expr), span),
            None => (None, Span::default()),
        };

        Ok(Stmt::new(
            StmtKind::Set { target, value },
            target_span.merge(value_span).merge(start),
        ))
    }

    fn parse_for(&mut self, ops: &mut Vec<Expr>, op_spans: &mut Vec<Span>) -> ParseResult<Stmt> {
        let (source, source_span) =
            drain_ops(ops, op_spans).unwrap_or_else(|| (Expr::variable(""), Span::default()));
        let start = self.peek_span();
        self.advance();
        // Condition (`i 3 < for`) or iterable (`numbers for`). Binders before
        // `for` are not part of the surface; use `std.loop` for value/index.
        let body = self.body(&[TokenKind::End])?;
        self.expect(TokenKind::End, "expected 'end' after for block")?;
        let end = self.prev_span();
        Ok(Stmt::new(
            StmtKind::For { source, body },
            source_span.merge(start).merge(end),
        ))
    }

    fn parse_match(&mut self, ops: &mut Vec<Expr>, op_spans: &mut Vec<Span>) -> ParseResult<Stmt> {
        let (value, value_span) =
            drain_ops(ops, op_spans).unwrap_or_else(|| (Expr::variable(""), Span::default()));
        let start = self.peek_span();
        self.advance();

        let mut cases = Vec::new();
        let mut else_branch = Vec::new();

        loop {
            if self.match_kind(TokenKind::End) {
                break;
            }
            if self.match_kind(TokenKind::Else) {
                else_branch = self.body(&[TokenKind::End])?;
                self.expect(TokenKind::End, "expected 'end' after match else block")?;
                continue;
            }

            let location = self.peek_location();

            // A type case on a union subject: `<Type> case <body> end`. Try to
            // parse a full type first; it only wins when `case` follows
            // immediately (`reference<i32> case`), otherwise the tokens are an
            // ordinary condition expression and we rewind.
            let save = self.current;
            if let Ok(ty) = self.parse_type()
                && self.match_kind(TokenKind::Case)
            {
                let body = self.body(&[TokenKind::End])?;
                self.expect(TokenKind::End, "expected 'end' after case block")?;
                cases.push(MatchCase {
                    kind: MatchCaseKind::Type(ty),
                    body,
                });
                continue;
            }
            self.current = save;

            // An expression case: `<condition words> case <body> end`.
            let mut cond_ops = Vec::new();
            let mut cond_op_spans: Vec<Span> = Vec::new();
            loop {
                let kind = self.peek_kind();
                if kind == TokenKind::Case {
                    break;
                }
                if matches!(kind, TokenKind::End | TokenKind::Else | TokenKind::Eof) {
                    return Err(ParseError::new(
                        "expected a condition before 'case'",
                        location,
                        "E205",
                    ));
                }
                self.process_expr_word(&mut cond_ops, &mut cond_op_spans)?;
            }
            self.advance(); // consume 'case'
            let (condition, _) = drain_ops(&mut cond_ops, &mut cond_op_spans)
                .unwrap_or_else(|| (Expr::variable(""), Span::default()));
            let body = self.body(&[TokenKind::End])?;
            self.expect(TokenKind::End, "expected 'end' after case block")?;
            cases.push(MatchCase {
                kind: MatchCaseKind::Condition(condition),
                body,
            });
        }

        let end = self.prev_span();
        Ok(Stmt::new(
            StmtKind::Match {
                value,
                cases,
                else_branch,
            },
            value_span.merge(start).merge(end),
        ))
    }

    fn parse_require(
        &mut self,
        ops: &mut Vec<Expr>,
        op_spans: &mut Vec<Span>,
    ) -> ParseResult<Stmt> {
        let start = self.peek_span();
        let location = self.peek_location();
        self.advance();

        // In the `"<path>" [<scope>] require` form, the scope name (if any)
        // is pushed on the operand stack before the `require` keyword, above
        // the path string. So the top of the stack is either the scope name
        // or the path directly.
        let (path_expr, alias, stack_span) = match ops.pop() {
            Some(Expr::Variable { name }) => {
                let alias_span = op_spans.pop().unwrap_or_default();
                let path = ops.pop().ok_or_else(|| {
                    ParseError::new("'require' expects a module path string", location, "E207")
                })?;
                let path_span = op_spans.pop().unwrap_or_default();
                (path, Some(name), path_span.merge(alias_span))
            }
            other => {
                let path = other.ok_or_else(|| {
                    ParseError::new("'require' expects a module path string", location, "E207")
                })?;
                (path, None, op_spans.pop().unwrap_or_default())
            }
        };

        let path = match path_expr {
            Expr::String { value } => {
                let bytes = literals::decode_string_literal(&value)
                    .map_err(|m| ParseError::new(m, location, "E207"))?;
                String::from_utf8(bytes).map_err(|_| {
                    ParseError::new("module path must be valid UTF-8", location, "E207")
                })?
            }
            _ => {
                return Err(ParseError::new(
                    "'require' expects a string module path",
                    location,
                    "E207",
                ));
            }
        };

        Ok(Stmt::new(
            StmtKind::Require { path, alias },
            stack_span.merge(start),
        ))
    }

    // ------------------------------------------------------------------
    // Declarations
    // ------------------------------------------------------------------

    fn parse_function(
        &mut self,
        name: String,
        visibility: Option<Visibility>,
        is_unsafe: bool,
    ) -> ParseResult<Function> {
        self.expect(TokenKind::Function, "expected 'function'")?;

        let mut params = Vec::new();
        while self.peek_kind() != TokenKind::Do {
            params.push(self.parse_parameter()?);
        }
        self.expect(TokenKind::Do, "expected 'do' to start function body")?;

        let body = self.body(&[TokenKind::End])?;
        self.expect(TokenKind::End, "expected 'end' to close function body")?;

        let returns = if self.match_kind(TokenKind::With) {
            vec![self.parse_type()?]
        } else {
            Vec::new()
        };

        Ok(Function {
            name,
            visibility,
            params,
            body,
            returns,
            is_unsafe,
        })
    }

    fn parse_parameter(&mut self) -> ParseResult<Parameter> {
        let ty = self.parse_type()?;
        let modifier = if self.match_kind(TokenKind::Copy) {
            Some(ParamModifier::Copy)
        } else if self.match_kind(TokenKind::Mutable) {
            Some(ParamModifier::Mutable)
        } else {
            None
        };
        Ok(Parameter { ty, modifier })
    }

    fn parse_struct(
        &mut self,
        name: String,
        visibility: Option<Visibility>,
    ) -> ParseResult<StructDecl> {
        self.expect(TokenKind::Struct, "expected 'struct'")?;

        let mut fields = Vec::new();
        while self.peek_kind() != TokenKind::End {
            let ty = self.parse_type()?;
            let field_name = self
                .expect(TokenKind::Identifier, "expected field name")?
                .lexeme
                .clone();
            let field_vis = self.parse_visibility();
            fields.push(Field {
                name: field_name,
                ty,
                visibility: field_vis,
            });
        }
        self.expect(TokenKind::End, "expected 'end' to close struct")?;

        Ok(StructDecl {
            name,
            visibility,
            fields,
        })
    }

    fn parse_enum(&mut self, name: String, underlying: Option<Type>) -> ParseResult<EnumDecl> {
        self.expect(TokenKind::Enum, "expected 'enum'")?;

        let mut members = Vec::new();
        while self.peek_kind() != TokenKind::End {
            let member_name = self
                .expect(TokenKind::Identifier, "expected enum member")?
                .lexeme
                .clone();
            // An explicit value, e.g. `RED 10` (juxtaposed, like other words);
            // `-5` lexes as a single Integer token so negatives work too.
            let value = if self.peek_kind() == TokenKind::Integer {
                Some(self.advance().lexeme.clone())
            } else {
                None
            };
            members.push(EnumMember {
                name: member_name,
                value,
            });
        }
        self.expect(TokenKind::End, "expected 'end' to close enum")?;

        Ok(EnumDecl {
            name,
            underlying,
            members,
        })
    }

    fn parse_error_decl(
        &mut self,
        ops: &mut Vec<Expr>,
        op_spans: &mut Vec<Span>,
    ) -> ParseResult<(ErrorDecl, Span)> {
        let location = self.peek_location();
        self.expect(TokenKind::Error, "expected 'error'")?;

        // Operand stack: `Name` or `Name InjectPath` before the `error` keyword.
        let mut head_span = Span::default();
        let inject = match ops.last() {
            Some(Expr::Member { .. }) | Some(Expr::Variable { .. }) if ops.len() >= 2 => {
                let inject_span = op_spans.pop().unwrap_or_default();
                head_span = head_span.merge(inject_span);
                Some(expr_to_path(ops.pop().unwrap())?)
            }
            _ => None,
        };
        let (name, name_span) = self.pop_name(ops, op_spans).map_err(|_| {
            ParseError::new("'error' declaration requires a type name", location, "E230")
        })?;
        head_span = head_span.merge(name_span);

        let mut members = Vec::new();
        while self.peek_kind() != TokenKind::End {
            let member = self
                .expect(TokenKind::Identifier, "expected error member")?
                .lexeme
                .clone();
            members.push(member);
        }
        self.expect(TokenKind::End, "expected 'end' to close error")?;

        Ok((
            ErrorDecl {
                name,
                inject,
                members,
            },
            head_span,
        ))
    }

    fn parse_union(&mut self, name: String) -> ParseResult<UnionDecl> {
        self.expect(TokenKind::Union, "expected 'union'")?;

        let mut types = Vec::new();
        while self.peek_kind() != TokenKind::End {
            types.push(self.parse_type()?);
        }
        self.expect(TokenKind::End, "expected 'end' to close union")?;

        Ok(UnionDecl { name, types })
    }

    fn parse_implement(&mut self, target: String) -> ParseResult<Implement> {
        self.expect(TokenKind::Implement, "expected 'implement'")?;

        let mut functions = Vec::new();
        while self.peek_kind() != TokenKind::End {
            if self.peek_kind() != TokenKind::Identifier {
                return Err(ParseError::new(
                    "expected a function name in 'implement' block",
                    self.peek_location(),
                    "E208",
                ));
            }
            let name = self.advance().lexeme.clone();
            let visibility = self.parse_visibility();
            let is_unsafe = self.match_kind(TokenKind::Unsafe);
            functions.push(self.parse_function(name, visibility, is_unsafe)?);
        }
        self.expect(TokenKind::End, "expected 'end' to close implement")?;

        Ok(Implement { target, functions })
    }

    // ------------------------------------------------------------------
    // Types
    // ------------------------------------------------------------------

    fn parse_type(&mut self) -> ParseResult<Type> {
        let location = self.peek_location();

        if self.match_kind(TokenKind::Pipe) {
            let mut members = Vec::new();
            while self.peek_kind() != TokenKind::Pipe {
                if matches!(self.peek_kind(), TokenKind::End | TokenKind::Eof) {
                    return Err(ParseError::new(
                        "unterminated union type literal; expected '|'",
                        location,
                        "E231",
                    ));
                }
                members.push(self.parse_type()?);
            }
            self.expect(TokenKind::Pipe, "expected '|' to close union type literal")?;
            if members.is_empty() {
                return Err(ParseError::new(
                    "union type literal requires at least one member type",
                    location,
                    "E232",
                ));
            }
            return Ok(Type {
                kind: TypeKind::Union(members),
                location,
            });
        }

        let name = self.expect_type_name_start()?.to_string();
        let path = self.parse_type_path(name)?;

        let kind = if self.match_kind(TokenKind::Less) {
            let args = self.parse_type_args(location)?;
            self.build_generic(&path, args, location)?
        } else if let Some(primitive) = Primitive::parse_name(&path) {
            TypeKind::Primitive(primitive)
        } else {
            TypeKind::Named(path)
        };

        Ok(Type { kind, location })
    }

    /// First token of a type name: identifier or the soft `error` keyword.
    fn expect_type_name_start(&mut self) -> ParseResult<String> {
        match self.peek_kind() {
            TokenKind::Identifier => Ok(self.advance().lexeme.clone()),
            TokenKind::Error => {
                self.advance();
                Ok("error".to_string())
            }
            _ => Err(ParseError::new(
                "expected a type",
                self.peek_location(),
                "E207",
            )),
        }
    }

    fn parse_type_args(&mut self, location: Location) -> ParseResult<Vec<TypeArg>> {
        let mut args = Vec::new();
        while self.peek_kind() != TokenKind::Greater {
            if self.peek_kind() == TokenKind::Integer {
                let size =
                    self.advance().lexeme.parse::<u64>().map_err(|_| {
                        ParseError::new("invalid array size literal", location, "E209")
                    })?;
                args.push(TypeArg::Size(size));
            } else {
                args.push(TypeArg::Type(self.parse_type()?));
            }
        }
        self.expect(TokenKind::Greater, "expected '>' to close generic type")?;
        Ok(args)
    }

    fn build_generic(
        &self,
        path: &str,
        mut args: Vec<TypeArg>,
        location: Location,
    ) -> ParseResult<TypeKind> {
        match path {
            "array" => Ok(TypeKind::Array {
                element: Box::new(take(&mut args, 0, location)?),
                size: take_size(&mut args, 0),
            }),
            "list" => Ok(TypeKind::List {
                element: Box::new(take(&mut args, 0, location)?),
            }),
            "hashmap" => Ok(TypeKind::Hashmap {
                key: Box::new(take(&mut args, 0, location)?),
                value: Box::new(take(&mut args, 0, location)?),
            }),
            "reference" => Ok(TypeKind::Reference {
                inner: Box::new(take(&mut args, 0, location)?),
            }),
            "pointer" => Ok(TypeKind::Pointer {
                inner: Box::new(take(&mut args, 0, location)?),
            }),
            other => Err(ParseError::new(
                format!("type '{other}' does not accept type arguments"),
                location,
                "E210",
            )),
        }
    }

    fn parse_type_path(&mut self, first: String) -> ParseResult<String> {
        let mut path = first;
        loop {
            if self.peek_kind() == TokenKind::Dot {
                self.advance();
                let part = self
                    .expect(TokenKind::Identifier, "expected identifier after '.'")?
                    .lexeme
                    .clone();
                path.push('.');
                path.push_str(&part);
            } else {
                break;
            }
        }
        Ok(path)
    }

    // ------------------------------------------------------------------
    // Expressions
    // ------------------------------------------------------------------

    fn process_expr_word(
        &mut self,
        ops: &mut Vec<Expr>,
        op_spans: &mut Vec<Span>,
    ) -> ParseResult<()> {
        let kind = self.peek_kind();

        match kind {
            TokenKind::LeftSquare => {
                ops.push(self.parse_array_literal()?);
                note_op_span(op_spans, self.prev_span());
            }
            TokenKind::LeftParen => {
                ops.push(self.parse_list_literal()?);
                note_op_span(op_spans, self.prev_span());
            }
            TokenKind::LeftCurly => {
                ops.push(self.parse_map_literal()?);
                note_op_span(op_spans, self.prev_span());
            }
            TokenKind::Integer
            | TokenKind::Float
            | TokenKind::String
            | TokenKind::Rune
            | TokenKind::True
            | TokenKind::False => {
                let tok = self.advance();
                let expr = literal_expr(tok)?;
                ops.push(expr);
                note_op_span(op_spans, tok.span());
            }
            TokenKind::Identifier | TokenKind::Error => {
                let (name, mut span) = if self.peek_kind() == TokenKind::Error {
                    let tok = self.advance();
                    ("error".to_string(), tok.span())
                } else {
                    let tok = self.advance();
                    (tok.lexeme.clone(), tok.span())
                };
                let mut expr = Expr::variable(&name);
                while self.match_kind(TokenKind::Dot) {
                    let member = self.expect_member_name("expected member name after '.'")?;
                    expr = Expr::Member {
                        base: Box::new(expr),
                        member,
                    };
                    span = span.merge(self.prev_span());
                }
                // A bare primitive type name is a type value on the stack
                // (e.g. the `i32` in `myVar typeof i32 ==`), not a variable.
                // A dotted path (`error.CustomError`) stays a member access.
                if matches!(expr, Expr::Variable { .. }) && Primitive::parse_name(&name).is_some() {
                    ops.push(Expr::TypeValue { name });
                } else {
                    ops.push(expr);
                }
                note_op_span(op_spans, span);
            }
            TokenKind::Call => {
                self.advance();
                let call_span = self.prev_span();
                let target = ops.pop().ok_or_else(|| {
                    op_spans.pop();
                    ParseError::new(
                        "'call' requires a function to call",
                        self.peek_location(),
                        "E211",
                    )
                })?;
                op_spans.pop();
                ops.push(Expr::Call {
                    target: Box::new(target),
                });
                note_op_span(op_spans, call_span);
            }
            TokenKind::At => {
                let tok = self.advance();
                let name = tok.lexeme.strip_prefix('@').unwrap_or(&tok.lexeme);
                if name.is_empty() {
                    return Err(ParseError::new(
                        "expected a builtin name after '@'",
                        self.peek_location(),
                        "E217",
                    ));
                }
                ops.push(Expr::Builtin {
                    name: name.to_string(),
                });
                note_op_span(op_spans, tok.span());
            }
            TokenKind::Unwrap => {
                self.advance();
                let unwrap_span = self.prev_span();
                let inner = ops.pop().ok_or_else(|| {
                    op_spans.pop();
                    ParseError::new(
                        "'unwrap' requires a value to unwrap",
                        self.peek_location(),
                        "E212",
                    )
                })?;
                op_spans.pop();
                ops.push(Expr::Unwrap {
                    inner: Box::new(inner),
                });
                note_op_span(op_spans, unwrap_span);
            }
            TokenKind::Typeof => {
                self.advance();
                let typeof_span = self.prev_span();
                if let Some(inner) = ops.pop() {
                    op_spans.pop();
                    ops.push(Expr::Typeof {
                        inner: Box::new(inner),
                    });
                } else {
                    ops.push(Expr::ApplyTypeof);
                }
                note_op_span(op_spans, typeof_span);
            }
            TokenKind::Borrow => {
                self.advance();
                let borrow_span = self.prev_span();
                if let Some(inner) = ops.pop() {
                    op_spans.pop();
                    ops.push(Expr::Borrow {
                        inner: Box::new(inner),
                    });
                } else {
                    ops.push(Expr::ApplyBorrow);
                }
                note_op_span(op_spans, borrow_span);
            }
            TokenKind::Load => {
                self.advance();
                let load_span = self.prev_span();
                if let Some(inner) = ops.pop() {
                    op_spans.pop();
                    ops.push(Expr::Load {
                        inner: Box::new(inner),
                    });
                } else {
                    ops.push(Expr::ApplyLoad);
                }
                note_op_span(op_spans, load_span);
            }
            TokenKind::Store => {
                self.advance();
                let store_span = self.prev_span();
                let value = ops.pop().ok_or_else(|| {
                    op_spans.pop();
                    ParseError::new(
                        "'store' requires a value and an address",
                        self.peek_location(),
                        "E221",
                    )
                })?;
                op_spans.pop();
                let addr = ops.pop().ok_or_else(|| {
                    op_spans.pop();
                    ParseError::new("'store' requires an address", self.peek_location(), "E221")
                })?;
                op_spans.pop();
                ops.push(Expr::Store {
                    addr: Box::new(addr),
                    value: Box::new(value),
                });
                note_op_span(op_spans, store_span);
            }
            TokenKind::Dup => {
                self.advance();
                let dup_span = self.prev_span();
                if !ops.is_empty() {
                    ops.push(ops[ops.len() - 1].clone());
                    op_spans.push(*op_spans.last().unwrap());
                } else {
                    ops.push(Expr::StackOp(StackOp::Dup));
                    note_op_span(op_spans, dup_span);
                }
            }
            TokenKind::Swap => {
                self.advance();
                let swap_span = self.prev_span();
                if ops.len() >= 2 {
                    let n = ops.len();
                    ops.swap(n - 1, n - 2);
                    op_spans.swap(n - 1, n - 2);
                } else {
                    ops.push(Expr::StackOp(StackOp::Swap));
                    note_op_span(op_spans, swap_span);
                }
            }
            TokenKind::Rot => {
                self.advance();
                let rot_span = self.prev_span();
                if ops.len() >= 3 {
                    let first = ops.remove(0);
                    let first_span = op_spans.remove(0);
                    ops.push(first);
                    op_spans.push(first_span);
                } else {
                    ops.push(Expr::StackOp(StackOp::Rot));
                    note_op_span(op_spans, rot_span);
                }
            }
            TokenKind::Unrot => {
                self.advance();
                let unrot_span = self.prev_span();
                if ops.len() >= 3 {
                    let last = ops.pop().unwrap();
                    let last_span = op_spans.pop().unwrap();
                    ops.insert(0, last);
                    op_spans.insert(0, last_span);
                } else {
                    ops.push(Expr::StackOp(StackOp::Unrot));
                    note_op_span(op_spans, unrot_span);
                }
            }
            TokenKind::Pop => {
                self.advance();
                note_op_span(op_spans, self.prev_span());
                // A runtime pop, not a parse-time removal: the top `ops` entry
                // may be a call/builtin that produces its value during
                // compilation, so it cannot be discarded here.
                ops.push(Expr::StackOp(StackOp::Pop));
            }
            TokenKind::Drop => {
                self.advance();
                note_op_span(op_spans, self.prev_span());
                // Always a runtime drop. Parse-time `ops.clear()` would discard
                // preceding calls/builtins that still need to run for their
                // side effects (e.g. `foo call` then `drop`).
                ops.push(Expr::StackOp(StackOp::Drop));
            }
            TokenKind::Not => {
                self.advance();
                let not_span = self.prev_span();
                let can_combine = ops.last().is_some_and(is_value);
                if can_combine {
                    let operand = ops.pop().unwrap();
                    op_spans.pop();
                    ops.push(Expr::Unary {
                        op: UnOp::Not,
                        operand: Box::new(operand),
                    });
                } else {
                    ops.push(Expr::ApplyUn(UnOp::Not));
                }
                note_op_span(op_spans, not_span);
            }
            _ => {
                if let Some(op) = binary_op(kind) {
                    self.advance();
                    let bin_span = self.prev_span();
                    let can_combine = ops.len() >= 2
                        && is_value(&ops[ops.len() - 1])
                        && is_value(&ops[ops.len() - 2]);
                    if can_combine {
                        let right = ops.pop().unwrap();
                        op_spans.pop();
                        let left = ops.pop().unwrap();
                        op_spans.pop();
                        ops.push(Expr::Binary {
                            op,
                            left: Box::new(left),
                            right: Box::new(right),
                        });
                    } else {
                        ops.push(Expr::ApplyBin(op));
                    }
                    note_op_span(op_spans, bin_span);
                } else {
                    return Err(ParseError::new(
                        format!("unexpected token '{kind:?}' in expression"),
                        self.peek_location(),
                        "E214",
                    ));
                }
            }
        }

        Ok(())
    }

    // ------------------------------------------------------------------
    // Container literals
    // ------------------------------------------------------------------

    fn parse_array_literal(&mut self) -> ParseResult<Expr> {
        self.advance(); // '['
        let elements = self.parse_container_elements(TokenKind::RightSquare)?;
        self.expect(
            TokenKind::RightSquare,
            "expected ']' to close array literal",
        )?;
        Ok(Expr::Array(elements))
    }

    fn parse_list_literal(&mut self) -> ParseResult<Expr> {
        self.advance(); // '('
        let elements = self.parse_container_elements(TokenKind::RightParen)?;
        self.expect(TokenKind::RightParen, "expected ')' to close list literal")?;
        Ok(Expr::List(elements))
    }

    fn parse_map_literal(&mut self) -> ParseResult<Expr> {
        self.advance(); // '{'
        let mut pairs = Vec::new();
        while self.peek_kind() != TokenKind::RightCurly {
            let key = self.parse_literal_element()?;
            let value = self.parse_literal_element()?;
            pairs.push((key, value));
        }
        self.expect(TokenKind::RightCurly, "expected '}' to close map literal")?;
        Ok(Expr::Map(pairs))
    }

    fn parse_container_elements(&mut self, close: TokenKind) -> ParseResult<Vec<Expr>> {
        let mut elements = Vec::new();
        while self.peek_kind() != close {
            elements.push(self.parse_literal_element()?);
        }
        Ok(elements)
    }

    fn parse_literal_element(&mut self) -> ParseResult<Expr> {
        match self.peek_kind() {
            TokenKind::Integer
            | TokenKind::Float
            | TokenKind::String
            | TokenKind::Rune
            | TokenKind::True
            | TokenKind::False => {
                let tok = self.advance();
                literal_expr(tok)
            }
            TokenKind::Identifier => {
                let tok = self.advance();
                let mut expr = Expr::variable(&tok.lexeme);
                while self.match_kind(TokenKind::Dot) {
                    let member = self.expect_member_name("expected member after '.'")?;
                    expr = Expr::Member {
                        base: Box::new(expr),
                        member,
                    };
                }
                Ok(expr)
            }
            TokenKind::LeftSquare => self.parse_array_literal(),
            TokenKind::LeftParen => self.parse_list_literal(),
            TokenKind::LeftCurly => self.parse_map_literal(),
            _ => Err(ParseError::new(
                "unexpected token in container literal",
                self.peek_location(),
                "E215",
            )),
        }
    }

    // ------------------------------------------------------------------
    // Token helpers
    // ------------------------------------------------------------------

    fn pop_name(
        &mut self,
        ops: &mut Vec<Expr>,
        op_spans: &mut Vec<Span>,
    ) -> ParseResult<(String, Span)> {
        let location = self.peek_location();
        let expr = ops.pop().ok_or_else(|| {
            ParseError::new(
                "expected a name before declaration keyword",
                location,
                "E216",
            )
        })?;
        let span = op_spans.pop().unwrap_or_default();
        match expr {
            Expr::Variable { name } => Ok((name, span)),
            _ => Err(ParseError::new(
                "expected an identifier name",
                location,
                "E216",
            )),
        }
    }

    /// `Name [underlying] enum`: optional type value sits above the enum name.
    fn pop_enum_head(
        &mut self,
        ops: &mut Vec<Expr>,
        op_spans: &mut Vec<Span>,
    ) -> ParseResult<(String, Option<Type>, Span)> {
        let location = self.peek_location();
        let mut head_span = Span::default();
        let underlying = match ops.last() {
            Some(Expr::TypeValue { name }) => {
                let name = name.clone();
                ops.pop();
                head_span = op_spans.pop().unwrap_or_default();
                let kind = if let Some(p) = Primitive::parse_name(&name) {
                    TypeKind::Primitive(p)
                } else {
                    TypeKind::Named(name)
                };
                Some(Type { kind, location })
            }
            Some(Expr::Variable { name }) if ops.len() >= 2 => {
                // Named underlying type that is not a primitive.
                let name = name.clone();
                ops.pop();
                head_span = op_spans.pop().unwrap_or_default();
                Some(Type {
                    kind: TypeKind::Named(name),
                    location,
                })
            }
            _ => None,
        };
        let (name, name_span) = self.pop_name(ops, op_spans)?;
        head_span = head_span.merge(name_span);
        Ok((name, underlying, head_span))
    }

    fn parse_visibility(&mut self) -> Option<Visibility> {
        match self.peek_kind() {
            TokenKind::Public => {
                self.advance();
                Some(Visibility::Public)
            }
            TokenKind::Private => {
                self.advance();
                Some(Visibility::Private)
            }
            _ => None,
        }
    }

    /// After an optional visibility token was consumed: finish the declaration.
    fn parse_visible_decl(
        &mut self,
        ops: &mut Vec<Expr>,
        op_spans: &mut Vec<Span>,
        visibility: Visibility,
    ) -> ParseResult<Stmt> {
        match self.peek_kind() {
            TokenKind::Unsafe => {
                let start = self.peek_span();
                self.advance();
                let (name, stack_span) = self.pop_name(ops, op_spans)?;
                let fn_start = self.peek_span();
                let func = self.parse_function(name, Some(visibility), true)?;
                let end = self.prev_span();
                Ok(Stmt::new(
                    StmtKind::Function(func),
                    stack_span.merge(start).merge(fn_start).merge(end),
                ))
            }
            TokenKind::Function => {
                let (name, stack_span) = self.pop_name(ops, op_spans)?;
                let start = self.peek_span();
                let func = self.parse_function(name, Some(visibility), false)?;
                let end = self.prev_span();
                Ok(Stmt::new(
                    StmtKind::Function(func),
                    stack_span.merge(start).merge(end),
                ))
            }
            TokenKind::Struct => {
                let (name, stack_span) = self.pop_name(ops, op_spans)?;
                let start = self.peek_span();
                let decl = self.parse_struct(name, Some(visibility))?;
                let end = self.prev_span();
                Ok(Stmt::new(
                    StmtKind::Struct(decl),
                    stack_span.merge(start).merge(end),
                ))
            }
            _ => Err(ParseError::new(
                "visibility must precede 'function', 'unsafe function', or 'struct'",
                self.peek_location(),
                "E233",
            )),
        }
    }

    /// True if tokens starting at `offset` from current form a function head
    /// (`[public|private] [unsafe] function`).
    fn peek_is_function_head(&self, offset: usize) -> bool {
        let kind_at = |i: usize| {
            self.tokens
                .get(self.current + i)
                .map(|t| t.kind)
                .unwrap_or(TokenKind::Eof)
        };
        let mut i = offset;
        if matches!(kind_at(i), TokenKind::Public | TokenKind::Private) {
            i += 1;
        }
        if kind_at(i) == TokenKind::Unsafe {
            i += 1;
        }
        kind_at(i) == TokenKind::Function
    }

    fn peek_kind(&self) -> TokenKind {
        self.tokens[self.current].kind
    }

    fn peek_next_kind(&self) -> TokenKind {
        self.tokens
            .get(self.current + 1)
            .map(|t| t.kind)
            .unwrap_or(TokenKind::Eof)
    }

    fn peek_lexeme(&self) -> String {
        self.tokens[self.current].lexeme.clone()
    }

    fn peek_location(&self) -> Location {
        self.tokens[self.current].location
    }

    fn peek_span(&self) -> Span {
        self.tokens[self.current].span()
    }

    fn prev_span(&self) -> Span {
        if self.current == 0 {
            self.tokens[0].span()
        } else {
            self.tokens[self.current - 1].span()
        }
    }

    fn advance(&mut self) -> &Token {
        if self.current >= self.tokens.len() {
            return self.tokens.last().unwrap();
        }
        let tok = &self.tokens[self.current];
        self.current += 1;
        tok
    }

    /// Consume a member name after `.`. `load`/`store` are keywords (typed
    /// pointer words) but are also valid member/function names (e.g. the
    /// `std.mem` functions used as `mem.load`/`mem.store`).
    fn expect_member_name(&mut self, message: &str) -> ParseResult<String> {
        let kind = self.peek_kind();
        if matches!(
            kind,
            TokenKind::Identifier | TokenKind::Load | TokenKind::Store
        ) {
            Ok(self.advance().lexeme.clone())
        } else {
            Err(ParseError::new(message, self.peek_location(), "E217"))
        }
    }

    fn match_kind(&mut self, kind: TokenKind) -> bool {
        if self.peek_kind() == kind {
            self.advance();
            true
        } else {
            false
        }
    }

    fn expect(&mut self, kind: TokenKind, message: &str) -> ParseResult<&Token> {
        if self.peek_kind() == kind {
            Ok(self.advance())
        } else {
            Err(ParseError::new(message, self.peek_location(), "E217"))
        }
    }
}

// ---------------------------------------------------------------------------
// Free helpers
// ---------------------------------------------------------------------------

enum TypeArg {
    Type(Type),
    Size(u64),
}

fn take(args: &mut Vec<TypeArg>, index: usize, location: Location) -> ParseResult<Type> {
    if args.len() <= index {
        return Err(ParseError::new("missing type argument", location, "E218"));
    }
    match args.remove(index) {
        TypeArg::Type(t) => Ok(t),
        _ => Err(ParseError::new(
            "expected a type argument",
            location,
            "E218",
        )),
    }
}

fn take_size(args: &mut Vec<TypeArg>, index: usize) -> Option<u64> {
    if args.len() <= index {
        return None;
    }
    match args.remove(index) {
        TypeArg::Size(n) => Some(n),
        _ => None,
    }
}

fn note_op_span(op_spans: &mut Vec<Span>, span: Span) {
    op_spans.push(span);
}

/// Drain ops + spans. Returns `(expr, merged_span)`.
fn drain_ops(ops: &mut Vec<Expr>, op_spans: &mut Vec<Span>) -> Option<(Expr, Span)> {
    match ops.len() {
        0 => {
            op_spans.clear();
            None
        }
        1 => {
            let e = ops.pop().unwrap();
            let s = op_spans.pop().unwrap_or_default();
            op_spans.clear();
            Some((e, s))
        }
        n => {
            debug_assert_eq!(op_spans.len(), n, "ops/op_spans length mismatch");
            let seq = std::mem::take(ops);
            let spans = std::mem::take(op_spans);
            let merged = spans
                .iter()
                .copied()
                .reduce(|a, b| a.merge(b))
                .unwrap_or_default();
            Some((Expr::Seq(seq.into_iter().zip(spans).collect()), merged))
        }
    }
}

/// Pull the `fallback` statement out of a `handle` body, returning the
/// remaining statements and the fallback value (if any).
fn extract_fallback(body: Vec<Stmt>) -> (Vec<Stmt>, Option<Expr>) {
    let mut fallback = None;
    let mut stmts = Vec::with_capacity(body.len());
    for stmt in body {
        match stmt.kind {
            StmtKind::Fallback { value } => {
                if fallback.is_none() {
                    fallback = value;
                }
            }
            _ => stmts.push(stmt),
        }
    }
    (stmts, fallback)
}

fn pop_target(ops: &mut Vec<Expr>, op_spans: &mut Vec<Span>) -> Option<(Expr, Span)> {
    let last = ops.last()?;
    if matches!(last, Expr::Variable { .. } | Expr::Member { .. }) {
        let e = ops.pop()?;
        let s = op_spans.pop().unwrap_or_default();
        return Some((e, s));
    }
    // Fall back to the first variable/field anywhere in the operand list.
    let index = ops
        .iter()
        .position(|e| matches!(e, Expr::Variable { .. } | Expr::Member { .. }))?;
    let e = ops.remove(index);
    let s = op_spans.remove(index);
    Some((e, s))
}

fn literal_expr(tok: &Token) -> ParseResult<Expr> {
    let loc = tok.location;
    match tok.kind {
        TokenKind::Integer => {
            literals::decode_int_literal(&tok.lexeme)
                .map_err(|m| ParseError::new(m, loc, "E217"))?;
            Ok(Expr::Integer {
                value: tok.lexeme.clone(),
            })
        }
        TokenKind::Float => {
            literals::decode_float_literal(&tok.lexeme)
                .map_err(|m| ParseError::new(m, loc, "E218"))?;
            Ok(Expr::Float {
                value: tok.lexeme.clone(),
            })
        }
        TokenKind::String => Ok(Expr::String {
            value: tok.lexeme.clone(),
        }),
        TokenKind::Rune => {
            literals::decode_rune_literal(&tok.lexeme)
                .map_err(|m| ParseError::new(m, loc, "E219"))?;
            Ok(Expr::Rune {
                value: tok.lexeme.clone(),
            })
        }
        TokenKind::True => Ok(Expr::Bool { value: true }),
        TokenKind::False => Ok(Expr::Bool { value: false }),
        _ => Ok(Expr::variable("")),
    }
}

fn is_value(e: &Expr) -> bool {
    matches!(
        e,
        Expr::Integer { .. }
            | Expr::Float { .. }
            | Expr::String { .. }
            | Expr::Rune { .. }
            | Expr::Bool { .. }
            | Expr::Variable { .. }
            | Expr::Member { .. }
            | Expr::TypeValue { .. }
            | Expr::Binary { .. }
            | Expr::Unary { .. }
            | Expr::Call { .. }
            | Expr::Typeof { .. }
            | Expr::Borrow { .. }
            | Expr::Array(_)
            | Expr::List(_)
            | Expr::Map(_)
            | Expr::Seq(_)
    )
}

fn binary_op(kind: TokenKind) -> Option<BinOp> {
    Some(match kind {
        TokenKind::Plus => BinOp::Plus,
        TokenKind::Minus => BinOp::Minus,
        TokenKind::Asterisk => BinOp::Mul,
        TokenKind::Slash => BinOp::Div,
        TokenKind::SlashSlash => BinOp::Fdiv,
        TokenKind::Percent => BinOp::Mod,
        TokenKind::Caret => BinOp::Pow,
        TokenKind::Tilde => BinOp::Concat,
        TokenKind::EqualEqual => BinOp::Eq,
        TokenKind::NotEqual => BinOp::Ne,
        TokenKind::Greater => BinOp::Gt,
        TokenKind::GreaterEqual => BinOp::Gte,
        TokenKind::Less => BinOp::Lt,
        TokenKind::LessEqual => BinOp::Lte,
        TokenKind::And => BinOp::And,
        TokenKind::Or => BinOp::Or,
        TokenKind::Xor => BinOp::Xor,
        TokenKind::LeftShift => BinOp::Lshift,
        TokenKind::RightShift => BinOp::Rshift,
        _ => return None,
    })
}

fn expr_to_path(expr: Expr) -> ParseResult<String> {
    match expr {
        Expr::Variable { name } => Ok(name),
        Expr::Member { base, member } => {
            let base = expr_to_path(*base)?;
            Ok(format!("{base}.{member}"))
        }
        _ => Err(ParseError::new(
            "expected a qualified name",
            Location::default(),
            "E234",
        )),
    }
}

pub fn parse(tokens: Vec<Token>) -> Result<Program, DiagnosticBatch> {
    Parser::new(tokens).parse()
}
