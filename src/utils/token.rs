use crate::utils::token_kind::TokenKind;

#[derive(Debug)]
pub struct Token {
    pub kind: TokenKind,
    pub lexeme: String,
    pub location: [usize; 3],
}

impl Token {
    pub fn new(kind: TokenKind, lexeme: String, location: [usize; 3]) -> Self {
        Token {
            kind,
            lexeme,
            location,
        }
    }
}
