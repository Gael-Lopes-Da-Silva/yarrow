from utils.enums.tokens import Tokens


class Tokenizer:
    def __init__(self, source, path):
        self.source = source
        self.path = path
        self.start = (0, 0)
        self.current = (0, 0)
        self.line = 1
        self.tokens = []
        self.keywords = [
            ("and", Tokens.AND),
            ("or", Tokens.OR),
            ("xor", Tokens.XOR),
            ("not", Tokens.NOT),
            ("lshift", Tokens.LEFT_SHIFT),
            ("rshift", Tokens.RIGHT_SHIFT),
            ("if", Tokens.IF),
            ("else", Tokens.ELSE),
            ("while", Tokens.WHILE),
            ("for", Tokens.FOR),
            ("break", Tokens.BREAK),
            ("continue", Tokens.CONTINUE),
            ("match", Tokens.MATCH),
            ("case", Tokens.CASE),
            ("unwrap", Tokens.UNWRAP),
            ("handle", Tokens.HANDLE),
            ("function", Tokens.FUNCTION),
            ("return", Tokens.RETURN),
            ("call", Tokens.CALL),
            ("do", Tokens.DO),
            ("with", Tokens.WITH),
            ("const", Tokens.CONST),
            ("static", Tokens.STATIC),
            ("mutable", Tokens.MUTABLE),
            ("set", Tokens.SET),
            ("struct", Tokens.STRUCT),
            ("implement", Tokens.IMPLEMENT),
            ("enum", Tokens.ENUM),
            ("union", Tokens.UNION),
            ("pop", Tokens.POP),
            ("drop", Tokens.DROP),
            ("dup", Tokens.DUP),
            ("over", Tokens.OVER),
            ("rot", Tokens.ROT),
            ("swap", Tokens.SWAP),
            ("require", Tokens.REQUIRE),
            ("defer", Tokens.DEFER),
            ("end", Tokens.END),
            ("true", Tokens.BOOLEAN),
            ("false", Tokens.BOOLEAN),
        ].extend(
            (type_kind.name.lower(), Tokens.TYPE) for type_kind in enums.types.Types
        )

    def tokenize(self) -> list:
        while not self.eof():
            self.start = self.current
            self.tokenize_lexeme()

        return self.tokens

    def tokenize_lexeme(self) -> None:
        lexeme = self.advance()

        match lexeme:
            case " " | "\t":
                pass

            case "\n":
                self.line += 1
                self.current[1] = 0

            case "#":
                while not self.eof() and self.peek() != "\n":
                    self.advance()

            case "(":
                self.add_token(Tokens.LEFT_PAREN)
            case ")":
                self.add_token(Tokens.RIGHT_PAREN)
            case "{":
                self.add_token(Tokens.LEFT_CURLY)
            case "}":
                self.add_token(Tokens.RIGHT_CURLY)
            case "[":
                self.add_token(Tokens.LEFT_SQUARE)
            case "]":
                self.add_token(Tokens.RIGHT_SQUARE)
            case ":":
                self.add_token(Tokens.COLON)
            case ";":
                self.add_token(Tokens.SEMI_COLON)
            case ",":
                self.add_token(Tokens.COMMA)
            case ".":
                self.add_token(Tokens.DOT)
            case "?":
                self.add_token(Tokens.QUESTION)
            case "%":
                self.add_token(Tokens.PERCENT)
            case "&":
                self.add_token(Tokens.AMPERSAND)
            case "|":
                self.add_token(Tokens.BAR)
            case "*":
                self.add_token(Tokens.STAR)
            case "^":
                self.add_token(Tokens.CARET)

            case "/":
                self.add_token(
                    Tokens.SLASH_SLASH if self.match("/") else enums.tokens.Tokens.SLASH
                )
            case "=":
                self.add_token(
                    Tokens.EQUAL_EQUAL if self.match("=") else enums.tokens.Tokens.EQUAL
                )

            case "<":
                if self.match("="):
                    self.add_token(Tokens.LESS_EQUAL)
                else:
                    self.add_token(Tokens.LESS)

            case ">":
                if self.match("="):
                    self.add_token(Tokens.GREATER_EQUAL)
                else:
                    self.add_token(Tokens.GREATER)

            case "!":
                if self.match("="):
                    self.add_token(Tokens.NOT_EQUAL)

            case '"':
                self.handle_strings()

            case "'":
                self.handle_runes()

            case "-":
                if not self.eof() and self.peek().isdigit():
                    self.handle_numbers()
                else:
                    self.add_token(Tokens.MINUS)

            case "+":
                if not self.eof() and self.peek().isdigit():
                    self.handle_numbers()
                else:
                    self.add_token(Tokens.PLUS)

            case _ if lexeme.isdigit():
                self.handle_numbers()

            case _ if lexeme.isalpha() or lexeme in ["_", "@"]:
                self.handle_identifiers()

            case _:
                self.logger.warning(
                    f"Invalid symbol '{lexeme}'",
                    location=self.get_location(),
                )

    def handle_numbers(self) -> None:
        while not self.eof() and (self.peek().isdigit() or self.peek() == "_"):
            self.advance()

        if (
            not self.eof()
            and self.peek() == "."
            and (self.peek_next().isdigit() or self.peek_next() == "_")
        ):
            self.advance()

            while not self.eof() and (self.peek().isdigit() or self.peek() == "_"):
                self.advance()

            self.add_token(Tokens.FLOAT)
        else:
            self.add_token(Tokens.INTEGER)

    def handle_strings(self) -> None:
        while not self.eof() and self.peek() != '"':
            if self.peek() == "\n":
                self.logger.error(
                    "Unterminated string literal !",
                    location=self.get_location(),
                    location_message="close the string with the corresponding quotes",
                )
                raise GlobalException

            if self.match("\\"):
                if self.eof():
                    self.logger.error(
                        "Incomplete escape sequence in string literal !",
                        location=self.get_location(),
                        location_message="expected character after backslash",
                    )
                    raise GlobalException

                escape_rune = self.peek()
                if escape_rune in ["\\", "'", '"', "n", "r", "t", "v", "b", "a", "f"]:
                    self.advance()
                else:
                    self.logger.error(
                        f"Invalid escape sequence '\\{escape_rune}' in string literal !",
                        location=self.get_location(),
                        location_message="unknown escape sequence",
                    )
                    raise GlobalException
            else:
                self.advance()

        if self.eof() or not self.match('"'):
            self.logger.error(
                "Unterminated string literal !",
                location=self.get_location(),
                location_message="close the string with the corresponding quotes",
            )
            raise GlobalException

        self.add_token(Tokens.STRING)

    def handle_runes(self) -> None:
        while not self.eof() and self.peek() != "'":
            if self.peek() == "\n":
                self.logger.error(
                    "Unterminated rune literal !",
                    location=self.get_location(),
                    location_message="close the rune with the corresponding quote",
                )
                raise GlobalException

            if self.peek() == "\\":
                self.advance()

                if self.eof():
                    self.logger.error(
                        "Incomplete escape sequence in rune literal !",
                        location=self.get_location(),
                        location_message="expected character after backslash",
                    )
                    raise GlobalException

                escape_rune = self.peek()
                if escape_rune in ["\\", "'", '"', "n", "r", "t", "v", "b", "a", "f"]:
                    self.advance()
                else:
                    self.logger.error(
                        f"Invalid escape sequence '\\{escape_rune}' in rune literal !",
                        location=self.get_location(),
                        location_message="unknown escape sequence",
                    )
                    raise GlobalException
            else:
                self.advance()

        if self.eof() or not self.match("'"):
            self.logger.error(
                "Unterminated rune literal !",
                location=self.get_location(),
                location_message="close the rune with the corresponding quote",
            )
            raise GlobalException

        content = self.source[self.start + 1 : self.current - 1]
        if len(content.replace("\\", "")) > 1:
            self.logger.error(
                "Invalid rune syntax !",
                location=self.get_location(),
                location_message="there should only be one character in a rune",
            )
            raise GlobalException

        self.add_token(Tokens.RUNE)

    def handle_identifiers(self) -> None:
        while not self.eof() and (self.peek().isalnum() or self.peek() == "_"):
            self.advance()

        text = self.source[self.start : self.current]
        token_type = self.get_keyword(text.lower()) or Tokens.IDENTIFIER
        self.add_token(token_type)

    def get_keyword(self, key):
        for keyword in self.keywords:
            if keyword[0] == key:
                return keyword[1]
        return None

    def add_token(self, token_kind):
        self.tokens.append(
            Token(token_kind, self.source[self.start[0] : self.current[0]], self.get_location())
        )

    def get_location(self):
        return (self.line, self.start[1], self.current[1])

    def eof(self):
        return self.current[0] >= len(self.source)

    def peek(self):
        return self.source[self.current[0]]

    def peek_next(self):
        return self.source[self.current[0] + 1]

    def advance(self):
        lexeme = self.peek()
        self.current[0] += 1
        self.current[1] += 1
        return lexeme

    def match(self, expected):
        if self.eof() or self.peek() != expected:
            return False

        self.advance()
        return True
