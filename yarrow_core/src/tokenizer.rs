use crate::token::Token;

pub struct Tokenizer {
    source: String,
    tokens: Vec<Token>,
    cursor: u32,
}

impl Tokenizer {
    pub fn new(source: String) -> Self {
        Self {
            source,
            tokens: vec![],
            cursor: 0,
        }
    }

    pub fn get_next_token(&self) -> Option<Token> {
        if self.has_next_token() {
            return None;
        }

        let string: String = self.source;

        return None;
    }

    pub fn get_token(self) -> Vec<Token> {
        return self.tokens;
    }

    pub fn has_next_token(&self) -> bool {
        return self.cursor < self.source.len() as u32;
    }
}
