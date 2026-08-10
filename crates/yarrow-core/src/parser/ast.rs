//! Abstract syntax tree for the Yarrow language.
//!
//! Yarrow is a stack-based language: values are pushed onto a stack and
//! operators/words pop operands. Because there are no newline tokens, the
//! parser works on an *operand stack* and emits one statement per region of
//! the stack. See the parser docs for details.

use crate::tokenizer::token::Location;

#[derive(Debug, Clone, PartialEq)]
pub struct Program {
    pub items: Vec<Stmt>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mutability {
    Mutable,
    Const,
    Static,
}

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TypeKind {
    Named(String),
    Primitive(Primitive),
    Array {
        element: Box<Type>,
        size: Option<u64>,
    },
    List {
        element: Box<Type>,
    },
    Hashmap {
        key: Box<Type>,
        value: Box<Type>,
    },
    Reference {
        inner: Box<Type>,
    },
    Pointer {
        inner: Box<Type>,
    },
    Union(Vec<Type>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Primitive {
    I8,
    I16,
    I32,
    I64,
    I128,
    U8,
    U16,
    U32,
    U64,
    U128,
    F16,
    F32,
    F64,
    F128,
    String,
    Rune,
    Bool,
    Void,
    Error,
    Type,
}

impl Primitive {
    pub fn parse_name(name: &str) -> Option<Self> {
        Some(match name {
            "i8" => Primitive::I8,
            "i16" => Primitive::I16,
            "i32" => Primitive::I32,
            "i64" => Primitive::I64,
            "i128" => Primitive::I128,
            "u8" => Primitive::U8,
            "u16" => Primitive::U16,
            "u32" => Primitive::U32,
            "u64" => Primitive::U64,
            "u128" => Primitive::U128,
            "f16" => Primitive::F16,
            "f32" => Primitive::F32,
            "f64" => Primitive::F64,
            "f128" => Primitive::F128,
            "string" => Primitive::String,
            "rune" => Primitive::Rune,
            "bool" => Primitive::Bool,
            "void" => Primitive::Void,
            "error" => Primitive::Error,
            "type" => Primitive::Type,
            _ => return None,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Type {
    pub kind: TypeKind,
    pub location: Location,
}

// ---------------------------------------------------------------------------
// Expressions
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinOp {
    Plus,
    Minus,
    Mul,
    Div,
    Fdiv,
    Mod,
    Pow,
    Eq,
    Ne,
    Gt,
    Gte,
    Lt,
    Lte,
    And,
    Or,
    Xor,
    Lshift,
    Rshift,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnOp {
    Not,
}

/// Operators that manipulate the runtime stack directly. Used when the
/// operand stack does not contain enough local values to apply the operation
/// at parse time (e.g. `dup` in a function body whose parameters arrive at
/// runtime).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StackOp {
    Dup,
    Swap,
    Rot,
    Unrot,
    Pop,
    Drop,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    Integer {
        value: String,
    },
    Float {
        value: String,
    },
    String {
        value: String,
    },
    Rune {
        value: String,
    },
    Bool {
        value: bool,
    },
    Variable {
        name: String,
    },
    Member {
        base: Box<Expr>,
        member: String,
    },
    /// A type used as a value (e.g. the `i32` in `myVar typeof i32 ==`).
    TypeValue {
        name: String,
    },
    Binary {
        op: BinOp,
        left: Box<Expr>,
        right: Box<Expr>,
    },
    Unary {
        op: UnOp,
        operand: Box<Expr>,
    },
    Call {
        target: Box<Expr>,
    },
    Unwrap {
        inner: Box<Expr>,
    },
    /// `value typeof`: pops the value and pushes its static type.
    Typeof {
        inner: Box<Expr>,
    },
    /// `value borrow`: pushes a borrow reference to the value.
    Borrow {
        inner: Box<Expr>,
    },
    /// A binary operator whose operands come from the runtime stack.
    ApplyBin(BinOp),
    /// An unary operator whose operand comes from the runtime stack.
    ApplyUn(UnOp),
    /// `typeof` applied to the top of the runtime stack.
    ApplyTypeof,
    /// `borrow` applied to the top of the runtime stack.
    ApplyBorrow,
    StackOp(StackOp),
    Array(Vec<Expr>),
    List(Vec<Expr>),
    Map(Vec<(Expr, Expr)>),
    Seq(Vec<Expr>),
}

impl Expr {
    pub fn variable(name: impl Into<String>) -> Expr {
        Expr::Variable { name: name.into() }
    }
}

// ---------------------------------------------------------------------------
// Statements
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Field {
    pub name: String,
    pub ty: Type,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Function {
    pub name: String,
    pub params: Vec<Type>,
    pub body: Vec<Stmt>,
    pub returns: Vec<Type>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StructDecl {
    pub name: String,
    pub fields: Vec<Field>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Implement {
    pub target: String,
    pub functions: Vec<Function>,
}

/// A single enum member: its name and optional explicit value (the raw integer
/// lexeme, kept lossless like other literals). Without a value, the member gets
/// the next implicit ordinal starting at 0.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnumMember {
    pub name: String,
    pub value: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnumDecl {
    pub name: String,
    pub members: Vec<EnumMember>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnionDecl {
    pub name: String,
    pub types: Vec<Type>,
}

/// The condition of a `match` case. Either a boolean expression evaluated
/// against the subject (value match) or a member type the union subject is
/// dispatched on (type match).
#[derive(Debug, Clone, PartialEq)]
pub enum MatchCaseKind {
    Condition(Expr),
    Type(Type),
}

#[derive(Debug, Clone, PartialEq)]
pub struct MatchCase {
    pub kind: MatchCaseKind,
    pub body: Vec<Stmt>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Stmt {
    Expr(Expr),
    VarDecl {
        name: String,
        mutability: Mutability,
        ty: Type,
        value: Option<Expr>,
    },
    Set {
        target: Expr,
        value: Option<Expr>,
    },
    Function(Function),
    Struct(StructDecl),
    Implement(Implement),
    Enum(EnumDecl),
    Union(UnionDecl),
    Require {
        path: String,
        alias: Option<String>,
    },
    If {
        condition: Expr,
        then_branch: Vec<Stmt>,
        else_branch: Vec<Stmt>,
    },
    /// `for` comes in three forms:
    /// - condition form (`counter 5 < for ... end`): `value`/`index` are `None`
    ///   and `source` is the loop condition;
    /// - value form (`numbers value for ... end`): `value` is the iteration
    ///   variable and `source` the iterable;
    /// - index form (`numbers value index for ... end`): both `value` and
    ///   `index` are bound and `source` is the iterable.
    For {
        source: Expr,
        value: Option<String>,
        index: Option<String>,
        body: Vec<Stmt>,
    },
    Match {
        value: Expr,
        cases: Vec<MatchCase>,
        else_branch: Vec<Stmt>,
    },
    Defer {
        body: Vec<Stmt>,
    },
    /// Error-handling block: `expression handle <body> end`. The body may
    /// contain a `fallback` statement, which is extracted into `fallback`.
    Handle {
        body: Vec<Stmt>,
        fallback: Option<Expr>,
    },
    /// `source target move`: transfers ownership of `source` to `target`.
    Move {
        target: String,
        source: Expr,
    },
    /// `value fallback`: the value pushed on the stack if a `handle` catches an
    /// error. Only meaningful inside a `handle ... end` block.
    Fallback {
        value: Option<Expr>,
    },
    Return {
        value: Option<Expr>,
    },
    Break,
    Continue,
}
