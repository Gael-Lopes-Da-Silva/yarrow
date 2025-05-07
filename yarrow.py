import os
import sys
import enum
import math


# ERRORS
class GlobalException(Exception):
    pass


# ENUMS
class TokenKind(enum.Enum):
    LEFT_PAREN = 1
    RIGHT_PAREN = 2
    LEFT_CURLY = 3
    RIGHT_CURLY = 4
    LEFT_SQUARE = 5
    RIGHT_SQUARE = 6

    PLUS = 7
    MINUS = 8
    STAR = 9
    SLASH = 10
    SLASH_SLASH = 11
    PERCENT = 12
    CARET = 13
    DOT = 14
    QUESTION = 15

    EXCLAMATION = 16
    AMPERSAND = 17
    BAR = 18
    COLON = 19
    SEMI_COLON = 20
    COMMA = 21

    EQUAL = 22
    EQUAL_EQUAL = 23
    NOT_EQUAL = 24
    GREATER = 25
    GREATER_EQUAL = 26
    LESS = 27
    LESS_EQUAL = 28

    IDENTIFIER = 29
    STRING = 30
    RUNE = 31
    INTEGER = 32
    FLOAT = 33
    BOOLEAN = 34

    TYPE = 35

    AND = 36
    OR = 37
    XOR = 38
    NOT = 39
    LEFT_SHIFT = 40
    RIGHT_SHIFT = 41

    IF = 42
    ELSE = 43
    WHILE = 44
    BREAK = 45
    CONTINUE = 46
    MATCH = 47
    CASE = 48

    UNWRAP = 49
    HANDLE = 50

    FUNCTION = 51
    RETURN = 52
    CALL = 53
    DO = 54
    WITH = 55

    CONST = 56
    STATIC = 57
    MUTABLE = 58
    SET = 59

    STRUCT = 60
    IMPLEMENT = 61
    ENUM = 62
    UNION = 63
    NEW = 64

    POP = 65
    DROP = 66
    DUP = 67
    OVER = 68
    ROT = 69
    SWAP = 70

    REQUIRE = 71
    DEFER = 72
    END = 73


class TypeKind(enum.Enum):
    I8 = 1
    I16 = 2
    I32 = 3
    I64 = 4
    I128 = 5
    U8 = 6
    U16 = 7
    U32 = 8
    U64 = 9
    U128 = 10
    F16 = 11
    F32 = 12
    F64 = 13
    F128 = 14
    BOOL = 15
    VOID = 16
    ERROR = 17
    TYPE = 18
    STRING = 19
    RUNE = 20
    ARRAY = 21
    LIST = 22
    HASHMAP = 23
    STACK = 24
    QUEUE = 25
    POINTER = 26
    REFERENCE = 27
    USIZE = 28
    ISIZE = 29
    C_CHAR = 30
    C_SHORT = 31
    C_USHORT = 32
    C_INT = 33
    C_UINT = 34
    C_LONG = 35
    C_ULONG = 36
    C_LONGLONG = 37
    C_ULONGLONG = 38
    C_DOUBLE = 39
    C_LONGDOUBLE = 40


# CLASSES
class Style:
    RESET = "\033[0m"
    GREY = "\033[90m"
    RED = "\033[91m"
    GREEN = "\033[92m"
    YELLOW = "\033[93m"
    BLUE = "\033[94m"
    PURPLE = "\033[95m"
    CYAN = "\033[96m"
    WHITE = "\033[97m"
    BOLD = "\033[1m"
    DIM = "\033[2m"
    ITALIC = "\033[3m"
    UNDERLINE = "\033[4m"
    BLINK = "\033[5m"


class Location:
    def __init__(self, line: int, start: int, end: int) -> None:
        self.line = line
        self.start = start
        self.end = end

    def __repr__(self) -> str:
        return f"{self.line}:{self.start}:{self.end}"


class Token:
    def __init__(self, kind: TokenKind, lexeme: str, location: Location) -> None:
        self.kind = kind
        self.lexeme = lexeme
        self.location = location

    def __repr__(self) -> str:
        return f"{self.kind}:{self.lexeme}"


class Keyword:
    def __init__(self, name: str, kind: TokenKind) -> None:
        self.name = name
        self.kind = kind

    def __repr__(self) -> str:
        return f"{self.name}:{self.kind}"


class Instruction:
    def __init__(self, kind: str, content: any, token: Token) -> None:
        self.kind = kind
        self.content = content
        self.token = token

    def __repr__(self) -> str:
        return f"{self.kind}:{self.content}"


class Logger:
    def __init__(self, source: str = None, path: str = None) -> None:
        self.pointer = "─"
        self.source = source
        self.path = path

    def error(
        self,
        message: str,
        *args: any,
        location: Location = None,
        location_message: str = None,
    ) -> None:
        output = f"[{Style.BOLD}{Style.RED}ERROR{Style.RESET}] {message}"

        if location and self.source:
            line = f"{location.line}│ {self.source.splitlines()[location.line - 1]}"
            pointer = " " * (
                len(str(location.line)) + 2 + location.start
            ) + self.pointer * max(1, location.end - location.start)
            output += f"{Style.GREY}\n| location:{f' {self.path}:{location.line}:{location.start}' if self.path else ''}\n|   {line}\n|   {Style.RED}{pointer}"
            output += (
                f" {location_message}{Style.RESET}"
                if location_message
                else f"{Style.RESET}"
            )

        for arg in args:
            output += f"{Style.GREY}\n| {arg}{Style.RESET}"

        print(f"{output}")

    def warning(
        self,
        message: str,
        *args: any,
        location: Location = None,
        location_message: str = None,
    ) -> None:
        output = f"[{Style.BOLD}{Style.YELLOW}WARNING{Style.RESET}] {message}"

        if location and self.source:
            line = f"{location.line}│ {self.source.splitlines()[location.line - 1]}"
            pointer = " " * (
                len(str(location.line)) + 2 + location.start
            ) + self.pointer * max(1, location.end - location.start)
            output += f"{Style.GREY}\n| location:{f' {self.path}:{location.line}:{location.start}' if self.path else ''}\n|   {line}\n|   {Style.YELLOW}{pointer}"
            output += (
                f" {location_message}{Style.RESET}"
                if location_message
                else f"{Style.RESET}"
            )

        for arg in args:
            output += f"{Style.GREY}\n| {arg}{Style.RESET}"

        print(f"{output}")

    def info(self, message: str, *args: any) -> None:
        output = f"[{Style.BOLD}{Style.BLUE}INFO{Style.RESET}] {message}"

        for arg in args:
            output += f"{Style.GREY}\n| {arg}{Style.RESET}"

        print(f"{output}")

    def debug(self, message: str, *args: any) -> None:
        output = f"[{Style.BOLD}{Style.GREY}DEBUG{Style.RESET}] {message}"

        for arg in args:
            output += f"{Style.GREY}\n| {arg}{Style.RESET}"

        print(f"{output}")


class Tokenizer:
    def __init__(self, source: str, path: str) -> None:
        self.source = source
        self.path = path
        self.logger = Logger(source, path)
        self.start = 0
        self.start_offset = 0
        self.current = 0
        self.current_offset = 0
        self.line = 1
        self.tokens = []
        self.keywords = [
            Keyword("and", TokenKind.AND),
            Keyword("or", TokenKind.OR),
            Keyword("xor", TokenKind.XOR),
            Keyword("not", TokenKind.NOT),
            Keyword("lshift", TokenKind.LEFT_SHIFT),
            Keyword("rshift", TokenKind.RIGHT_SHIFT),
            Keyword("if", TokenKind.IF),
            Keyword("else", TokenKind.ELSE),
            Keyword("while", TokenKind.WHILE),
            Keyword("break", TokenKind.BREAK),
            Keyword("continue", TokenKind.CONTINUE),
            Keyword("match", TokenKind.MATCH),
            Keyword("case", TokenKind.CASE),
            Keyword("unwrap", TokenKind.UNWRAP),
            Keyword("handle", TokenKind.HANDLE),
            Keyword("function", TokenKind.FUNCTION),
            Keyword("return", TokenKind.RETURN),
            Keyword("call", TokenKind.CALL),
            Keyword("do", TokenKind.DO),
            Keyword("with", TokenKind.WITH),
            Keyword("const", TokenKind.CONST),
            Keyword("static", TokenKind.STATIC),
            Keyword("mutable", TokenKind.MUTABLE),
            Keyword("set", TokenKind.SET),
            Keyword("struct", TokenKind.STRUCT),
            Keyword("implement", TokenKind.IMPLEMENT),
            Keyword("enum", TokenKind.ENUM),
            Keyword("union", TokenKind.UNION),
            Keyword("new", TokenKind.NEW),
            Keyword("pop", TokenKind.POP),
            Keyword("drop", TokenKind.DROP),
            Keyword("dup", TokenKind.DUP),
            Keyword("over", TokenKind.OVER),
            Keyword("rot", TokenKind.ROT),
            Keyword("swap", TokenKind.SWAP),
            Keyword("require", TokenKind.REQUIRE),
            Keyword("defer", TokenKind.DEFER),
            Keyword("end", TokenKind.END),
            Keyword("true", TokenKind.BOOLEAN),
            Keyword("false", TokenKind.BOOLEAN),
        ]

        self.keywords.extend(
            Keyword(type_kind.name.lower(), TokenKind.TYPE) for type_kind in TypeKind
        )

    def __repr__(self) -> str:
        return f"{self.tokens}"

    def tokenize(self) -> list:
        while not self.eof():
            self.start = self.current
            self.start_offset = self.current_offset
            self.tokenize_lexeme()

        return self.tokens

    def tokenize_lexeme(self) -> None:
        rune = self.advance()

        match rune:
            case " " | "\t":
                pass

            case "\n":
                self.line += 1
                self.current_offset = 0

            case "#":
                while not self.eof() and self.peek() != "\n":
                    self.advance()

            case "(":
                self.add_token(TokenKind.LEFT_PAREN)
            case ")":
                self.add_token(TokenKind.RIGHT_PAREN)
            case "{":
                self.add_token(TokenKind.LEFT_CURLY)
            case "}":
                self.add_token(TokenKind.RIGHT_CURLY)
            case "[":
                self.add_token(TokenKind.LEFT_SQUARE)
            case "]":
                self.add_token(TokenKind.RIGHT_SQUARE)
            case ":":
                self.add_token(TokenKind.COLON)
            case ";":
                self.add_token(TokenKind.SEMI_COLON)
            case ",":
                self.add_token(TokenKind.COMMA)
            case ".":
                self.add_token(TokenKind.DOT)
            case "?":
                self.add_token(TokenKind.QUESTION)
            case "%":
                self.add_token(TokenKind.PERCENT)
            case "&":
                self.add_token(TokenKind.AMPERSAND)
            case "|":
                self.add_token(TokenKind.BAR)
            case "*":
                self.add_token(TokenKind.STAR)
            case "^":
                self.add_token(TokenKind.CARET)

            case "/":
                self.add_token(
                    TokenKind.SLASH_SLASH if self.match("/") else TokenKind.SLASH
                )
            case "=":
                self.add_token(
                    TokenKind.EQUAL_EQUAL if self.match("=") else TokenKind.EQUAL
                )

            case "<":
                if self.match("="):
                    self.add_token(TokenKind.LESS_EQUAL)
                else:
                    self.add_token(TokenKind.LESS)

            case ">":
                if self.match("="):
                    self.add_token(TokenKind.GREATER_EQUAL)
                else:
                    self.add_token(TokenKind.GREATER)

            case "!":
                if self.match("="):
                    self.add_token(TokenKind.NOT_EQUAL)

            case '"':
                self.handle_strings()

            case "'":
                self.handle_runes()

            case "-":
                if not self.eof() and self.peek().isdigit():
                    self.handle_numbers()
                else:
                    self.add_token(TokenKind.MINUS)

            case "+":
                if not self.eof() and self.peek().isdigit():
                    self.handle_numbers()
                else:
                    self.add_token(TokenKind.PLUS)

            case _ if rune.isdigit():
                self.handle_numbers()

            case _ if rune.isalpha() or rune in ["_", "@"]:
                self.handle_identifiers()

            case _:
                self.logger.warning(
                    f"Invalid symbol '{rune}'",
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

            self.add_token(TokenKind.FLOAT)
        else:
            self.add_token(TokenKind.INTEGER)

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
                if escape_rune in [
                    "\\",
                    "'",
                    '"',
                    "n",
                    "r",
                    "t",
                    "v",
                    "b",
                    "a",
                    "f",
                ]:
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

        self.add_token(TokenKind.STRING)

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

        self.add_token(TokenKind.RUNE)

    def handle_identifiers(self) -> None:
        while not self.eof() and (self.peek().isalnum() or self.peek() == "_"):
            self.advance()

        text = self.source[self.start : self.current]
        token_type = self.get_keyword(text.lower()) or TokenKind.IDENTIFIER
        self.add_token(token_type)

    def get_keyword(self, key: str) -> TokenKind | None:
        for keyword in self.keywords:
            if keyword.name == key:
                return keyword.kind
        return None

    def add_token(self, kind: TokenKind) -> None:
        self.tokens.append(
            Token(kind, self.source[self.start : self.current], self.get_location())
        )

    def get_location(self) -> Location:
        return Location(self.line, self.start_offset, self.current_offset)

    def eof(self) -> bool:
        return self.current >= len(self.source)

    def peek(self) -> str:
        return self.source[self.current]

    def peek_next(self) -> str:
        return self.source[self.current + 1]

    def advance(self) -> str:
        char = self.peek()
        self.current += 1
        self.current_offset += 1
        return char

    def match(self, expected: str) -> bool:
        if self.eof() or self.peek() != expected:
            return False

        self.advance()
        return True


class Parser:
    def __init__(self, source: str, path: str) -> None:
        self.source = source
        self.path = path
        self.logger = Logger(source, path)
        self.tokens = []
        self.instructions = []
        self.current = 0

    def __repr__(self) -> str:
        return f"{self.instructions}"

    def parse(self, tokens: list) -> list:
        self.tokens = tokens.copy()

        while not self.eof():
            instruction = self.parse_instruction()
            if instruction is not None:
                self.instructions.append(instruction)

        return self.instructions

    def parse_instruction(self) -> Instruction | None:
        token = self.advance()

        match token.kind:
            case TokenKind.IDENTIFIER:
                return Instruction(
                    "push",
                    {"type": TypeKind.VOID, "value": token.lexeme},
                    token,
                )
            case TokenKind.STRING:
                return Instruction(
                    "push",
                    {"type": TypeKind.STRING, "value": str(token.lexeme[1:-1])},
                    token,
                )
            case TokenKind.RUNE:
                return Instruction(
                    "push",
                    {"type": TypeKind.RUNE, "value": str(token.lexeme[1:-1])},
                    token,
                )
            case TokenKind.INTEGER:
                int_value = int(token.lexeme)
                int_type = self.get_smallest_integer_type(int_value)

                return Instruction(
                    "push",
                    {"type": int_type, "value": int_value},
                    token,
                )
            case TokenKind.FLOAT:
                float_value = float(token.lexeme)
                float_type = self.get_smallest_float_type(float_value)

                return Instruction(
                    "push",
                    {"type": float_type, "value": float_value},
                    token,
                )
            case TokenKind.BOOLEAN:
                return Instruction(
                    "push",
                    {"type": TypeKind.BOOL, "value": token.lexeme.lower() == "true"},
                    token,
                )
            case TokenKind.TYPE:
                return Instruction(
                    "push",
                    {
                        "type": TypeKind.TYPE,
                        "value": self.parse_type(default_token=token),
                    },
                    token,
                )

            case TokenKind.PLUS:
                return Instruction(
                    "addition",
                    {"type": None, "value": token.lexeme},
                    token,
                )
            case TokenKind.MINUS:
                return Instruction(
                    "subtraction",
                    {"type": None, "value": token.lexeme},
                    token,
                )
            case TokenKind.STAR:
                return Instruction(
                    "multiplication",
                    {"type": None, "value": token.lexeme},
                    token,
                )
            case TokenKind.SLASH:
                return Instruction(
                    "division",
                    {"type": None, "value": token.lexeme},
                    token,
                )
            case TokenKind.SLASH_SLASH:
                return Instruction(
                    "euclidian_division",
                    {"type": None, "value": token.lexeme},
                    token,
                )
            case TokenKind.PERCENT:
                return Instruction(
                    "remainder",
                    {"type": None, "value": token.lexeme},
                    token,
                )
            case TokenKind.CARET:
                return Instruction(
                    "power",
                    {"type": None, "value": token.lexeme},
                    token,
                )
            case TokenKind.QUESTION:
                return Instruction(
                    "default",
                    {"type": None, "value": token.lexeme},
                    token,
                )

            case TokenKind.AND:
                return Instruction(
                    "and",
                    {"type": None, "value": token.lexeme},
                    token,
                )
            case TokenKind.OR:
                return Instruction(
                    "or",
                    {"type": None, "value": token.lexeme},
                    token,
                )
            case TokenKind.XOR:
                return Instruction(
                    "xor",
                    {"type": None, "value": token.lexeme},
                    token,
                )
            case TokenKind.NOT:
                return Instruction(
                    "not",
                    {"type": None, "value": token.lexeme},
                    token,
                )
            case TokenKind.LEFT_SHIFT:
                return Instruction(
                    "left_shift",
                    {"type": None, "value": token.lexeme},
                    token,
                )
            case TokenKind.RIGHT_SHIFT:
                return Instruction(
                    "right_shift",
                    {"type": None, "value": token.lexeme},
                    token,
                )

            case TokenKind.EQUAL_EQUAL:
                return Instruction(
                    "equal",
                    {"type": None, "value": token.lexeme},
                    token,
                )
            case TokenKind.NOT_EQUAL:
                return Instruction(
                    "not_equal",
                    {"type": None, "value": token.lexeme},
                    token,
                )
            case TokenKind.GREATER:
                return Instruction(
                    "greater",
                    {"type": None, "value": token.lexeme},
                    token,
                )
            case TokenKind.GREATER_EQUAL:
                return Instruction(
                    "greater_equal",
                    {"type": None, "value": token.lexeme},
                    token,
                )
            case TokenKind.LESS:
                return Instruction(
                    "less",
                    {"type": None, "value": token.lexeme},
                    token,
                )
            case TokenKind.LESS_EQUAL:
                return Instruction(
                    "less_equal",
                    {"type": None, "value": token.lexeme},
                    token,
                )

            case TokenKind.POP:
                return Instruction(
                    "pop",
                    {"type": None, "value": token.lexeme},
                    token,
                )
            case TokenKind.DROP:
                return Instruction(
                    "drop",
                    {"type": None, "value": token.lexeme},
                    token,
                )
            case TokenKind.DUP:
                return Instruction(
                    "dup",
                    {"type": None, "value": token.lexeme},
                    token,
                )
            case TokenKind.OVER:
                return Instruction(
                    "over",
                    {"type": None, "value": token.lexeme},
                    token,
                )
            case TokenKind.ROT:
                return Instruction(
                    "rot",
                    {"type": None, "value": token.lexeme},
                    token,
                )
            case TokenKind.SWAP:
                return Instruction(
                    "swap",
                    {"type": None, "value": token.lexeme},
                    token,
                )

            case TokenKind.RETURN:
                return Instruction(
                    "return",
                    {"type": None, "value": token.lexeme},
                    token,
                )
            case TokenKind.CALL:
                return Instruction(
                    "call",
                    {"type": None, "value": token.lexeme},
                    token,
                )
            case TokenKind.BREAK:
                return Instruction(
                    "break",
                    {"type": None, "value": token.lexeme},
                    token,
                )
            case TokenKind.CONTINUE:
                return Instruction(
                    "continue",
                    {"type": None, "value": token.lexeme},
                    token,
                )
            case TokenKind.UNWRAP:
                return Instruction(
                    "unwrap",
                    {"type": None, "value": token.lexeme},
                    token,
                )
            case TokenKind.SET:
                return Instruction(
                    "set",
                    {"type": None, "value": token.lexeme},
                    token,
                )

            case TokenKind.MUTABLE:
                return self.handle_mutables(token)
            case TokenKind.CONST:
                return self.handle_consts(token)
            case TokenKind.STATIC:
                return self.handle_statics(token)
            case TokenKind.FUNCTION:
                return self.handle_functions(token)
            case TokenKind.IF:
                return self.handle_if_elses(token)
            case TokenKind.MATCH:
                return self.handle_matchs(token)
            case TokenKind.WHILE:
                return self.handle_whiles(token)
            case TokenKind.STRUCT:
                return self.handle_structs(token)
            case TokenKind.IMPLEMENT:
                return self.handle_implements(token)
            case TokenKind.ENUM:
                return self.handle_enums(token)
            case TokenKind.UNION:
                return self.handle_unions(token)
            case TokenKind.REQUIRE:
                return self.handle_requires(token)
            case TokenKind.DOT:
                return self.handle_dots(token)
            case TokenKind.DEFER:
                return self.handle_defers(token)
            case TokenKind.HANDLE:
                return self.handle_handles(token)
            case TokenKind.NEW:
                return self.handle_news(token)

            case TokenKind.LEFT_SQUARE:
                return self.handle_arrays(token)
            case TokenKind.LEFT_CURLY:
                return self.handle_hashmaps(token)
            case TokenKind.LEFT_PAREN:
                return self.handle_lists(token)

        return None

    def handle_mutables(self, token: Token) -> Instruction:
        variable_type = self.parse_type()

        return Instruction(
            "mutable",
            {
                "type": None,
                "value": variable_type,
            },
            token,
        )

    def handle_consts(self, token: Token) -> Instruction:
        variable_type = self.parse_type()

        return Instruction(
            "const",
            {
                "type": None,
                "value": variable_type,
            },
            token,
        )

    def handle_statics(self, token: Token) -> Instruction:
        variable_type = self.parse_type()

        return Instruction(
            "static",
            {
                "type": None,
                "value": variable_type,
            },
            token,
        )

    def handle_functions(self, token: Token) -> Instruction:
        parameters = []
        while not self.eof() and self.peek().kind != TokenKind.DO:
            if self.peek().kind not in [
                TokenKind.TYPE,
                TokenKind.IDENTIFIER,
                TokenKind.DO,
            ]:
                self.logger.error(
                    "Invalid function syntax !",
                    location=self.peek().location,
                    location_message="parameters are composed of a type followed by an identifier",
                )
                raise GlobalException

            variable_type = self.parse_type(no_error=True)
            variable_name = self.expect(TokenKind.IDENTIFIER)

            if variable_type is None:
                self.logger.error(
                    "Invalid parameter syntax !",
                    location=variable_name.location,
                    location_message="there should be a type before this",
                )
                raise GlobalException
            elif variable_name is None:
                self.logger.error(
                    "Invalid parameter syntax !",
                    location=variable_type.location,
                    location_message="there should be an identifier after this",
                )
                raise GlobalException

            parameters.append(
                {"variable_name": variable_name, "variable_type": variable_type}
            )

        if self.eof() or self.expect(TokenKind.DO) is None:
            self.logger.error(
                "Invalid function syntax !",
                location=self.peek_previous().location,
                location_message="there should be a function body after this",
            )
            self.logger.info(
                "Open a function body with a `do` and close it with a `end` !"
            )
            raise GlobalException

        body = []
        while not self.eof() and self.peek().kind != TokenKind.END:
            instruction = self.parse_instruction()
            if instruction is not None:
                body.append(instruction)

        if self.eof() or self.expect(TokenKind.END) is None:
            self.logger.error(
                "Function not closed !",
                location=self.peek_previous().location,
                location_message="you need to close a function with `end`",
            )
            raise GlobalException

        return_type = None
        return_error = None
        if not self.eof() and self.expect(TokenKind.WITH) is not None:
            return_type = self.parse_type(no_error=True)
            if return_type is None:
                self.logger.error(
                    "Invalid function syntax !",
                    location=self.peek_previous().location,
                    location_message="there should be a type after this",
                )
                self.logger.info(
                    "If you don't want to specify a return type, don't put a `with`. It will return `void` by default !"
                )
                raise GlobalException

            if not self.eof() and self.expect(TokenKind.OR) is not None:
                return_error = self.parse_type(no_error=True)
                if return_error is None:
                    self.logger.error(
                        "Invalid function syntax !",
                        location=self.peek_previous().location,
                        location_message="there should be an error type after this",
                    )
                    raise GlobalException

        return Instruction(
            "function",
            {
                "type": None,
                "value": {
                    "parameters": parameters,
                    "body": body,
                    "return_type": return_type,
                    "return_error": return_error,
                },
            },
            token,
        )

    def handle_if_elses(self, token: Token) -> Instruction:
        if_body = []
        while not self.eof() and self.peek().kind not in [
            TokenKind.ELSE,
            TokenKind.END,
        ]:
            instruction = self.parse_instruction()
            if instruction is not None:
                if_body.append(instruction)

        else_body = []
        else_token = self.expect(TokenKind.ELSE)
        if not self.eof() and else_token is not None:
            while not self.eof() and self.peek().kind != TokenKind.END:
                instruction = self.parse_instruction()
                if instruction is not None:
                    else_body.append(instruction)

        if self.eof() or self.expect(TokenKind.END) is None:
            self.logger.error(
                "If statement not closed !",
                location=else_token.location
                if else_token is not None
                else token.location,
                location_message="you need to close an if statement with `end`",
            )
            raise GlobalException

        return Instruction(
            "if",
            {
                "type": None,
                "value": {"if": if_body, "else": else_body},
            },
            token,
        )

    def handle_matchs(self, token: Token) -> Instruction:
        cases = []
        else_body = []
        while not self.eof() and self.peek().kind != TokenKind.END:
            else_token = self.expect(TokenKind.ELSE)
            if else_token is not None:
                while not self.eof() and self.peek().kind != TokenKind.END:
                    instruction = self.parse_instruction()
                    if instruction is not None:
                        else_body.append(instruction)

                if self.eof() or self.expect(TokenKind.END) is None:
                    self.logger.error(
                        "Case not closed !",
                        location=else_token.location,
                        location_message="you need to close a case with `end`",
                    )
                    raise GlobalException

                break

            case_condition = []
            while not self.eof() and self.peek().kind != TokenKind.CASE:
                instruction = self.parse_instruction()
                if instruction is not None:
                    case_condition.append(instruction)

            case_token = self.expect(TokenKind.CASE)
            if case_condition and case_token is None:
                self.logger.error(
                    "Invalid match syntax !",
                    location=case_condition[-1].token.location,
                    location_message="there should be a case body after this",
                )
                raise GlobalException
            elif not case_condition and case_token is not None:
                self.logger.error(
                    "Invalid match syntax !",
                    location=self.peek_previous().location,
                    location_message="there should be a condition before this",
                )
                raise GlobalException

            if not case_condition and case_token is None:
                break

            case_body = []
            while not self.eof() and self.peek().kind != TokenKind.END:
                instruction = self.parse_instruction()
                if instruction is not None:
                    case_body.append(instruction)

            if self.eof() or self.expect(TokenKind.END) is None:
                self.logger.error(
                    "Case not closed !",
                    location=case_token.location,
                    location_message="you need to close a case with `end`",
                )
                raise GlobalException

            cases.append({"condition": case_condition, "body": case_body})

        if self.eof() or self.expect(TokenKind.END) is None:
            self.logger.error(
                "Match statement not closed !",
                location=token.location,
                location_message="you need to close a match statement with `end`",
            )
            raise GlobalException

        return Instruction(
            "match",
            {
                "type": None,
                "value": {"cases": cases, "else": else_body},
            },
            token,
        )

    def handle_whiles(self, token: Token) -> Instruction:
        body = []
        while not self.eof() and self.peek().kind != TokenKind.END:
            instruction = self.parse_instruction()
            if instruction is not None:
                body.append(instruction)

        if self.eof() or self.expect(TokenKind.END) is None:
            self.logger.error(
                "While statement not closed !",
                location=token.location,
                location_message="you need to close a case with `end`",
            )
            raise GlobalException

        return Instruction(
            "while",
            {
                "type": None,
                "value": {"body": body},
            },
            token,
        )

    def handle_structs(self, token: Token) -> Instruction:
        body = []
        while not self.eof() and self.peek().kind != TokenKind.END:
            if self.peek().kind not in [
                TokenKind.TYPE,
                TokenKind.IDENTIFIER,
            ]:
                self.logger.error(
                    "Invalid struct syntax !",
                    location=self.peek().location,
                    location_message="struct fields are composed of a type followed by an identifier",
                )
                raise GlobalException

            variable_type = self.parse_type(no_error=True)
            variable_name = self.expect(TokenKind.IDENTIFIER)

            if variable_type is None:
                self.logger.error(
                    "Invalid struct syntax !",
                    location=variable_name.location,
                    location_message="there should be a type before this",
                )
                raise GlobalException
            elif variable_name is None:
                self.logger.error(
                    "Invalid struct syntax !",
                    location=variable_type.location,
                    location_message="there should be an identifier after this",
                )
                raise GlobalException

            body.append(
                {"variable_name": variable_name, "variable_type": variable_type}
            )

        if self.eof() or self.expect(TokenKind.END) is None:
            self.logger.error(
                "Struct statement not closed !",
                location=token.location,
                location_message="you need to close a struct statement with `end`",
            )
            raise GlobalException

        return Instruction(
            "struct",
            {
                "type": None,
                "value": {"body": body},
            },
            token,
        )

    def handle_implements(self, token: Token) -> Instruction:
        body = []
        while not self.eof() and self.peek().kind != TokenKind.END:
            if self.peek().kind not in [
                TokenKind.FUNCTION,
                TokenKind.IDENTIFIER,
            ]:
                self.logger.error(
                    "Invalid implement syntax !",
                    location=self.peek().location,
                    location_message="implement are composed of functions only",
                )
                raise GlobalException

            instruction = self.parse_instruction()
            if instruction is not None:
                body.append(instruction)

        if self.eof() or self.expect(TokenKind.END) is None:
            self.logger.error(
                "Implement statement not closed !",
                location=token.location,
                location_message="you need to close an implement statement with `end`",
            )
            raise GlobalException

        return Instruction(
            "implement",
            {
                "type": None,
                "value": {"body": body},
            },
            token,
        )

    def handle_enums(self, token: Token) -> Instruction:
        body = []
        while not self.eof() and self.peek().kind != TokenKind.END:
            identifier = self.expect(TokenKind.IDENTIFIER)
            if identifier is None:
                self.logger.error(
                    "Invalid enum syntax !",
                    location=self.peek().location,
                    location_message="there should be an identifier here",
                )
                self.logger.info(
                    "After an identifier, you can give an integer or a float to start the enum from !"
                )
                raise GlobalException

            value = self.expect(TokenKind.INTEGER) or self.expect(TokenKind.FLOAT)
            body.append({"identifier": identifier, "value": value})

        if self.eof() or self.expect(TokenKind.END) is None:
            self.logger.error(
                "Enum statement not closed !",
                location=token.location,
                location_message="you need to close an enum statement with `end`",
            )
            raise GlobalException

        return Instruction(
            "enum",
            {
                "type": None,
                "value": {"body": body},
            },
            token,
        )

    def handle_unions(self, token: Token) -> Instruction:
        body = []
        while not self.eof() and self.peek().kind != TokenKind.END:
            union_type = self.parse_type(no_error=True)
            if union_type is None:
                self.logger.error(
                    "Invalid union syntax !",
                    location=self.peek().location,
                    location_message="there should be a type here",
                )
                raise GlobalException

            body.append(union_type)

        if self.eof() or self.expect(TokenKind.END) is None:
            self.logger.error(
                "Union statement not closed !",
                location=token.location,
                location_message="you need to close an union statement with `end`",
            )
            raise GlobalException

        return Instruction(
            "union",
            {
                "type": None,
                "value": {"body": body},
            },
            token,
        )

    def handle_requires(self, token: Token) -> Instruction:
        scope = self.expect(TokenKind.IDENTIFIER)

        return Instruction(
            "require",
            {
                "type": None,
                "value": {"scope": scope},
            },
            token,
        )

    def handle_dots(self, token: Token) -> Instruction:
        identifier = self.expect(TokenKind.IDENTIFIER)
        if identifier is None:
            self.logger.error(
                "Invalid dot syntax !",
                location=token.location,
                location_message="there should be an identifier after this",
            )
            raise GlobalException

        return Instruction(
            "dot",
            {
                "type": None,
                "value": {"identifier": identifier},
            },
            token,
        )

    def handle_defers(self, token: Token) -> Instruction:
        body = []
        while not self.eof() and self.peek().kind != TokenKind.END:
            instruction = self.parse_instruction()
            if instruction is not None:
                body.append(instruction)

        if self.eof() or self.expect(TokenKind.END) is None:
            self.logger.error(
                "Defer not closed !",
                location=token.location,
                location_message="you need to close a defer with `end`",
            )
            raise GlobalException

        return Instruction(
            "defer",
            {
                "type": None,
                "value": {"body": body},
            },
            token,
        )

    def handle_handles(self, token: Token) -> Instruction:
        body = []
        while not self.eof() and self.peek().kind != TokenKind.END:
            instruction = self.parse_instruction()
            if instruction is not None:
                body.append(instruction)

        if self.eof() or self.expect(TokenKind.END) is None:
            self.logger.error(
                "Handle not closed !",
                location=token.location,
                location_message="you need to close a handle with `end`",
            )
            raise GlobalException

        return Instruction(
            "handle",
            {
                "type": None,
                "value": {"body": body},
            },
            token,
        )

    def handle_news(self, token: Token) -> Instruction:
        new_type = self.parse_type(no_error=True)
        if new_type is None:
            self.logger.error(
                "Invalid new syntax !",
                location=token.location,
                location_message="there should be a type after this",
            )
            raise GlobalException

        return Instruction(
            "new",
            {
                "type": None,
                "value": {"type": new_type},
            },
            token,
        )

    def handle_lists(self, token: Token) -> Instruction:
        body = []
        while not self.eof() and self.peek().kind != TokenKind.RIGHT_PAREN:
            instruction = self.parse_instruction()
            if instruction is not None:
                body.append(instruction)

        if self.eof() or self.expect(TokenKind.RIGHT_PAREN) is None:
            self.logger.error(
                "List not closed !",
                location=token.location,
                location_message="you need to close a list with `)`",
            )
            raise GlobalException

        return Instruction(
            "list",
            {"body": body},
            token,
        )

    def handle_arrays(self, token: Token) -> Instruction:
        body = []
        while not self.eof() and self.peek().kind != TokenKind.RIGHT_SQUARE:
            instruction = self.parse_instruction()
            if instruction is not None:
                body.append(instruction)

        if self.eof() or self.expect(TokenKind.RIGHT_SQUARE) is None:
            self.logger.error(
                "Array not closed !",
                location=token.location,
                location_message="you need to close an array with `]`",
            )
            raise GlobalException

        return Instruction(
            "array",
            {"body": body},
            token,
        )

    def handle_hashmaps(self, token: Token) -> Instruction:
        body = []
        while not self.eof() and self.peek().kind != TokenKind.RIGHT_CURLY:
            instruction = self.parse_instruction()
            if instruction is not None:
                body.append(instruction)

        if self.eof() or self.expect(TokenKind.RIGHT_CURLY) is None:
            self.logger.error(
                "Hashmap not closed !",
                location=token.location,
                location_message="you need to close an hashmap with `}`",
            )
            raise GlobalException

        return Instruction(
            "hashmap",
            {"body": body},
            token,
        )

    def get_smallest_integer_type(self, value: int) -> TypeKind:
        if value >= 0:
            if value <= 2**8 - 1:
                return TypeKind.U8
            elif value <= 2**16 - 1:
                return TypeKind.U16
            elif value <= 2**32 - 1:
                return TypeKind.U32
            elif value <= 2**64 - 1:
                return TypeKind.U64
            elif value <= 2**128 - 1:
                return TypeKind.U128

        if -(2**7) <= value <= 2**7 - 1:
            return TypeKind.I8
        elif -(2**15) <= value <= 2**15 - 1:
            return TypeKind.I16
        elif -(2**31) <= value <= 2**31 - 1:
            return TypeKind.I32
        elif -(2**63) <= value <= 2**63 - 1:
            return TypeKind.I64
        else:
            return TypeKind.I128

    def get_smallest_float_type(self, value: float) -> TypeKind:
        if math.isnan(value) or math.isinf(value):
            self.logger.error(
                f"Invalid float value {value} (NaN or Infinity) !",
                location=self.get_location(),
                location_message="need to be a valid float value",
            )
            raise GlobalException
        if value == 0.0:
            return TypeKind.F16

        abs_value = abs(value)
        str_value = f"{abs_value:.16e}".split("e")[0].replace(".", "").rstrip("0")
        significant_digits = len(str_value)

        if abs_value <= 65504 and significant_digits <= 4:
            return TypeKind.F16
        elif abs_value <= 3.4e38 and significant_digits <= 7:
            return TypeKind.F32
        elif abs_value <= 1.8e308 and significant_digits <= 16:
            return TypeKind.F64
        else:
            return TypeKind.F128

    def parse_type(
        self, no_error: bool = False, default_token: Token | None = None
    ) -> dict | None:
        variable_type = default_token
        if variable_type is None:
            variable_type = self.expect(TokenKind.TYPE) or self.expect(
                TokenKind.IDENTIFIER
            )
            if variable_type is None:
                if no_error:
                    return None

                self.logger.error(
                    "Invalid type syntax !",
                    location=self.peek_previous().location,
                    location_message="there should be a type after this",
                )
                raise GlobalException

        key_type = None
        value_type = None
        contained_size = None
        if not self.eof() and self.expect(TokenKind.LESS) is not None:
            key_type = self.parse_type()
            value_type = self.parse_type(no_error=True)

            if value_type is None:
                contained_size = self.expect(TokenKind.INTEGER)

            if self.eof() or self.expect(TokenKind.GREATER) is None:
                self.logger.error(
                    "Invalid type syntax !",
                    location=variable_type.location,
                    location_message="you need to close a contained type definition with `>`",
                )
                raise GlobalException

        return {
            "type": variable_type,
            "contained_type": {
                "key_type": key_type,
                "value_type": value_type,
            },
            "contained_size": contained_size,
        }

    def peek_previous(self) -> Token:
        return self.tokens[self.current - 1]

    def peek(self) -> Token:
        return self.tokens[self.current]

    def advance(self) -> Token:
        token = self.peek()
        self.current += 1
        return token

    def expect(self, expected_type: TokenKind) -> Token | None:
        if not self.eof() and self.peek().kind == expected_type:
            return self.advance()
        return None

    def eof(self) -> bool:
        return self.current >= len(self.tokens)


class Compiler:
    def __init__(self, source: str, path: str):
        self.source = source
        self.path = path
        self.output_path = ""
        self.logger = Logger(source, path)
        self.instructions = []
        self.current = 0
        self.temp_count = 0
        self.label_count = 0
        self.stack_types = []
        self.stack_symbols = {}
        self.type_map = {
            TypeKind.I8: ("i8", 1),
            TypeKind.U8: ("i8", 1),
            TypeKind.I16: ("i16", 2),
            TypeKind.U16: ("i16", 2),
            TypeKind.I32: ("i32", 4),
            TypeKind.U32: ("i32", 4),
            TypeKind.I64: ("i64", 8),
            TypeKind.U64: ("i64", 8),
            TypeKind.I128: ("i128", 16),
            TypeKind.U128: ("i128", 16),
            TypeKind.F16: ("half", 2),
            TypeKind.F32: ("float", 4),
            TypeKind.F64: ("double", 8),
            TypeKind.F128: ("fp128", 16),
            TypeKind.BOOL: ("i1", 1),
            TypeKind.STRING: ("{ ptr, i32 }", 8),
            TypeKind.RUNE: ("i32", 4),
            TypeKind.TYPE: ("ptr", 8),
            TypeKind.VOID: ("ptr", 8),
        }
        self.ir = {
            "preamble": [
                '; ModuleID = "yarrow"',
                f'source_filename = "{path}"',
                'target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i64:64-f80:128-n8:16:32:64-S128"',
                'target triple = "x86_64-pc-linux-gnu"',
            ],
            "global_declarations": [],
            "runtime_declarations": [
                "declare void @llvm.trap()",
            ],
            "functions": [],
            "start_main": [
                "define i32 @main() #0 {",
                "entry:",
                "   %stack = alloca [1024 x i8], align 16",
                "   %stack_pointer = alloca ptr, align 8",
                "   %type_stack = alloca [1024 x i32], align 4",
                "   %type_stack_pointer = alloca ptr, align 8",
                "   store ptr %stack, ptr %stack_pointer, align 8",
                "   store ptr %type_stack, ptr %type_stack_pointer, align 8",
                "   %stack_end = getelementptr [1024 x i8], [1024 x i8]* %stack, i32 0, i32 1024",
                "   br label %main_body",
                "main_body:",
            ],
            "body_main": [],
            "end_main": [
                "   ret i32 0",
                "error:",
                "   call void @llvm.trap()",
                "   unreachable",
                "}",
                "attributes #0 = { noinline nounwind optnone uwtable }",
            ],
        }

    def __repr__(self) -> str:
        return f"{self.ir}"

    def compile(self, instructions: list) -> None:
        self.instructions = instructions.copy()
        self.output_path = os.path.splitext(self.path)[0] + ".ll"

        i = 0
        while i < len(self.instructions):
            if self.instructions[i].kind == "function":
                self.current = i
                self.emit_function(self.instructions[i])
                self.instructions.pop(i)
                self.instructions.pop(i - 1)
            else:
                i += 1

        self.current = 0
        while not self.eof():
            instruction = self.advance()
            self.translate_instruction(instruction)

        try:
            with open(self.output_path, "w", encoding="utf-8") as file:
                for line in [line for section in self.ir.values() for line in section]:
                    file.write(line + "\n")
        except Exception:
            self.logger.error(
                f"Failed to write LLVM IR to `{self.output_path}` !",
            )
            raise GlobalException

    def translate_instruction(
        self, instruction: Instruction, expected_return_type: TypeKind | None = None
    ) -> None:
        match instruction.kind:
            case "push":
                content = instruction.content
                value_type = content["type"]
                value = content["value"]

                match value_type:
                    case (
                        TypeKind.I8
                        | TypeKind.U8
                        | TypeKind.I16
                        | TypeKind.U16
                        | TypeKind.I32
                        | TypeKind.U32
                        | TypeKind.I64
                        | TypeKind.U64
                        | TypeKind.I128
                        | TypeKind.U128
                    ):
                        temp1 = self.next_temp()
                        llvm_type = self.type_map[value_type][0]
                        llvm_size = self.type_map[value_type][1]
                        self.ir["body_main"].append(
                            f"   %t{temp1} = add {llvm_type} 0, {value}"
                        )
                        self.emit_push(llvm_type, f"%t{temp1}", value_type, llvm_size)

                    case TypeKind.F16 | TypeKind.F32 | TypeKind.F64 | TypeKind.F128:
                        temp1 = self.next_temp()
                        llvm_type = self.type_map[value_type][0]
                        llvm_size = self.type_map[value_type][1]
                        self.ir["body_main"].append(
                            f"   %t{temp1} = fadd {llvm_type} 0.0, {value}"
                        )
                        self.emit_push(llvm_type, f"%t{temp1}", value_type, llvm_size)

                    case TypeKind.BOOL:
                        temp1 = self.next_temp()
                        llvm_type = self.type_map[value_type][0]
                        llvm_size = self.type_map[value_type][1]
                        self.ir["body_main"].append(
                            f"   %t{temp1} = add {llvm_type} 0, {'1' if value else '0'}"
                        )
                        self.emit_push(llvm_type, f"%t{temp1}", value_type, llvm_size)

                    case TypeKind.STRING:
                        temp1 = self.next_temp()
                        temp2 = self.next_temp()
                        temp3 = self.next_temp()
                        string_id = self.next_temp()
                        string_data = self.escape_string(value, instruction)
                        string_len = len(value)
                        llvm_type = self.type_map[value_type][0]
                        llvm_size = self.type_map[value_type][1]
                        self.ir["global_declarations"].append(
                            f'@.str.{string_id} = private unnamed_addr constant [{string_len} x i8] c"{string_data}"'
                        )
                        self.ir["body_main"].extend(
                            [
                                f"   %t{temp1} = getelementptr [{string_len} x i8], [{string_len} x i8]* @.str.{string_id}, i32 0, i32 0",
                                f"   %t{temp2} = insertvalue {llvm_type} undef, ptr %t{temp1}, 0",
                                f"   %t{temp3} = insertvalue {llvm_type} %t{temp2}, i32 {string_len}, 1",
                            ]
                        )
                        self.emit_push(llvm_type, f"%t{temp3}", value_type, llvm_size)

                    case TypeKind.RUNE:
                        rune_value = self.rune_to_unicode(value, instruction)
                        temp1 = self.next_temp()
                        llvm_type = self.type_map[value_type][0]
                        llvm_size = self.type_map[value_type][1]
                        self.ir["body_main"].append(
                            f"   %t{temp1} = add {llvm_type} 0, {rune_value}"
                        )
                        self.emit_push(llvm_type, f"%t{temp1}", value_type, llvm_size)

                    case TypeKind.TYPE:
                        temp1 = self.next_temp()
                        type_id = self.next_temp()
                        type_index = (
                            list(TypeKind).index(value) if value in TypeKind else 0
                        )
                        llvm_type = self.type_map[value_type][0]
                        llvm_size = self.type_map[value_type][1]
                        self.ir["global_declarations"].append(
                            f"   @.type.{type_id} = private unnamed_addr constant {{ i32, ptr }} {{ i32 {type_index}, ptr null }}"
                        )
                        self.ir["body_main"].append(
                            f"   %t{temp1} = getelementptr {{ i32, ptr }}, {{ i32, ptr }}* @.type.{type_id}, i32 0, i32 0"
                        )
                        self.emit_push(llvm_type, f"%t{temp1}", value_type, llvm_size)

                    case TypeKind.VOID:
                        llvm_type = self.type_map[value_type][0]
                        llvm_size = self.type_map[value_type][1]
                        self.emit_push(llvm_type, "null", value_type, llvm_size)

                    case _:
                        self.logger.warning(
                            f"Unsupported push type '{value_type}' for value '{value}'",
                            location=instruction.token.location,
                        )

            case "call":
                self.emit_call(instruction)
            case "return":
                self.emit_return(instruction, expected_return_type)

            case "pop":
                self.emit_pop()
            case "drop":
                self.emit_drop()
            case "dup":
                self.emit_dup()
            case "swap":
                self.emit_swap()
            case "over":
                self.emit_over()
            case "rot":
                self.emit_rot()

            case _:
                self.logger.warning(
                    f"Unsupported instruction '{instruction.kind}'",
                    location=instruction.token.location,
                )

    def emit_push(
        self, llvm_type: str, value: str, type_kind: TypeKind, size: int
    ) -> None:
        temp1 = self.next_temp()
        temp2 = self.next_temp()
        temp3 = self.next_temp()
        temp4 = self.next_temp()
        label1 = self.next_label()
        label2 = self.next_label()
        self.ir["body_main"].extend(
            [
                f"   %t{temp1} = load ptr, ptr %stack_pointer, align 8",
                f"   %t{temp2} = icmp ule ptr %t{temp1}, %stack_end",
                f"   br i1 %t{temp2}, label %{label1}, label %error",
                f"{label1}:",
                f"   store {llvm_type} {value}, ptr %t{temp1}, align {min(size, 8)}",
                f"   %t{temp3} = getelementptr i8, ptr %t{temp1}, i32 {size}",
                f"   store ptr %t{temp3}, ptr %stack_pointer, align 8",
                f"   %t{temp4} = load ptr, ptr %type_stack_pointer, align 8",
                f"   store i32 {type_kind.value}, ptr %t{temp4}, align 4",
                f"   %t{temp4}.1 = getelementptr i8, ptr %t{temp4}, i32 4",
                f"   store ptr %t{temp4}.1, ptr %type_stack_pointer, align 8",
                f"   br label %{label2}",
                f"{label2}:",
            ]
        )
        self.stack_types.append(type_kind)

    def emit_pop(self) -> None:
        if not self.stack_types:
            self.logger.error(
                "Pop operation on empty stack",
                location=self.instructions[self.current - 1].token.location,
            )
            raise GlobalException

        temp1 = self.next_temp()
        temp2 = self.next_temp()
        label1 = self.next_label()
        label2 = self.next_label()
        size = self.type_map[self.stack_types.pop()][1]
        self.ir["body_main"].extend(
            [
                f"   %t{temp1} = load ptr, ptr %stack_pointer, align 8",
                f"   %t{temp2} = icmp ugt ptr %t{temp1}, %stack",
                f"   br i1 %t{temp2}, label %{label1}, label %error",
                f"{label1}:",
                f"   %t{temp1}.1 = getelementptr i8, ptr %t{temp1}, i32 -{size}",
                f"   store ptr %t{temp1}.1, ptr %stack_pointer, align 8",
                f"   %t{temp1}.2 = load ptr, ptr %type_stack_pointer, align 8",
                f"   %t{temp1}.3 = getelementptr i8, ptr %t{temp1}.2, i32 -4",
                f"   store ptr %t{temp1}.3, ptr %type_stack_pointer, align 8",
                f"   br label %{label2}",
                f"{label2}:",
            ]
        )

    def emit_drop(self) -> None:
        self.ir["body_main"].extend(
            [
                "   store ptr %stack, ptr %stack_pointer, align 8",
                "   store ptr %type_stack, ptr %type_stack_pointer, align 8",
            ]
        )
        self.stack_types.clear()

    def emit_dup(self) -> None:
        if not self.stack_types:
            self.logger.error(
                "Dup operation on empty stack",
                location=self.instructions[self.current - 1].token.location,
            )
            raise GlobalException

        type_kind = self.stack_types[-1]
        llvm_type, size = self.type_map[type_kind]
        value, _ = self.load_typed_value(0, type_kind)
        temp1 = self.next_temp()
        self.ir["body_main"].extend(
            [
                f"   %t{temp1} = load ptr, ptr %stack_pointer, align 8",
                f"   store {llvm_type} {value}, ptr %t{temp1}, align {min(size, 8)}",
                f"   %t{temp1}.1 = getelementptr i8, ptr %t{temp1}, i32 {size}",
                f"   store ptr %t{temp1}.1, ptr %stack_pointer, align 8",
                f"   %t{temp1}.2 = load ptr, ptr %type_stack_pointer, align 8",
                f"   %t{temp1}.3 = load i32, ptr %t{temp1}.2, align 4",
                f"   store i32 %t{temp1}.3, ptr %t{temp1}.2, align 4",
                f"   %t{temp1}.4 = getelementptr i8, ptr %t{temp1}.2, i32 4",
                f"   store ptr %t{temp1}.4, ptr %type_stack_pointer, align 8",
            ]
        )
        self.stack_types.append(type_kind)

    def emit_swap(self) -> None:
        if len(self.stack_types) < 2:
            self.logger.error(
                "Swap operation requires at least two stack elements",
                location=self.instructions[self.current - 1].token.location,
            )
            raise GlobalException

        type_kind1 = self.stack_types[-1]
        size1 = self.type_map[type_kind1][1]
        type_kind2 = self.stack_types[-2]
        size2 = self.type_map[type_kind2][1]
        value1, llvm_type1 = self.load_typed_value(0, type_kind1)
        value2, llvm_type2 = self.load_typed_value(1, type_kind2)
        temp1 = self.next_temp()
        temp2 = self.next_temp()
        self.ir["body_main"].extend(
            [
                f"   %t{temp1} = load ptr, ptr %stack_pointer, align 8",
                f"   %t{temp2} = getelementptr i8, ptr %t{temp1}, i32 {size1}",
                f"   store {llvm_type2} {value2}, ptr %t{temp1}, align {min(size2, 8)}",
                f"   store {llvm_type1} {value1}, ptr %t{temp2}, align {min(size1, 8)}",
                f"   %t{temp1}.3 = load ptr, ptr %type_stack_pointer, align 8",
                f"   %t{temp1}.4 = getelementptr i8, ptr %t{temp1}.3, i32 -4",
                f"   %t{temp1}.5 = load i32, ptr %t{temp1}.4, align 4",
                f"   %t{temp1}.6 = getelementptr i8, ptr %t{temp1}.4, i32 -4",
                f"   %t{temp1}.7 = load i32, ptr %t{temp1}.6, align 4",
                f"   store i32 %t{temp1}.7, ptr %t{temp1}.4, align 4",
                f"   store i32 %t{temp1}.5, ptr %t{temp1}.6, align 4",
            ]
        )
        self.stack_types[-1], self.stack_types[-2] = (
            self.stack_types[-2],
            self.stack_types[-1],
        )

    def emit_over(self) -> None:
        if len(self.stack_types) < 2:
            self.logger.error(
                "Over operation requires at least two stack elements",
                location=self.instructions[self.current - 1].token.location,
            )
            raise GlobalException

        type_kind = self.stack_types[-2]
        size = self.type_map[type_kind][1]
        value, llvm_type = self.load_typed_value(1, type_kind)
        temp1 = self.next_temp()
        self.ir["body_main"].extend(
            [
                f"   %t{temp1} = load ptr, ptr %stack_pointer, align 8",
                f"   store {llvm_type} {value}, ptr %t{temp1}, align {min(size, 8)}",
                f"   %t{temp1}.1 = getelementptr i8, ptr %t{temp1}, i32 {size}",
                f"   store ptr %t{temp1}.1, ptr %stack_pointer, align 8",
                f"   %t{temp1}.2 = load ptr, ptr %type_stack_pointer, align 8",
                f"   %t{temp1}.3 = getelementptr i8, ptr %t{temp1}.2, i32 -4",
                f"   %t{temp1}.4 = load i32, ptr %t{temp1}.3, align 4",
                f"   store i32 %t{temp1}.4, ptr %t{temp1}.2, align 4",
                f"   %t{temp1}.5 = getelementptr i8, ptr %t{temp1}.2, i32 4",
                f"   store ptr %t{temp1}.5, ptr %type_stack_pointer, align 8",
            ]
        )
        self.stack_types.append(type_kind)

    def emit_rot(self) -> None:
        if len(self.stack_types) < 3:
            self.logger.error(
                "Rot operation requires at least three stack elements",
                location=self.instructions[self.current - 1].token.location,
            )
            raise GlobalException

        type_kind1 = self.stack_types[-1]
        size1 = self.type_map[type_kind1][1]
        type_kind2 = self.stack_types[-2]
        size2 = self.type_map[type_kind2][1]
        type_kind3 = self.stack_types[-3]
        size3 = self.type_map[type_kind3][1]
        value1, llvm_type1 = self.load_typed_value(0, type_kind1)
        value2, llvm_type2 = self.load_typed_value(1, type_kind2)
        value3, llvm_type3 = self.load_typed_value(2, type_kind3)
        temp1 = self.next_temp()
        temp2 = self.next_temp()
        temp3 = self.next_temp()
        self.ir["body_main"].extend(
            [
                f"   %t{temp1} = load ptr, ptr %stack_pointer, align 8",
                f"   %t{temp2} = getelementptr i8, ptr %t{temp1}, i32 {size1}",
                f"   %t{temp3} = getelementptr i8, ptr %t{temp2}, i32 {size2}",
                f"   store {llvm_type2} {value2}, ptr %t{temp1}, align {min(size2, 8)}",
                f"   store {llvm_type3} {value3}, ptr %t{temp2}, align {min(size3, 8)}",
                f"   store {llvm_type1} {value1}, ptr %t{temp3}, align {min(size1, 8)}",
                f"   %t{temp1}.4 = load ptr, ptr %type_stack_pointer, align 8",
                f"   %t{temp1}.5 = getelementptr i8, ptr %t{temp1}.4, i32 -4",
                f"   %t{temp1}.6 = load i32, ptr %t{temp1}.5, align 4",
                f"   %t{temp1}.7 = getelementptr i8, ptr %t{temp1}.5, i32 -4",
                f"   %t{temp1}.8 = load i32, ptr %t{temp1}.7, align 4",
                f"   %t{temp1}.9 = getelementptr i8, ptr %t{temp1}.7, i32 -4",
                f"   %t{temp1}.10 = load i32, ptr %t{temp1}.9, align 4",
                f"   store i32 %t{temp1}.8, ptr %t{temp1}.5, align 4",
                f"   store i32 %t{temp1}.10, ptr %t{temp1}.7, align 4",
                f"   store i32 %t{temp1}.6, ptr %t{temp1}.9, align 4",
            ]
        )
        self.stack_types[-3:] = [
            self.stack_types[-2],
            self.stack_types[-3],
            self.stack_types[-1],
        ]

    def emit_function(self, instruction: Instruction) -> None:
        if (
            self.current == 0
            or self.instructions[self.current - 1].kind != "push"
            or self.instructions[self.current - 1].content["type"] != TypeKind.VOID
        ):
            self.logger.error(
                "Function must be preceded by an identifier",
                location=instruction.token.location,
                location_message="expected function name before declaration",
            )
            raise GlobalException

        function_data = instruction.content["value"]
        function_name = self.instructions[self.current - 1].content["value"]

        return_type = function_data["return_type"]
        llvm_return_type = "void"
        type_kind_return = None
        if return_type:
            type_kind_return = TypeKind[return_type["type"].lexeme.upper()]
            llvm_return_type = self.type_map[type_kind_return][0]

        if llvm_return_type != "void":
            has_return = False
            for body_instruction in function_data["body"]:
                if body_instruction.kind == "return":
                    has_return = True
                    break
            if not has_return:
                self.logger.error(
                    f"Function '{function_name}' with non-void return type '{llvm_return_type}' must contain a return statement",
                    location=instruction.token.location,
                    location_message="add a return statement in the function body",
                )
                raise GlobalException

        parameter_types = []
        parameter_names = []
        parameter_type_kinds = []
        for parameter in function_data["parameters"]:
            param_type_kind = TypeKind[
                parameter["variable_type"]["type"].lexeme.upper()
            ]
            parameter_type_kinds.append(param_type_kind)
            parameter_types.append(self.type_map[param_type_kind][0])
            parameter_names.append(parameter["variable_name"].lexeme)
            expected_type = TypeKind[parameter["variable_type"]["type"].lexeme.upper()]
            if param_type_kind != expected_type:
                self.logger.error(
                    f"Type mismatch for parameter '{parameter['variable_name'].lexeme}': expected {expected_type}, got {param_type_kind}",
                    location=parameter["variable_type"]["type"].location,
                )
                raise GlobalException

        self.stack_symbols[function_name] = {
            "kind": "function",
            "type": llvm_return_type,
            "parameter_types": parameter_types,
            "parameter_names": parameter_names,
            "parameter_type_kinds": parameter_type_kinds,
            "return_type_kind": type_kind_return,
        }

        function_ir = [
            f"define {llvm_return_type} @{function_name}({', '.join([f'{parameter_type} %{parameter_name}' for parameter_type, parameter_name in zip(parameter_types, parameter_names)])}) #0 {{",
            "entry:",
        ]

        function_ir.extend(
            [
                "   %stack = alloca [1024 x i8], align 16",
                "   %stack_pointer = alloca ptr, align 8",
                "   %type_stack = alloca [1024 x i32], align 4",
                "   %type_stack_pointer = alloca ptr, align 8",
                "   store ptr %stack, ptr %stack_pointer, align 8",
                "   store ptr %type_stack, ptr %type_stack_pointer, align 8",
                "   %stack_end = getelementptr [1024 x i8], [1024 x i8]* %stack, i32 0, i32 1024",
            ]
        )

        saved_stack_types = self.stack_types.copy()
        self.stack_types = []
        for parameter_type, param_name, param_type_kind in zip(
            parameter_types, parameter_names, parameter_type_kinds
        ):
            size = self.type_map[param_type_kind][1]
            saved_temp_count = self.temp_count
            saved_label_count = self.label_count
            function_ir.append(
                f"   %t{self.next_temp()} = load ptr, ptr %stack_pointer, align 8"
            )
            self.ir["body_main"] = function_ir
            self.emit_push(parameter_type, f"%{param_name}", param_type_kind, size)
            function_ir = self.ir["body_main"]
            self.ir["body_main"] = []
            self.temp_count = saved_temp_count
            self.label_count = saved_label_count

        saved_body_main = self.ir["body_main"]
        self.ir["body_main"] = []
        for body_instruction in function_data["body"]:
            self.translate_instruction(
                body_instruction, expected_return_type=type_kind_return
            )
        function_ir.extend(self.ir["body_main"])
        self.ir["body_main"] = saved_body_main
        self.stack_types = saved_stack_types

        if llvm_return_type == "void":
            function_ir.append("   ret void")

        function_ir.extend(
            [
                "error:",
                "   call void @llvm.trap()",
                "   unreachable",
                "}",
            ]
        )
        self.ir["functions"].extend(function_ir)

    def emit_call(self, instruction: Instruction) -> None:
        if (
            self.current == 0
            or self.instructions[self.current - 2].kind != "push"
            or self.instructions[self.current - 2].content["type"] != TypeKind.VOID
        ):
            self.logger.error(
                "Function call must be preceded by function name",
                location=instruction.token.location,
                location_message="expected function identifier before call",
            )
            raise GlobalException

        function_name = self.instructions[self.current - 2].content["value"]
        self.emit_pop()

        if (
            function_name not in self.stack_symbols
            or self.stack_symbols[function_name]["kind"] != "function"
        ):
            self.logger.error(
                f"Undefined function '{function_name}'",
                location=instruction.token.location,
            )
            raise GlobalException

        function_data = self.stack_symbols[function_name]
        llvm_return_type = function_data["type"]
        parameter_types = function_data["parameter_types"]
        parameter_type_kinds = function_data["parameter_type_kinds"]
        parameter_count = len(parameter_types)

        if len(self.stack_types) < parameter_count:
            self.logger.error(
                f"Function '{function_name}' requires {parameter_count} parameters, but stack has only {len(self.stack_types)} elements",
                location=instruction.token.location,
            )
            raise GlobalException

        for i in range(parameter_count):
            expected_type = parameter_type_kinds[parameter_count - 1 - i]
            actual_type = self.stack_types[-(i + 1)]
            if actual_type != expected_type:
                self.logger.error(
                    f"Type mismatch for parameter {i + 1}: expected {expected_type}, got {actual_type}",
                    location=instruction.token.location,
                )
                raise GlobalException

        parameters = []
        total_size = 0
        for i in range(parameter_count):
            param_type_kind = parameter_type_kinds[parameter_count - 1 - i]
            parameter_type = parameter_types[parameter_count - 1 - i]
            size = self.type_map[param_type_kind][1]
            total_size += size
            temp2 = self.next_temp()
            self.ir["body_main"].extend(
                [
                    f"   %t{temp2} = load ptr, ptr %stack_pointer, align 8",
                    f"   %t{temp2}.1 = getelementptr i8, ptr %t{temp2}, i32 -{total_size}",
                    f"   %t{temp2}.2 = load {parameter_type}, ptr %t{temp2}.1, align {min(size, 8)}",
                ]
            )
            parameters.append(f"{parameter_type} %t{temp2}.2")

        if parameter_count > 0:
            temp3 = self.next_temp()
            self.ir["body_main"].extend(
                [
                    f"   %t{temp3} = load ptr, ptr %stack_pointer, align 8",
                    f"   %t{temp3}.1 = getelementptr i8, ptr %t{temp3}, i32 -{total_size}",
                    f"   store ptr %t{temp3}.1, ptr %stack_pointer, align 8",
                    f"   %t{temp3}.2 = load ptr, ptr %type_stack_pointer, align 8",
                    f"   %t{temp3}.3 = getelementptr i8, ptr %t{temp3}.2, i32 -{parameter_count * 4}",
                    f"   store ptr %t{temp3}.3, ptr %type_stack_pointer, align 8",
                ]
            )
            for _ in range(parameter_count):
                self.stack_types.pop()

        return_type_kind = function_data["return_type_kind"]
        function_call = (
            f"call {llvm_return_type} @{function_name}({', '.join(parameters)})"
        )
        if llvm_return_type != "void":
            temp4 = self.next_temp()
            size = self.type_map[return_type_kind][1]
            self.ir["body_main"].extend(
                [
                    f"   %t{temp4} = {function_call}",
                    f"   %t{temp4}.1 = load ptr, ptr %stack_pointer, align 8",
                    f"   store {llvm_return_type} %t{temp4}, ptr %t{temp4}.1, align {min(size, 8)}",
                    f"   %t{temp4}.2 = getelementptr i8, ptr %t{temp4}.1, i32 {size}",
                    f"   store ptr %t{temp4}.2, ptr %stack_pointer, align 8",
                    f"   %t{temp4}.3 = load ptr, ptr %type_stack_pointer, align 8",
                    f"   store i32 {return_type_kind.value}, ptr %t{temp4}.3, align 4",
                    f"   %t{temp4}.4 = getelementptr i8, ptr %t{temp4}.3, i32 4",
                    f"   store ptr %t{temp4}.4, ptr %type_stack_pointer, align 8",
                ]
            )
            self.stack_types.append(return_type_kind)
        else:
            self.ir["body_main"].append(f"   {function_call}")

    def emit_return(
        self, instruction: Instruction, expected_return_type: TypeKind | None = None
    ) -> None:
        if expected_return_type is not None:
            if not self.stack_types:
                self.logger.error(
                    "Return statement with empty stack in function with non-void return type",
                    location=instruction.token.location,
                    location_message=f"expected a value of type {expected_return_type} on the stack",
                )
                raise GlobalException

            top_type = self.stack_types[-1]
            top_size = self.type_map[top_type][1]
            if top_type != expected_return_type:
                self.logger.error(
                    f"Type mismatch in return statement: expected {expected_return_type}, got {top_type}",
                    location=instruction.token.location,
                    location_message="return value type must match function's return type",
                )
                raise GlobalException

            llvm_type = self.type_map[expected_return_type][0]
            temp1 = self.next_temp()
            self.ir["body_main"].extend(
                [
                    f"   %t{temp1} = load ptr, ptr %stack_pointer, align 8",
                    f"   %t{temp1}.1 = getelementptr i8, ptr %t{temp1}, i32 -{top_size}",
                    f"   %t{temp1}.2 = load {llvm_type}, ptr %t{temp1}.1, align {min(top_size, 8)}",
                    f"   ret {llvm_type} %t{temp1}.2",
                ]
            )
        else:
            if self.stack_types:
                self.logger.warning(
                    "Return statement with non-empty stack in void function",
                    location=instruction.token.location,
                    location_message="stack values will be ignored",
                )
            self.ir["body_main"].append("   ret void")

    def load_typed_value(
        self, offset: int, expected_type: TypeKind | None = None
    ) -> tuple:
        if offset >= len(self.stack_types):
            self.logger.error(
                f"Stack underflow: attempted to load element at offset {offset}, but stack has only {len(self.stack_types)} elements",
                location=self.instructions[self.current - 1].token.location,
            )
            raise GlobalException

        total_offset = 0
        for i in range(offset):
            size = self.type_map[self.stack_types[-(i + 1)]][1]
            total_offset += size

        type_kind = self.stack_types[-(offset + 1)]
        size = self.type_map[type_kind][1]
        llvm_type = self.type_map[type_kind][0]

        temp1 = self.next_temp()
        temp2 = self.next_temp()
        temp3 = self.next_temp()
        self.ir["body_main"].extend(
            [
                f"   %t{temp1} = load ptr, ptr %stack_pointer, align 8",
                f"   %t{temp2} = getelementptr i8, ptr %t{temp1}, i32 -{total_offset}",
                f"   %t{temp2}.1 = load {llvm_type}, ptr %t{temp2}, align {min(size, 8)}",
                f"   %t{temp3} = load ptr, ptr %type_stack_pointer, align 8",
                f"   %t{temp3}.1 = getelementptr i8, ptr %t{temp3}, i32 -{offset * 4}",
                f"   %t{temp3}.2 = load i32, ptr %t{temp3}.1, align 4",
            ]
        )

        if expected_type and expected_type != type_kind and self.debug_type_checks:
            temp4 = self.next_temp()
            label1 = self.next_label()
            self.ir["body_main"].extend(
                [
                    f"   %t{temp4} = icmp eq i32 %t{temp3}.2, {expected_type.value}",
                    f"   br i1 %t{temp4}, label %{label1}, label %error",
                    f"{label1}:",
                ]
            )

        return f"%t{temp2}.1", llvm_type

    def escape_string(self, value: str, instruction: Instruction) -> str:
        result = ""
        i = 0
        while i < len(value):
            if value[i] == "\\" and i + 1 < len(value):
                i += 1
                escape = value[i]
                if escape in {
                    "n": "\n",
                    "r": "\r",
                    "t": "\t",
                    "v": "\v",
                    "b": "\b",
                    "a": "\a",
                    "f": "\f",
                    "\\": "\\",
                    '"': '"',
                }:
                    result += f"\\{ord(escape):02X}"
                else:
                    self.logger.error(
                        f"Invalid escape sequence '\\{escape}' in string literal",
                        location=instruction.token.location,
                    )
                    raise GlobalException
            elif value[i].isprintable() and value[i] not in '"\\':
                result += value[i]
            else:
                result += f"\\{ord(value[i]):02X}"
            i += 1
        return result

    def rune_to_unicode(self, value: str, instruction: Instruction) -> int:
        if len(value) == 0:
            return 0
        if value[0] == "\\" and len(value) > 1:
            escape = value[1]
            if escape in {
                "n": "\n",
                "r": "\r",
                "t": "\t",
                "v": "\v",
                "b": "\b",
                "a": "\a",
                "f": "\f",
                "\\": "\\",
                "'": "'",
            }:
                return ord(escape)
            else:
                self.logger.error(
                    f"Invalid escape sequence '\\{escape}' in rune literal",
                    location=instruction.token.location,
                )
                raise GlobalException
        return ord(value[0])

    def next_temp(self) -> str:
        temp = self.temp_count
        self.temp_count += 1
        return str(temp)

    def next_label(self) -> str:
        label = f"bb{self.label_count}"
        self.label_count += 1
        return label

    def peek(self) -> Instruction:
        return self.instructions[self.current]

    def advance(self) -> Instruction:
        instruction = self.peek()
        self.current += 1
        return instruction

    def eof(self) -> bool:
        return self.current >= len(self.instructions)


class Cli:
    def __init__(self) -> None:
        self.path = ""
        self.source = ""
        self.logger = Logger()

    def scan_file(self, path: str) -> None:
        self.path = path

        try:
            with open(self.path, "r", encoding="utf-8") as file:
                self.source = self.sanitize_source(file.read().strip())
        except Exception:
            self.logger.error(
                "No sush file or directory !",
                f"path: {self.path}",
            )
            exit(1)

        if self.source != "":
            try:
                tokenizer = Tokenizer(self.source, self.path)
                parser = Parser(self.source, self.path)
                compiler = Compiler(self.source, self.path)

                tokens = tokenizer.tokenize()
                instructions = parser.parse(tokens)
                compiler.compile(instructions)

                print(parser)
            except Exception as error:
                if not isinstance(error, GlobalException):
                    raise

    def sanitize_source(self, source: str) -> str:
        return source.replace("\t", "").replace("\r\n", "\n").replace("\r", "\n")


if __name__ == "__main__":
    cli = Cli()

    if len(sys.argv) == 2:
        cli.scan_file(sys.argv[1])
