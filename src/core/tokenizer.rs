use std::collections::HashMap;

use crate::utils::token::Token;
use crate::utils::token_kind::TokenKind;

#[derive(Debug)]
pub struct Tokenizer {
    source: String,
    start: [usize; 2],
    current: [usize; 2],
    line: usize,
    tokens: Vec<Token>,
    keywords: HashMap<String, TokenKind>,
}

impl Tokenizer {
    pub fn new() -> Self {
        let mut keywords = HashMap::new();
        keywords.insert("and".to_string(), TokenKind::And);
        keywords.insert("or".to_string(), TokenKind::Or);
        keywords.insert("xor".to_string(), TokenKind::Xor);
        keywords.insert("not".to_string(), TokenKind::Not);
        keywords.insert("lshift".to_string(), TokenKind::LeftShift);
        keywords.insert("rshift".to_string(), TokenKind::RightShift);
        keywords.insert("if".to_string(), TokenKind::If);
        keywords.insert("else".to_string(), TokenKind::Else);
        keywords.insert("while".to_string(), TokenKind::While);
        keywords.insert("for".to_string(), TokenKind::For);
        keywords.insert("break".to_string(), TokenKind::Break);
        keywords.insert("continue".to_string(), TokenKind::Continue);
        keywords.insert("match".to_string(), TokenKind::Match);
        keywords.insert("case".to_string(), TokenKind::Case);
        keywords.insert("unwrap".to_string(), TokenKind::Unwrap);
        keywords.insert("handle".to_string(), TokenKind::Handle);
        keywords.insert("function".to_string(), TokenKind::Function);
        keywords.insert("return".to_string(), TokenKind::Return);
        keywords.insert("call".to_string(), TokenKind::Call);
        keywords.insert("do".to_string(), TokenKind::Do);
        keywords.insert("with".to_string(), TokenKind::With);
        keywords.insert("const".to_string(), TokenKind::Const);
        keywords.insert("static".to_string(), TokenKind::Static);
        keywords.insert("mutable".to_string(), TokenKind::Mutable);
        keywords.insert("set".to_string(), TokenKind::Set);
        keywords.insert("struct".to_string(), TokenKind::Struct);
        keywords.insert("implement".to_string(), TokenKind::Implement);
        keywords.insert("enum".to_string(), TokenKind::Enum);
        keywords.insert("union".to_string(), TokenKind::Union);
        keywords.insert("pop".to_string(), TokenKind::Pop);
        keywords.insert("drop".to_string(), TokenKind::Drop);
        keywords.insert("dup".to_string(), TokenKind::Dup);
        keywords.insert("over".to_string(), TokenKind::Over);
        keywords.insert("rot".to_string(), TokenKind::Rot);
        keywords.insert("swap".to_string(), TokenKind::Swap);
        keywords.insert("require".to_string(), TokenKind::Require);
        keywords.insert("defer".to_string(), TokenKind::Defer);
        keywords.insert("end".to_string(), TokenKind::End);
        keywords.insert("true".to_string(), TokenKind::Boolean);
        keywords.insert("false".to_string(), TokenKind::Boolean);

        return Tokenizer {
            source: String::new(),
            start: [0, 0],
            current: [0, 0],
            line: 1,
            tokens: Vec::new(),
            keywords,
        };
    }

    pub fn tokenize(&mut self, source: String) -> &Vec<Token> {
        self.source = source;
        self.tokens.clear();

        while !self.eof() {
            self.start = self.current;

            let lexeme = self.advance();

            match lexeme {
                ' ' | '\t' => {}
                '\n' => {
                    self.line += 1;
                    self.current[1] = 0;
                }
                '#' => {
                    while !self.eof() && self.peek() != '\n' {
                        self.advance();
                    }
                }
                '(' => self.add_token(TokenKind::LeftParen),
                ')' => self.add_token(TokenKind::RightParen),
                '{' => self.add_token(TokenKind::LeftCurly),
                '}' => self.add_token(TokenKind::RightCurly),
                '[' => self.add_token(TokenKind::LeftSquare),
                ']' => self.add_token(TokenKind::RightSquare),
                ':' => self.add_token(TokenKind::Colon),
                ';' => self.add_token(TokenKind::SemiColon),
                ',' => self.add_token(TokenKind::Comma),
                '.' => self.add_token(TokenKind::Dot),
                '?' => self.add_token(TokenKind::Question),
                '%' => self.add_token(TokenKind::Percent),
                '&' => self.add_token(TokenKind::Ampersand),
                '|' => self.add_token(TokenKind::Bar),
                '*' => self.add_token(TokenKind::Asterisk),
                '^' => self.add_token(TokenKind::Caret),
                '/' => {
                    if self.match_char('/') {
                        self.add_token(TokenKind::SlashSlash);
                    } else {
                        self.add_token(TokenKind::Slash);
                    }
                }
                '=' => {
                    if self.match_char('=') {
                        self.add_token(TokenKind::EqualEqual);
                    } else {
                        self.add_token(TokenKind::Equal);
                    }
                }
                '<' => {
                    if self.match_char('=') {
                        self.add_token(TokenKind::LessEqual);
                    } else {
                        self.add_token(TokenKind::Less);
                    }
                }
                '>' => {
                    if self.match_char('=') {
                        self.add_token(TokenKind::GreaterEqual);
                    } else {
                        self.add_token(TokenKind::Greater);
                    }
                }
                '!' => {
                    if self.match_char('=') {
                        self.add_token(TokenKind::NotEqual);
                    } else {
                        self.add_token(TokenKind::Exclamation);
                    }
                }
                '"' => self.handle_strings(),
                '\'' => self.handle_runes(),
                '-' => {
                    if !self.eof() && self.peek().is_ascii_digit() {
                        self.handle_numbers();
                    } else {
                        self.add_token(TokenKind::Minus);
                    }
                }
                '+' => {
                    if !self.eof() && self.peek().is_ascii_digit() {
                        self.handle_numbers();
                    } else {
                        self.add_token(TokenKind::Plus);
                    }
                }
                c if c.is_ascii_digit() => self.handle_numbers(),
                c if c.is_alphabetic() || c == '_' || c == '@' => self.handle_identifiers(),
                _ => {
                    self.log.log(
                        "warning",
                        "Unsupported symbol",
                        self.get_location(),
                        "W001",
                        None,
                    );
                }
            }
        }

        return &self.tokens;
    }

    fn handle_numbers(&mut self) {
        while !self.eof()
            && (self.peek().is_ascii_digit() || self.peek() == '_' || self.peek() == ',')
        {
            self.advance();
        }

        if !self.eof()
            && self.peek() == '.'
            && (self.peek_next().is_ascii_digit()
                || self.peek_next() == '_'
                || self.peek_next() == ',')
        {
            self.advance();
            while !self.eof()
                && (self.peek().is_ascii_digit() || self.peek() == '_' || self.peek() == ',')
            {
                self.advance();
            }
            self.add_token(Tokens::FLOAT);
        } else {
            self.add_token(Tokens::INTEGER);
        }
    }

    fn handle_strings(&mut self) {
        while !self.eof() && self.peek() != '"' {
            if self.peek() == '\n' {
                self.log.log(
                    "error",
                    "Invalid string syntax",
                    self.get_location(),
                    "E120",
                    Some("new lines are not supported inside strings"),
                );
                std::process::exit(120);
            }

            if self.match_char('\\') {
                if self.eof() {
                    let mut loc = self.get_location();
                    loc[1] = loc[1].saturating_sub(1);
                    loc[2] = loc[2].saturating_sub(1);
                    self.log.log(
                        "error",
                        "Invalid string syntax",
                        loc,
                        "E121",
                        Some("escape symbols should be followed by a valid letter"),
                    );
                    std::process::exit(121);
                }

                let escape_rune = self.peek();
                if ['\\', '"', '\'', 'n', 'r', 't', 'v', 'b', 'a', 'f'].contains(&escape_rune) {
                    self.advance();
                } else {
                    let mut loc = self.get_location();
                    loc[1] = loc[1].saturating_sub(1);
                    self.log.log(
                        "error",
                        "Invalid string syntax",
                        loc,
                        "E121",
                        Some("escape symbols should be followed by a valid letter"),
                    );
                    std::process::exit(121);
                }
            } else {
                self.advance();
            }
        }

        if self.eof() || !self.match_char('"') {
            self.log.log(
                "error",
                "Invalid string syntax",
                self.get_location(),
                "E122",
                Some("string literals need to be closed with a corresponding quote"),
            );
            std::process::exit(122);
        }

        self.add_token(Tokens::STRING);
    }

    fn handle_runes(&mut self) {
        while !self.eof() && self.peek() != '\'' {
            if self.peek() == '\n' {
                self.log.log(
                    "error",
                    "Invalid rune syntax",
                    self.get_location(),
                    "E130",
                    Some("new lines are not supported inside runes"),
                );
                std::process::exit(130);
            }

            if self.match_char('\\') {
                if self.eof() {
                    let mut loc = self.get_location();
                    loc[1] = loc[1].saturating_sub(1);
                    loc[2] = loc[2].saturating_sub(1);
                    self.log.log(
                        "error",
                        "Invalid rune syntax",
                        loc,
                        "E131",
                        Some("escape symbols should be followed by a valid letter"),
                    );
                    std::process::exit(131);
                }

                let escape_rune = self.peek();
                if ['\\', '\'', '"', 'n', 'r', 't', 'v', 'b', 'a', 'f'].contains(&escape_rune) {
                    self.advance();
                } else {
                    let mut loc = self.get_location();
                    loc[1] = loc[1].saturating_sub(1);
                    self.log.log(
                        "error",
                        "Invalid rune syntax",
                        loc,
                        "E131",
                        Some("escape symbols should be followed by a valid letter"),
                    );
                    std::process::exit(131);
                }
            } else {
                self.advance();
            }
        }

        if self.eof() || !self.match_char('\'') {
            self.log.log(
                "error",
                "Invalid rune syntax",
                self.get_location(),
                "E132",
                Some("rune literals need to be closed with a corresponding quote"),
            );
            std::process::exit(132);
        }

        let content = &self.source[self.start[0] + 1..self.current[0] - 1];
        if content.replace("\\", "").len() > 1 {
            self.log.log(
                "error",
                "Invalid rune syntax",
                self.get_location(),
                "E133",
                Some("runes should only contain a letter or an escape sequence"),
            );
            std::process::exit(133);
        }

        self.add_token(Tokens::RUNE);
    }

    fn handle_identifiers(&mut self) {
        while !self.eof()
            && (self.peek().is_alphanumeric() || self.peek() == '_' || self.peek() == '@')
        {
            self.advance();
        }

        let text = self.source[self.start[0]..self.current[0]].to_string();
        let token_type = self
            .keywords
            .get(&text.to_lowercase())
            .copied()
            .unwrap_or(Tokens::IDENTIFIER);
        self.add_token(token_type);
    }

    fn add_token(&mut self, token_kind: TokenKind) {
        let lexeme = self.source[self.start[0]..self.current[0]].to_string();
        self.tokens
            .push(Token::new(token_kind, lexeme, self.get_location()));
    }

    fn advance(&mut self) -> char {
        let lexeme = self.peek();
        self.current[0] += 1;
        self.current[1] += 1;
        return lexeme;
    }

    fn match_char(&mut self, expected: char) -> bool {
        if self.eof() || self.peek() != expected {
            return false;
        }

        self.advance();
        return true;
    }

    fn get_location(&self) -> [usize; 3] {
        return [self.line, self.start[1], self.current[1]];
    }

    fn eof(&self) -> bool {
        return self.current[0] >= self.source.len();
    }

    fn peek(&self) -> char {
        return self.source.chars().nth(self.current[0]).unwrap_or('\0');
    }

    fn peek_next(&self) -> char {
        return self.source.chars().nth(self.current[0] + 1).unwrap_or('\0');
    }
}
