from utils.enums.tokens import Tokens
from utils.enums.types import Types
from utils.token import Token


class Tokenizer:
    def __init__(self):
        self.source = ""
        self.start = [0, 0]
        self.current = [0, 0]
        self.line = 1
        self.tokens = []
        self.keywords = {
            "and": Tokens.AND,
            "or": Tokens.OR,
            "xor": Tokens.XOR,
            "not": Tokens.NOT,
            "lshift": Tokens.LEFT_SHIFT,
            "rshift": Tokens.RIGHT_SHIFT,
            "if": Tokens.IF,
            "else": Tokens.ELSE,
            "while": Tokens.WHILE,
            "for": Tokens.FOR,
            "break": Tokens.BREAK,
            "continue": Tokens.CONTINUE,
            "match": Tokens.MATCH,
            "case": Tokens.CASE,
            "unwrap": Tokens.UNWRAP,
            "handle": Tokens.HANDLE,
            "function": Tokens.FUNCTION,
            "return": Tokens.RETURN,
            "call": Tokens.CALL,
            "do": Tokens.DO,
            "with": Tokens.WITH,
            "const": Tokens.CONST,
            "static": Tokens.STATIC,
            "mutable": Tokens.MUTABLE,
            "set": Tokens.SET,
            "struct": Tokens.STRUCT,
            "implement": Tokens.IMPLEMENT,
            "enum": Tokens.ENUM,
            "union": Tokens.UNION,
            "pop": Tokens.POP,
            "drop": Tokens.DROP,
            "dup": Tokens.DUP,
            "over": Tokens.OVER,
            "rot": Tokens.ROT,
            "swap": Tokens.SWAP,
            "require": Tokens.REQUIRE,
            "defer": Tokens.DEFER,
            "end": Tokens.END,
            "true": Tokens.BOOLEAN,
            "false": Tokens.BOOLEAN,
        }
        self.keywords.update({
            type_kind.name.lower(): Tokens.TYPE for type_kind in Types
        })

    def tokenize(self, source):
        self.source = source

        while not self.__eof():
            self.start[0] = self.current[0]
            self.start[1] = self.current[1]
            self.__tokenize_lexeme()

        return self.tokens

    def __tokenize_lexeme(self):
        lexeme = self.__advance()

        match lexeme:
            case " " | "\t":
                pass

            case "\n":
                self.line += 1
                self.current[1] = 0

            case "#":
                while not self.__eof() and self.__peek() != "\n":
                    self.__advance()

            case "(":
                self.__add_token(Tokens.LEFT_PAREN)
            case ")":
                self.__add_token(Tokens.RIGHT_PAREN)
            case "{":
                self.__add_token(Tokens.LEFT_CURLY)
            case "}":
                self.__add_token(Tokens.RIGHT_CURLY)
            case "[":
                self.__add_token(Tokens.LEFT_SQUARE)
            case "]":
                self.__add_token(Tokens.RIGHT_SQUARE)
            case ":":
                self.__add_token(Tokens.COLON)
            case ";":
                self.__add_token(Tokens.SEMI_COLON)
            case ",":
                self.__add_token(Tokens.COMMA)
            case ".":
                self.__add_token(Tokens.DOT)
            case "?":
                self.__add_token(Tokens.QUESTION)
            case "%":
                self.__add_token(Tokens.PERCENT)
            case "&":
                self.__add_token(Tokens.AMPERSAND)
            case "|":
                self.__add_token(Tokens.BAR)
            case "*":
                self.__add_token(Tokens.ASTERISK)
            case "^":
                self.__add_token(Tokens.CARET)

            case "/":
                self.__add_token(
                    Tokens.SLASH_SLASH if self.__match("/") else Tokens.SLASH
                )
            case "=":
                self.__add_token(
                    Tokens.EQUAL_EQUAL if self.__match("=") else Tokens.EQUAL
                )
            case "<":
                self.__add_token(
                    Tokens.LESS_EQUAL if self.__match("=") else Tokens.LESS
                )
            case ">":
                self.__add_token(
                    Tokens.GREATER_EQUAL if self.__match("=") else Tokens.GREATER
                )
            case "!":
                self.__add_token(
                    Tokens.NOT_EQUAL if self.__match("=") else Tokens.EXCLAMATION
                )

            case '"':
                self.__handle_strings()

            case "'":
                self.__handle_runes()

            case "-":
                if not self.__eof() and self.__peek().isdigit():
                    self.__handle_numbers()
                else:
                    self.__add_token(Tokens.MINUS)

            case "+":
                if not self.__eof() and self.__peek().isdigit():
                    self.__handle_numbers()
                else:
                    self.__add_token(Tokens.PLUS)

            case _ if lexeme.isdigit():
                self.__handle_numbers()

            case _ if lexeme.isalpha() or lexeme in ["_", "@"]:
                self.__handle_identifiers()

            case _:
                # FIXME: add warning
                pass

    def __handle_numbers(self):
        while not self.__eof() and (self.__peek().isdigit() or self.__peek() in ["_", ","]):
            self.__advance()

        if (
            not self.__eof()
            and self.__peek() == "."
            and (self.__peek_next().isdigit() or self.__peek_next() in ["_", ","])
        ):
            self.__advance()

            while not self.__eof() and (self.__peek().isdigit() or self.__peek() in ["_", ","]):
                self.__advance()

            self.__add_token(Tokens.FLOAT)
        else:
            self.__add_token(Tokens.INTEGER)

    def __handle_strings(self):
        while not self.__eof() and self.__peek() != '"':
            if self.__peek() == "\n":
                # FIXME: add error
                pass

            if self.__match("\\"):
                if self.__eof():
                    # FIXME: add error
                    pass

                escape_rune = self.__peek()
                if escape_rune in ["\\", "'", '"', "n", "r", "t", "v", "b", "a", "f"]:
                    self.__advance()
                else:
                    # FIXME: add error
                    pass
            else:
                self.__advance()

        if self.__eof() or not self.__match('"'):
            # FIXME: add error
            pass

        self.__add_token(Tokens.STRING)

    def __handle_runes(self):
        while not self.__eof() and self.__peek() != "'":
            if self.__peek() == "\n":
                # FIXME: add error
                pass

            if self.__peek() == "\\":
                self.__advance()

                if self.__eof():
                    # FIXME: add error
                    pass

                escape_rune = self.__peek()
                if escape_rune in ["\\", "'", '"', "n", "r", "t", "v", "b", "a", "f"]:
                    self.__advance()
                else:
                    # FIXME: add error
                    pass
            else:
                self.__advance()

        if self.__eof() or not self.__match("'"):
            # FIXME: add error
            pass

        content = self.source[self.start + 1 : self.current - 1]
        if len(content.replace("\\", "")) > 1:
            # FIXME: add error
            pass

        self.__add_token(Tokens.RUNE)

    def __handle_identifiers(self):
        while not self.__eof() and (self.__peek().isalnum() or self.__peek() == "_"):
            self.__advance()

        text = self.source[self.start[0] : self.current[0]]
        token_type = self.keywords.get(text.lower()) or Tokens.IDENTIFIER
        self.__add_token(token_type)

    def __add_token(self, token_kind):
        self.tokens.append(
            Token(
                token_kind,
                self.source[self.start[0] : self.current[0]],
                self.__get_location(),
            )
        )

    def __advance(self):
        lexeme = self.__peek()
        self.current[0] += 1
        self.current[1] += 1
        return lexeme

    def __match(self, expected):
        if self.__eof() or self.__peek() != expected:
            return False
        self.__advance()
        return True

    def __get_location(self):
        return (self.line, self.start[1], self.current[1])

    def __eof(self):
        return self.current[0] >= len(self.source)

    def __peek(self):
        return self.source[self.current[0]]

    def __peek_next(self):
        return self.source[self.current[0] + 1]
