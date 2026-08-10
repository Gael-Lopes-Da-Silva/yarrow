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
    Break,
    Continue,
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

    Fallback,

    True,
    False,

    Eof,
}
