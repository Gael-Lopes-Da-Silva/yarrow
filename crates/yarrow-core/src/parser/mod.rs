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
}

impl Parser {
    pub fn new(tokens: Vec<Token>) -> Self {
        Self { tokens, current: 0 }
    }

    pub fn parse(&mut self) -> ParseResult<Program> {
        let items = self.body(&[TokenKind::Eof])?;
        Ok(Program { items })
    }

    // ------------------------------------------------------------------
    // Statement parsing
    // ------------------------------------------------------------------

    fn body(&mut self, stops: &[TokenKind]) -> ParseResult<Vec<Stmt>> {
        let mut stmts = Vec::new();
        let mut ops: Vec<Expr> = Vec::new();

        loop {
            let kind = self.peek_kind();

            if stops.contains(&kind) {
                break;
            }

            match kind {
                TokenKind::End | TokenKind::Else | TokenKind::Case | TokenKind::Eof => break,

                TokenKind::Mutable | TokenKind::Const | TokenKind::Static => {
                    let decl = self.parse_var_decl(&mut ops)?;
                    stmts.push(decl);
                }

                TokenKind::Function => {
                    let name = self.pop_name(&mut ops)?;
                    let func = self.parse_function(name)?;
                    stmts.push(Stmt::Function(func));
                }

                TokenKind::Struct => {
                    let name = self.pop_name(&mut ops)?;
                    let decl = self.parse_struct(name)?;
                    stmts.push(Stmt::Struct(decl));
                }

                TokenKind::Enum => {
                    let name = self.pop_name(&mut ops)?;
                    let decl = self.parse_enum(name)?;
                    stmts.push(Stmt::Enum(decl));
                }

                TokenKind::Union => {
                    let name = self.pop_name(&mut ops)?;
                    let decl = self.parse_union(name)?;
                    stmts.push(Stmt::Union(decl));
                }

                TokenKind::Implement => {
                    let name = self.pop_name(&mut ops)?;
                    let impls = self.parse_implement(name)?;
                    stmts.push(Stmt::Implement(impls));
                }

                TokenKind::Require => {
                    let req = self.parse_require(&mut ops)?;
                    stmts.push(req);
                }

                TokenKind::Set => {
                    let set = self.parse_set(&mut ops)?;
                    stmts.push(set);
                }

                TokenKind::If => {
                    let condition = drain_ops(&mut ops).unwrap_or_else(|| Expr::variable(""));
                    self.advance();
                    let then_branch = self.body(&[TokenKind::Else, TokenKind::End])?;
                    let else_branch = if self.match_kind(TokenKind::Else) {
                        self.body(&[TokenKind::End])?
                    } else {
                        Vec::new()
                    };
                    self.expect(TokenKind::End, "expected 'end' after if/else block")?;
                    stmts.push(Stmt::If {
                        condition,
                        then_branch,
                        else_branch,
                    });
                }

                TokenKind::For => {
                    let stmt = self.parse_for(&mut ops)?;
                    stmts.push(stmt);
                }

                TokenKind::Match => {
                    let stmt = self.parse_match(&mut ops)?;
                    stmts.push(stmt);
                }

                TokenKind::Defer => {
                    self.advance();
                    let body = self.body(&[TokenKind::End])?;
                    self.expect(TokenKind::End, "expected 'end' after defer block")?;
                    stmts.push(Stmt::Defer { body });
                }

                TokenKind::Handle => {
                    if let Some(expr) = drain_ops(&mut ops) {
                        stmts.push(Stmt::Expr(expr));
                    }
                    self.advance();
                    let body = self.body(&[TokenKind::End])?;
                    self.expect(TokenKind::End, "expected 'end' after handle block")?;
                    let (body, fallback) = extract_fallback(body);
                    stmts.push(Stmt::Handle { body, fallback });
                }

                TokenKind::Move => {
                    let location = self.peek_location();
                    self.advance();
                    let target = ops.pop().ok_or_else(|| {
                        ParseError::new("'move' requires a target variable", location, "E220")
                    })?;
                    let target = match target {
                        Expr::Variable { name } => name,
                        _ => {
                            return Err(ParseError::new(
                                "'move' requires a target variable",
                                location,
                                "E221",
                            ));
                        }
                    };
                    let source = drain_ops(&mut ops).unwrap_or_else(|| Expr::variable(""));
                    stmts.push(Stmt::Move { target, source });
                }

                TokenKind::Fallback => {
                    self.advance();
                    let value = drain_ops(&mut ops);
                    stmts.push(Stmt::Fallback { value });
                }

                TokenKind::Return => {
                    self.advance();
                    if let Some(expr) = drain_ops(&mut ops) {
                        stmts.push(Stmt::Expr(expr));
                    }
                    stmts.push(Stmt::Return { value: None });
                }

                TokenKind::Break => {
                    self.advance();
                    stmts.push(Stmt::Break);
                }

                TokenKind::Continue => {
                    self.advance();
                    stmts.push(Stmt::Continue);
                }

                _ => self.process_expr_word(&mut ops)?,
            }
        }

        if let Some(expr) = drain_ops(&mut ops) {
            stmts.push(Stmt::Expr(expr));
        }

        Ok(stmts)
    }

    fn parse_var_decl(&mut self, ops: &mut Vec<Expr>) -> ParseResult<Stmt> {
        let mutability = match self.peek_kind() {
            TokenKind::Mutable => Mutability::Mutable,
            TokenKind::Const => Mutability::Const,
            TokenKind::Static => Mutability::Static,
            _ => unreachable!(),
        };
        self.advance();

        let name = self.pop_name(ops)?;
        let value = drain_ops(ops);
        let ty = self.parse_type()?;

        Ok(Stmt::VarDecl {
            name,
            mutability,
            ty,
            value,
        })
    }

    fn parse_set(&mut self, ops: &mut Vec<Expr>) -> ParseResult<Stmt> {
        let location = self.peek_location();
        self.advance();

        if ops.is_empty() {
            return Err(ParseError::new(
                "'set' requires a target variable",
                location,
                "E202",
            ));
        }

        let target = pop_target(ops).ok_or_else(|| {
            ParseError::new("'set' target must be a variable or field", location, "E203")
        })?;
        let value = drain_ops(ops);

        Ok(Stmt::Set { target, value })
    }

    fn parse_for(&mut self, ops: &mut Vec<Expr>) -> ParseResult<Stmt> {
        let location = self.peek_location();
        self.advance();

        // Distinguish the three `for` forms by the operand stack:
        //   condition form:  `<condition> for`              (top is not a bare name)
        //   value form:      `<iterable> <var> for`         (two operands, top is a name)
        //   index form:      `<iterable> <var> <index> for` (three+ operands, top two are names)
        let n = ops.len();
        let top_is_var = matches!(ops.last(), Some(Expr::Variable { .. }));
        let second_is_var = matches!(ops.get(n.saturating_sub(2)), Some(Expr::Variable { .. }));

        let (source, value, index) = if n >= 2 && top_is_var && second_is_var {
            if n == 2 {
                let value = pop_var_name(ops, location)?;
                let source = drain_ops(ops).unwrap_or_else(|| Expr::variable(""));
                (source, Some(value), None)
            } else {
                let index = pop_var_name(ops, location)?;
                let value = pop_var_name(ops, location)?;
                let source = drain_ops(ops).unwrap_or_else(|| Expr::variable(""));
                (source, Some(value), Some(index))
            }
        } else if n >= 2 && top_is_var {
            let value = pop_var_name(ops, location)?;
            let source = drain_ops(ops).unwrap_or_else(|| Expr::variable(""));
            (source, Some(value), None)
        } else {
            let condition = drain_ops(ops).unwrap_or_else(|| Expr::variable(""));
            (condition, None, None)
        };

        let body = self.body(&[TokenKind::End])?;
        self.expect(TokenKind::End, "expected 'end' after for block")?;

        Ok(Stmt::For {
            source,
            value,
            index,
            body,
        })
    }

    fn parse_match(&mut self, ops: &mut Vec<Expr>) -> ParseResult<Stmt> {
        let value = drain_ops(ops).unwrap_or_else(|| Expr::variable(""));
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
                self.process_expr_word(&mut cond_ops)?;
            }
            self.advance(); // consume 'case'
            let condition = drain_ops(&mut cond_ops).unwrap_or_else(|| Expr::variable(""));
            let body = self.body(&[TokenKind::End])?;
            self.expect(TokenKind::End, "expected 'end' after case block")?;
            cases.push(MatchCase {
                kind: MatchCaseKind::Condition(condition),
                body,
            });
        }

        Ok(Stmt::Match {
            value,
            cases,
            else_branch,
        })
    }

    fn parse_require(&mut self, ops: &mut Vec<Expr>) -> ParseResult<Stmt> {
        let location = self.peek_location();
        self.advance();

        // In the `"<path>" [<scope>] require` form, the scope name (if any)
        // is pushed on the operand stack before the `require` keyword, above
        // the path string. So the top of the stack is either the scope name
        // or the path directly.
        let (path_expr, alias) = match ops.pop() {
            Some(Expr::Variable { name }) => {
                let path = ops.pop().ok_or_else(|| {
                    ParseError::new("'require' expects a module path string", location, "E207")
                })?;
                (path, Some(name))
            }
            other => (
                other.ok_or_else(|| {
                    ParseError::new("'require' expects a module path string", location, "E207")
                })?,
                None,
            ),
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

        Ok(Stmt::Require { path, alias })
    }

    // ------------------------------------------------------------------
    // Declarations
    // ------------------------------------------------------------------

    fn parse_function(&mut self, name: String) -> ParseResult<Function> {
        self.expect(TokenKind::Function, "expected 'function'")?;

        let mut params = Vec::new();
        while self.peek_kind() != TokenKind::Do {
            params.push(self.parse_type()?);
        }
        self.expect(TokenKind::Do, "expected 'do' to start function body")?;

        let body = self.body(&[TokenKind::End])?;
        self.expect(TokenKind::End, "expected 'end' to close function body")?;

        let returns = if self.match_kind(TokenKind::With) {
            self.parse_type_union()?
        } else {
            Vec::new()
        };

        Ok(Function {
            name,
            params,
            body,
            returns,
        })
    }

    fn parse_struct(&mut self, name: String) -> ParseResult<StructDecl> {
        self.expect(TokenKind::Struct, "expected 'struct'")?;

        let mut fields = Vec::new();
        while self.peek_kind() != TokenKind::End {
            let ty = self.parse_type()?;
            let field_name = self
                .expect(TokenKind::Identifier, "expected field name")?
                .lexeme
                .clone();
            fields.push(Field {
                name: field_name,
                ty,
            });
        }
        self.expect(TokenKind::End, "expected 'end' to close struct")?;

        Ok(StructDecl { name, fields })
    }

    fn parse_enum(&mut self, name: String) -> ParseResult<EnumDecl> {
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

        Ok(EnumDecl { name, members })
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
            functions.push(self.parse_function(name)?);
        }
        self.expect(TokenKind::End, "expected 'end' to close implement")?;

        Ok(Implement { target, functions })
    }

    // ------------------------------------------------------------------
    // Types
    // ------------------------------------------------------------------

    fn parse_type_union(&mut self) -> ParseResult<Vec<Type>> {
        let mut types = vec![self.parse_type()?];
        while self.match_kind(TokenKind::Or) {
            types.push(self.parse_type()?);
        }
        Ok(types)
    }

    fn parse_type(&mut self) -> ParseResult<Type> {
        let location = self.peek_location();

        let name = self
            .expect(TokenKind::Identifier, "expected a type")?
            .lexeme
            .clone();

        // Module/error paths like `error.CustomError`.
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

    fn process_expr_word(&mut self, ops: &mut Vec<Expr>) -> ParseResult<()> {
        let kind = self.peek_kind();

        match kind {
            TokenKind::LeftSquare => {
                ops.push(self.parse_array_literal()?);
            }
            TokenKind::LeftParen => {
                ops.push(self.parse_list_literal()?);
            }
            TokenKind::LeftCurly => {
                ops.push(self.parse_map_literal()?);
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
            }
            TokenKind::Identifier => {
                let name = self.advance().lexeme.clone();
                let mut expr = Expr::variable(&name);
                while self.match_kind(TokenKind::Dot) {
                    let member = self
                        .expect(TokenKind::Identifier, "expected member name after '.'")?
                        .lexeme
                        .clone();
                    expr = Expr::Member {
                        base: Box::new(expr),
                        member,
                    };
                }
                // A bare primitive type name is a type value on the stack
                // (e.g. the `i32` in `myVar typeof i32 ==`), not a variable.
                // A dotted path (`error.CustomError`) stays a member access.
                if matches!(expr, Expr::Variable { .. }) && Primitive::parse_name(&name).is_some() {
                    ops.push(Expr::TypeValue { name });
                } else {
                    ops.push(expr);
                }
            }
            TokenKind::Call => {
                self.advance();
                let target = ops.pop().ok_or_else(|| {
                    ParseError::new(
                        "'call' requires a function to call",
                        self.peek_location(),
                        "E211",
                    )
                })?;
                ops.push(Expr::Call {
                    target: Box::new(target),
                });
            }
            TokenKind::At => {
                let lexeme = self.advance().lexeme.clone();
                let name = lexeme.strip_prefix('@').unwrap_or(&lexeme);
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
            }
            TokenKind::Unwrap => {
                self.advance();
                let inner = ops.pop().ok_or_else(|| {
                    ParseError::new(
                        "'unwrap' requires a value to unwrap",
                        self.peek_location(),
                        "E212",
                    )
                })?;
                ops.push(Expr::Unwrap {
                    inner: Box::new(inner),
                });
            }
            TokenKind::Typeof => {
                self.advance();
                if let Some(inner) = ops.pop() {
                    ops.push(Expr::Typeof {
                        inner: Box::new(inner),
                    });
                } else {
                    ops.push(Expr::ApplyTypeof);
                }
            }
            TokenKind::Borrow => {
                self.advance();
                if let Some(inner) = ops.pop() {
                    ops.push(Expr::Borrow {
                        inner: Box::new(inner),
                    });
                } else {
                    ops.push(Expr::ApplyBorrow);
                }
            }
            TokenKind::Load => {
                self.advance();
                if let Some(inner) = ops.pop() {
                    ops.push(Expr::Load {
                        inner: Box::new(inner),
                    });
                } else {
                    ops.push(Expr::ApplyLoad);
                }
            }
            TokenKind::Store => {
                self.advance();
                let value = ops.pop().ok_or_else(|| {
                    ParseError::new(
                        "'store' requires a value and an address",
                        self.peek_location(),
                        "E221",
                    )
                })?;
                let addr = ops.pop().ok_or_else(|| {
                    ParseError::new("'store' requires an address", self.peek_location(), "E221")
                })?;
                ops.push(Expr::Store {
                    addr: Box::new(addr),
                    value: Box::new(value),
                });
            }
            TokenKind::Dup => {
                self.advance();
                if !ops.is_empty() {
                    ops.push(ops[ops.len() - 1].clone());
                } else {
                    ops.push(Expr::StackOp(StackOp::Dup));
                }
            }
            TokenKind::Swap => {
                self.advance();
                if ops.len() >= 2 {
                    let n = ops.len();
                    ops.swap(n - 1, n - 2);
                } else {
                    ops.push(Expr::StackOp(StackOp::Swap));
                }
            }
            TokenKind::Rot => {
                self.advance();
                if ops.len() >= 3 {
                    let first = ops.remove(0);
                    ops.push(first);
                } else {
                    ops.push(Expr::StackOp(StackOp::Rot));
                }
            }
            TokenKind::Unrot => {
                self.advance();
                if ops.len() >= 3 {
                    let last = ops.pop().unwrap();
                    ops.insert(0, last);
                } else {
                    ops.push(Expr::StackOp(StackOp::Unrot));
                }
            }
            TokenKind::Pop => {
                self.advance();
                // A runtime pop, not a parse-time removal: the top `ops` entry
                // may be a call/builtin that produces its value during
                // compilation, so it cannot be discarded here.
                ops.push(Expr::StackOp(StackOp::Pop));
            }
            TokenKind::Drop => {
                self.advance();
                if !ops.is_empty() {
                    ops.clear();
                } else {
                    ops.push(Expr::StackOp(StackOp::Drop));
                }
            }
            TokenKind::Not => {
                self.advance();
                let can_combine = ops.last().is_some_and(is_value);
                if can_combine {
                    let operand = ops.pop().unwrap();
                    ops.push(Expr::Unary {
                        op: UnOp::Not,
                        operand: Box::new(operand),
                    });
                } else {
                    ops.push(Expr::ApplyUn(UnOp::Not));
                }
            }
            _ => {
                if let Some(op) = binary_op(kind) {
                    self.advance();
                    let can_combine = ops.len() >= 2
                        && is_value(&ops[ops.len() - 1])
                        && is_value(&ops[ops.len() - 2]);
                    if can_combine {
                        let right = ops.pop().unwrap();
                        let left = ops.pop().unwrap();
                        ops.push(Expr::Binary {
                            op,
                            left: Box::new(left),
                            right: Box::new(right),
                        });
                    } else {
                        ops.push(Expr::ApplyBin(op));
                    }
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
                    let member = self
                        .expect(TokenKind::Identifier, "expected member after '.'")?
                        .lexeme
                        .clone();
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

    fn pop_name(&mut self, ops: &mut Vec<Expr>) -> ParseResult<String> {
        let location = self.peek_location();
        let expr = ops.pop().ok_or_else(|| {
            ParseError::new(
                "expected a name before declaration keyword",
                location,
                "E216",
            )
        })?;
        match expr {
            Expr::Variable { name } => Ok(name),
            _ => Err(ParseError::new(
                "expected an identifier name",
                location,
                "E216",
            )),
        }
    }

    fn peek_kind(&self) -> TokenKind {
        self.tokens[self.current].kind
    }

    fn peek_location(&self) -> Location {
        self.tokens[self.current].location
    }

    fn advance(&mut self) -> &Token {
        if self.current >= self.tokens.len() {
            return self.tokens.last().unwrap();
        }
        let tok = &self.tokens[self.current];
        self.current += 1;
        tok
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

fn drain_ops(ops: &mut Vec<Expr>) -> Option<Expr> {
    match ops.len() {
        0 => None,
        1 => ops.pop(),
        _ => {
            let seq = std::mem::take(ops);
            Some(Expr::Seq(seq))
        }
    }
}

fn pop_var_name(ops: &mut Vec<Expr>, location: Location) -> ParseResult<String> {
    match ops.pop() {
        Some(Expr::Variable { name }) => Ok(name),
        _ => Err(ParseError::new(
            "expected a variable name",
            location,
            "E204",
        )),
    }
}

/// Pull the `fallback` statement out of a `handle` body, returning the
/// remaining statements and the fallback value (if any).
fn extract_fallback(body: Vec<Stmt>) -> (Vec<Stmt>, Option<Expr>) {
    let mut fallback = None;
    let mut stmts = Vec::with_capacity(body.len());
    for stmt in body {
        match stmt {
            Stmt::Fallback { value } => {
                if fallback.is_none() {
                    fallback = value;
                }
            }
            other => stmts.push(other),
        }
    }
    (stmts, fallback)
}

fn pop_target(ops: &mut Vec<Expr>) -> Option<Expr> {
    let last = ops.last()?;
    if matches!(last, Expr::Variable { .. } | Expr::Member { .. }) {
        return ops.pop();
    }
    // Fall back to the first variable/field anywhere in the operand list.
    let index = ops
        .iter()
        .position(|e| matches!(e, Expr::Variable { .. } | Expr::Member { .. }))?;
    Some(ops.remove(index))
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

pub fn parse(tokens: Vec<Token>) -> ParseResult<Program> {
    Parser::new(tokens).parse()
}
