use crate::tokenizer::token::Location;
use crate::tokenizer::token::Token;
use crate::tokenizer::token::TokenizeError;
use crate::tokenizer::token_kind::TokenKind;

use std::collections::HashMap;

pub struct Tokenizer {
    source: String,
    start: usize,
    current: usize,
    line: usize,
    line_start: usize,
    tokens: Vec<Token>,
    keywords: HashMap<String, TokenKind>,
}

impl Tokenizer {
    pub fn new(source: String) -> Self {
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
        keywords.insert("default".to_string(), TokenKind::Default);
        keywords.insert("unwrap".to_string(), TokenKind::Unwrap);
        keywords.insert("handle".to_string(), TokenKind::Handle);
        keywords.insert("function".to_string(), TokenKind::Function);
        keywords.insert("return".to_string(), TokenKind::Return);
        keywords.insert("call".to_string(), TokenKind::Call);
        keywords.insert("do".to_string(), TokenKind::Do);
        keywords.insert("with".to_string(), TokenKind::With);
        keywords.insert("end".to_string(), TokenKind::End);
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
        keywords.insert("as".to_string(), TokenKind::As);
        keywords.insert("defer".to_string(), TokenKind::Defer);
        keywords.insert("true".to_string(), TokenKind::True);
        keywords.insert("false".to_string(), TokenKind::False);

        Self {
            source,
            start: 0,
            current: 0,
            line: 1,
            line_start: 0,
            tokens: Vec::new(),
            keywords,
        }
    }

    pub fn tokenize(&mut self) -> Result<Vec<Token>, TokenizeError> {
        while !self.is_at_end() {
            self.start = self.current;
            self.scan_token()?;
        }

        let location = self.get_location_at(self.current);
        self.tokens.push(Token::eof(location));

        Ok(std::mem::take(&mut self.tokens))
    }

    fn scan_token(&mut self) -> Result<(), TokenizeError> {
        let c = self.advance();

        match c {
            ' ' | '\t' | '\r' => {}
            '\n' => {
                self.line += 1;
                self.line_start = self.current;
            }
            '#' => self.skip_comment(),
            '(' => self.add_token(TokenKind::LeftParen),
            ')' => self.add_token(TokenKind::RightParen),
            '{' => self.add_token(TokenKind::LeftCurly),
            '}' => self.add_token(TokenKind::RightCurly),
            '[' => self.add_token(TokenKind::LeftSquare),
            ']' => self.add_token(TokenKind::RightSquare),
            ':' => self.add_token(TokenKind::Colon),
            ';' => self.add_token(TokenKind::SemiColon),
            ',' => self.add_token(TokenKind::Comma),
            '?' => self.add_token(TokenKind::Question),
            '%' => self.add_token(TokenKind::Percent),
            '&' => self.add_token(TokenKind::Ampersand),
            '|' => self.add_token(TokenKind::Bar),
            '*' => self.add_token(TokenKind::Asterisk),
            '^' => self.add_token(TokenKind::Caret),
            '@' => self.add_token(TokenKind::At),
            '/' => {
                if self.match_char('/') {
                    self.add_token(TokenKind::SlashSlash);
                } else {
                    self.add_token(TokenKind::Slash);
                }
            }
            '.' => {
                if self.match_char('.') {
                    self.add_token(TokenKind::DotDot);
                } else {
                    self.add_token(TokenKind::Dot);
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
            '-' => {
                if self.peek().is_ascii_digit() {
                    self.handle_number()?;
                } else if self.match_char('>') {
                    self.add_token(TokenKind::Arrow);
                } else {
                    self.add_token(TokenKind::Minus);
                }
            }
            '+' => {
                if self.peek().is_ascii_digit() {
                    self.handle_number()?;
                } else {
                    self.add_token(TokenKind::Plus);
                }
            }
            c if c.is_ascii_digit() => self.handle_number()?,
            '"' => self.handle_string()?,
            '\'' => self.handle_rune()?,
            c if c.is_ascii_alphabetic() || c == '_' => self.handle_identifier(),
            _ => {
                return Err(TokenizeError::new(
                    format!("unexpected character '{c}'"),
                    self.get_location(),
                    "E100".to_string(),
                ));
            }
        }

        Ok(())
    }

    fn handle_number(&mut self) -> Result<(), TokenizeError> {
        while self.peek().is_ascii_digit() || self.peek() == '_' {
            self.advance();
        }

        if self.peek() == '.' && self.peek_next().is_ascii_digit() {
            self.advance();
            while self.peek().is_ascii_digit() || self.peek() == '_' {
                self.advance();
            }
            self.add_token(TokenKind::Float);
        } else {
            self.add_token(TokenKind::Integer);
        }

        Ok(())
    }

    fn handle_string(&mut self) -> Result<(), TokenizeError> {
        while !self.is_at_end() && self.peek() != '"' {
            if self.peek() == '\n' {
                return Err(TokenizeError::new(
                    "unterminated string literal: newlines are not allowed inside strings"
                        .to_string(),
                    self.get_location(),
                    "E120".to_string(),
                ));
            }

            if self.peek() == '\\' {
                self.advance();
                if self.is_at_end() {
                    return Err(TokenizeError::new(
                        "invalid escape sequence".to_string(),
                        self.get_location(),
                        "E121".to_string(),
                    ));
                }

                let escape = self.peek();
                if !['\\', '"', '\'', 'n', 'r', 't', 'v', 'b', 'a', 'f'].contains(&escape) {
                    return Err(TokenizeError::new(
                        format!("invalid escape sequence '\\{escape}'"),
                        self.get_location(),
                        "E121".to_string(),
                    ));
                }
                self.advance();
            } else {
                self.advance();
            }
        }

        if self.is_at_end() {
            return Err(TokenizeError::new(
                "unterminated string literal".to_string(),
                self.get_location(),
                "E122".to_string(),
            ));
        }

        self.advance();
        self.add_token(TokenKind::String);
        Ok(())
    }

    fn handle_rune(&mut self) -> Result<(), TokenizeError> {
        while !self.is_at_end() && self.peek() != '\'' {
            if self.peek() == '\n' {
                return Err(TokenizeError::new(
                    "unterminated rune literal: newlines are not allowed inside runes".to_string(),
                    self.get_location(),
                    "E130".to_string(),
                ));
            }

            if self.peek() == '\\' {
                self.advance();
                if self.is_at_end() {
                    return Err(TokenizeError::new(
                        "invalid escape sequence".to_string(),
                        self.get_location(),
                        "E131".to_string(),
                    ));
                }

                let escape = self.peek();
                if !['\\', '\'', '"', 'n', 'r', 't', 'v', 'b', 'a', 'f'].contains(&escape) {
                    return Err(TokenizeError::new(
                        format!("invalid escape sequence '\\{escape}'"),
                        self.get_location(),
                        "E131".to_string(),
                    ));
                }
                self.advance();
            } else {
                self.advance();
            }
        }

        if self.is_at_end() {
            return Err(TokenizeError::new(
                "unterminated rune literal".to_string(),
                self.get_location(),
                "E132".to_string(),
            ));
        }

        self.advance();

        let content = &self.source[self.start + 1..self.current - 1];
        if content.replace('\\', "").len() > 1 {
            return Err(TokenizeError::new(
                "rune literals must contain exactly one character or escape sequence".to_string(),
                self.get_location(),
                "E133".to_string(),
            ));
        }

        self.add_token(TokenKind::Rune);
        Ok(())
    }

    fn handle_identifier(&mut self) {
        while self.peek().is_ascii_alphanumeric() || self.peek() == '_' || self.peek() == '@' {
            self.advance();
        }

        let text = self.lexeme();
        let kind = self
            .keywords
            .get(text.as_str())
            .copied()
            .unwrap_or(TokenKind::Identifier);
        self.add_token(kind);
    }

    fn skip_comment(&mut self) {
        while !self.is_at_end() && self.peek() != '\n' {
            self.advance();
        }
    }

    fn add_token(&mut self, kind: TokenKind) {
        let lexeme = self.lexeme();
        let location = self.get_location_at(self.start);
        self.tokens.push(Token::new(kind, lexeme, location));
    }

    fn match_char(&mut self, expected: char) -> bool {
        if self.is_at_end() || self.peek() != expected {
            return false;
        }
        self.advance();
        true
    }

    fn advance(&mut self) -> char {
        let c = self.peek();
        self.current += 1;
        c
    }

    fn is_at_end(&self) -> bool {
        self.current >= self.source.len()
    }

    fn peek(&self) -> char {
        self.source.chars().nth(self.current).unwrap_or('\0')
    }

    fn peek_next(&self) -> char {
        self.source.chars().nth(self.current + 1).unwrap_or('\0')
    }

    fn lexeme(&self) -> String {
        self.source[self.start..self.current].to_string()
    }

    fn get_location_at(&self, offset: usize) -> Location {
        Location::new(
            self.line,
            offset.saturating_sub(self.line_start) + 1,
            offset,
        )
    }

    fn get_location(&self) -> Location {
        self.get_location_at(self.current)
    }
}
