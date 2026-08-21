#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenKind {
    LeftParen,
    RightParen,
    LeftCurly,
    RightCurly,
    LeftSquare,
    RightSquare,
    LeftAngle,
    RightAngle,

    Plus,
    Minus,
    Asterisk,
    Slash,
    SlashSlash,
    Percent,
    Caret,
    /// String concatenation (`~`).
    Tilde,
    /// Union type literal delimiters (`|T U|`).
    Pipe,
    Dot,

    EqualEqual,
    NotEqual,
    Greater,
    GreaterEqual,
    Less,
    LessEqual,

    Identifier,
    String,
    Rune,
    Integer,
    Float,

    And,
    Or,
    Xor,
    Not,
    LeftShift,
    RightShift,

    Typeof,

    If,
    Else,
    For,
    Match,
    Case,

    Unwrap,
    Handle,

    Function,
    Return,
    Call,
    Do,
    With,
    End,

    Const,
    Static,
    Mutable,
    Set,
    Unsafe,

    Public,
    Private,
    Copy,
    Error,

    Struct,
    Implement,
    Enum,
    Union,

    Require,
    Defer,

    Pop,
    Drop,
    Dup,
    Rot,
    Unrot,
    Swap,

    Borrow,
    Move,

    Load,
    Store,

    Fallback,

    At,

    True,
    False,

    Eof,
}
