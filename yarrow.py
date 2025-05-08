import os
import sys
import enum
import math


# ERRORS
class GlobalException(Exception):
    pass


# ENUMS
class TokenKind(enum.Enum):
    LEFT_PAREN = enum.auto()
    RIGHT_PAREN = enum.auto()
    LEFT_CURLY = enum.auto()
    RIGHT_CURLY = enum.auto()
    LEFT_SQUARE = enum.auto()
    RIGHT_SQUARE = enum.auto()

    PLUS = enum.auto()
    MINUS = enum.auto()
    STAR = enum.auto()
    SLASH = enum.auto()
    SLASH_SLASH = enum.auto()
    PERCENT = enum.auto()
    CARET = enum.auto()
    DOT = enum.auto()
    QUESTION = enum.auto()

    EXCLAMATION = enum.auto()
    AMPERSAND = enum.auto()
    BAR = enum.auto()
    COLON = enum.auto()
    SEMI_COLON = enum.auto()
    COMMA = enum.auto()

    EQUAL = enum.auto()
    EQUAL_EQUAL = enum.auto()
    NOT_EQUAL = enum.auto()
    GREATER = enum.auto()
    GREATER_EQUAL = enum.auto()
    LESS = enum.auto()
    LESS_EQUAL = enum.auto()

    IDENTIFIER = enum.auto()
    STRING = enum.auto()
    RUNE = enum.auto()
    INTEGER = enum.auto()
    FLOAT = enum.auto()
    BOOLEAN = enum.auto()

    TYPE = enum.auto()

    AND = enum.auto()
    OR = enum.auto()
    XOR = enum.auto()
    NOT = enum.auto()
    LEFT_SHIFT = enum.auto()
    RIGHT_SHIFT = enum.auto()

    IF = enum.auto()
    ELSE = enum.auto()
    WHILE = enum.auto()
    FOR = enum.auto()
    BREAK = enum.auto()
    CONTINUE = enum.auto()
    MATCH = enum.auto()
    CASE = enum.auto()

    UNWRAP = enum.auto()
    HANDLE = enum.auto()

    FUNCTION = enum.auto()
    RETURN = enum.auto()
    CALL = enum.auto()
    DO = enum.auto()
    WITH = enum.auto()

    CONST = enum.auto()
    STATIC = enum.auto()
    MUTABLE = enum.auto()
    SET = enum.auto()

    STRUCT = enum.auto()
    IMPLEMENT = enum.auto()
    ENUM = enum.auto()
    UNION = enum.auto()

    POP = enum.auto()
    DROP = enum.auto()
    DUP = enum.auto()
    OVER = enum.auto()
    ROT = enum.auto()
    SWAP = enum.auto()

    REQUIRE = enum.auto()
    DEFER = enum.auto()
    END = enum.auto()


class TypeKind(enum.Enum):
    I8 = enum.auto()
    I16 = enum.auto()
    I32 = enum.auto()
    I64 = enum.auto()
    I128 = enum.auto()
    U8 = enum.auto()
    U16 = enum.auto()
    U32 = enum.auto()
    U64 = enum.auto()
    U128 = enum.auto()
    F16 = enum.auto()
    F32 = enum.auto()
    F64 = enum.auto()
    F128 = enum.auto()
    BOOL = enum.auto()
    VOID = enum.auto()
    ERROR = enum.auto()
    TYPE = enum.auto()
    STRING = enum.auto()
    RUNE = enum.auto()
    ARRAY = enum.auto()
    LIST = enum.auto()
    HASHMAP = enum.auto()
    STACK = enum.auto()
    QUEUE = enum.auto()
    POINTER = enum.auto()
    REFERENCE = enum.auto()
    USIZE = enum.auto()
    ISIZE = enum.auto()
    C_CHAR = enum.auto()
    C_SHORT = enum.auto()
    C_USHORT = enum.auto()
    C_INT = enum.auto()
    C_UINT = enum.auto()
    C_LONG = enum.auto()
    C_ULONG = enum.auto()
    C_LONGLONG = enum.auto()
    C_ULONGLONG = enum.auto()
    C_DOUBLE = enum.auto()
    C_LONGDOUBLE = enum.auto()


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
            Keyword("for", TokenKind.FOR),
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
            case TokenKind.FOR:
                return self.handle_fors(token)
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
            if self.peek().kind != TokenKind.TYPE:
                self.logger.error(
                    "Invalid function syntax !",
                    location=self.peek().location,
                    location_message="parameters are only composed of types",
                )
                raise GlobalException

            parameter_type = self.parse_type(no_error=True)
            parameters.append(parameter_type)

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
                location_message="you need to close a while statement with `end`",
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

    def handle_fors(self, token: Token) -> Instruction:
        body = []
        while not self.eof() and self.peek().kind != TokenKind.END:
            instruction = self.parse_instruction()
            if instruction is not None:
                body.append(instruction)

        if self.eof() or self.expect(TokenKind.END) is None:
            self.logger.error(
                "For statement not closed !",
                location=token.location,
                location_message="you need to close a for statement with `end`",
            )
            raise GlobalException

        return Instruction(
            "for",
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

        if abs_value < 1e-307:
            return TypeKind.F16

        significant_digits = 0
        if abs_value != 0.0:
            order = math.floor(math.log10(abs_value))
            normalized = abs_value / (10**order)

            temp = normalized
            epsilon = 1e-10
            max_digits = 34
            while temp > epsilon and significant_digits < max_digits:
                temp *= 10
                digit = int(temp)
                significant_digits += 1
                temp -= digit
                if abs(temp) < epsilon:
                    break

        if significant_digits > 0:
            str_value = f"{abs_value:.16e}".split("e")[0].replace(".", "").rstrip("0")
            significant_digits = min(significant_digits, len(str_value))

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
        self.parameter_count = 0
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
                continue

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

    def translate_instruction(self, instruction: Instruction) -> None:
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
                        self.ir["body_main"].extend(
                            [f"   %t{temp1} = add {llvm_type} 0, {value}"]
                            + self.emit_push(
                                llvm_type, f"%t{temp1}", value_type, llvm_size
                            )
                        )

                    case TypeKind.F16 | TypeKind.F32 | TypeKind.F64 | TypeKind.F128:
                        temp1 = self.next_temp()
                        llvm_type = self.type_map[value_type][0]
                        llvm_size = self.type_map[value_type][1]
                        self.ir["body_main"].extend(
                            [f"   %t{temp1} = fadd {llvm_type} 0.0, {value}"]
                            + self.emit_push(
                                llvm_type, f"%t{temp1}", value_type, llvm_size
                            )
                        )

                    case TypeKind.BOOL:
                        temp1 = self.next_temp()
                        llvm_type = self.type_map[value_type][0]
                        llvm_size = self.type_map[value_type][1]
                        self.ir["body_main"].extend(
                            [
                                f"   %t{temp1} = add {llvm_type} 0, {'1' if value else '0'}"
                            ]
                            + self.emit_push(
                                llvm_type, f"%t{temp1}", value_type, llvm_size
                            )
                        )

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
                            + self.emit_push(
                                llvm_type, f"%t{temp3}", value_type, llvm_size
                            )
                        )

                    case TypeKind.RUNE:
                        rune_value = self.rune_to_unicode(value, instruction)
                        temp1 = self.next_temp()
                        llvm_type = self.type_map[value_type][0]
                        llvm_size = self.type_map[value_type][1]
                        self.ir["body_main"].extend(
                            [f"   %t{temp1} = add {llvm_type} 0, {rune_value}"]
                            + self.emit_push(
                                llvm_type, f"%t{temp1}", value_type, llvm_size
                            )
                        )

                    case TypeKind.TYPE:
                        temp1 = self.next_temp()
                        type_id = self.next_temp()
                        type_index = TypeKind[value["type"].lexeme.upper()].value
                        llvm_type = self.type_map[value_type][0]
                        llvm_size = self.type_map[value_type][1]
                        self.ir["global_declarations"].append(
                            f"@.type.{type_id} = private unnamed_addr constant {{ i32, ptr }} {{ i32 {type_index}, ptr null }}"
                        )
                        self.ir["body_main"].extend(
                            [
                                f"   %t{temp1} = getelementptr {{ i32, ptr }}, {{ i32, ptr }}* @.type.{type_id}, i32 0, i32 0"
                            ]
                            + self.emit_push(
                                llvm_type, f"%t{temp1}", value_type, llvm_size
                            )
                        )

                    case TypeKind.VOID:
                        llvm_type = self.type_map[value_type][0]
                        llvm_size = self.type_map[value_type][1]
                        self.ir["body_main"].extend(
                            self.emit_push(
                                llvm_type, "null", value_type, llvm_size, value
                            )
                        )

                    case _:
                        self.logger.warning(
                            f"Unsupported push type '{value_type}' for value '{value}'",
                            location=instruction.token.location,
                        )

            case "call":
                self.emit_call(instruction)

            case "mutable" | "const" | "static":
                self.emit_variable(instruction)

            case "pop":
                self.ir["body_main"].extend(self.emit_pop())
            case "drop":
                self.ir["body_main"].extend(self.emit_drop())
            case "dup":
                self.ir["body_main"].extend(self.emit_dup())
            case "swap":
                self.ir["body_main"].extend(self.emit_swap())
            case "over":
                self.ir["body_main"].extend(self.emit_over())
            case "rot":
                self.ir["body_main"].extend(self.emit_rot())

            case "addition":
                self.emit_addition(instruction)
            case "subtraction":
                self.emit_subtraction(instruction)

            case _:
                self.logger.warning(
                    f"Unsupported instruction '{instruction.kind}'",
                    location=instruction.token.location,
                )

    def emit_variable(self, instruction: Instruction) -> None:
        if len(self.stack_types) < 2:
            self.logger.error(
                "Variable declaration requires at least two stack elements (value and identifier)",
                location=instruction.token.location,
            )
            raise GlobalException

        identifier_entry = self.stack_types[-1]
        value_entry = self.stack_types[-2]

        if identifier_entry["type"] != TypeKind.VOID:
            self.logger.error(
                "Top stack element must be an identifier for variable declaration",
                location=instruction.token.location,
            )
            raise GlobalException

        variable_name = identifier_entry["value"]
        value_type = value_entry["type"]
        value = value_entry["value"]
        declared_type = TypeKind[instruction.content["value"]["type"].lexeme.upper()]

        if variable_name in self.stack_symbols:
            self.logger.error(
                f"Variable '{variable_name}' already defined",
                location=instruction.token.location,
                location_message="variable identifiers must be unique",
            )
            raise GlobalException

        if value_type != declared_type:
            self.logger.error(
                f"Type mismatch: variable '{variable_name}' declared as {declared_type}, but value is {value_type}",
                location=instruction.token.location,
            )
            raise GlobalException

        if instruction.kind == "static":
            if value_type in [
                TypeKind.STRING,
                TypeKind.LIST,
                TypeKind.HASHMAP,
                TypeKind.ARRAY,
            ]:
                self.logger.error(
                    f"Static variable '{variable_name}' cannot have runtime type {value_type}",
                    location=instruction.token.location,
                    location_message="static variables must have compile-time known values",
                )
                raise GlobalException

        llvm_type, size = self.type_map[declared_type]
        symbol = {
            "kind": "variable",
            "type": declared_type,
            "llvm_type": llvm_type,
            "size": size,
            "modifier": instruction.kind,
        }

        if instruction.kind == "static":
            global_id = self.next_temp()
            self.ir["global_declarations"].append(
                f"@{variable_name} = private constant {llvm_type} {value}"
            )
            symbol["address"] = f"@{variable_name}"
        else:
            temp1 = self.next_temp()
            self.ir["body_main"].extend(
                [
                    f"   %{variable_name} = alloca {llvm_type}, align {min(size, 8)}",
                    f"   %t{temp1} = load ptr, ptr %stack_pointer, align 8",
                    f"   %t{temp1}.1 = getelementptr i8, ptr %t{temp1}, i32 -{size}",
                    f"   %t{temp1}.2 = load {llvm_type}, ptr %t{temp1}.1, align {min(size, 8)}",
                    f"   store {llvm_type} %t{temp1}.2, ptr %{variable_name}, align {min(size, 8)}",
                ]
            )
            symbol["address"] = f"%{variable_name}"

        self.stack_symbols[variable_name] = symbol
        self.ir["body_main"].extend(self.emit_pop())
        self.ir["body_main"].extend(self.emit_pop())

    def emit_call(self, instruction: Instruction) -> None:
        if not self.stack_types:
            self.logger.error(
                "Function call requires at least a function name on the stack",
                location=instruction.token.location,
            )
            raise GlobalException

        function_entry = self.stack_types[-1]
        if function_entry["type"] != TypeKind.VOID:
            self.logger.error(
                "Top stack element must be a function identifier for call",
                location=instruction.token.location,
            )
            raise GlobalException

        function_name = function_entry["value"]
        self.ir["body_main"].extend(self.emit_pop())

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
        expected_param_count = len(function_data["parameters"])
        if len(self.stack_types) < expected_param_count:
            self.logger.error(
                f"Function '{function_name}' requires {expected_param_count} parameters, but stack has only {len(self.stack_types)} elements",
                location=instruction.token.location,
            )
            raise GlobalException

        parameters = []
        total_size = 0
        for index, parameter in enumerate(reversed(function_data["parameters"])):
            param_entry = self.stack_types[-(index + 1)]
            expected_type = parameter["type_kind"]

            if (
                param_entry["type"] == TypeKind.VOID
                and param_entry["value"] in self.stack_symbols
            ):
                variable = self.stack_symbols[param_entry["value"]]
                if variable["kind"] != "variable":
                    self.logger.error(
                        f"Identifier '{param_entry['value']}' is not a variable",
                        location=instruction.token.location,
                    )
                    raise GlobalException
                if variable["type"] != expected_type:
                    self.logger.error(
                        f"Type mismatch for parameter {len(parameters) + 1}: expected {expected_type}, got {variable['type']}",
                        location=instruction.token.location,
                    )
                    raise GlobalException
                if variable["modifier"] == "const" and variable["type"] not in [
                    TypeKind.I8,
                    TypeKind.U8,
                    TypeKind.I16,
                    TypeKind.U16,
                    TypeKind.I32,
                    TypeKind.U32,
                    TypeKind.I64,
                    TypeKind.U64,
                    TypeKind.I128,
                    TypeKind.U128,
                    TypeKind.F16,
                    TypeKind.F32,
                    TypeKind.F64,
                    TypeKind.F128,
                    TypeKind.BOOL,
                    TypeKind.RUNE,
                ]:
                    self.logger.error(
                        f"Cannot pass const variable '{param_entry['value']}' of non-copyable type {variable['type']} as parameter",
                        location=instruction.token.location,
                    )
                    raise GlobalException

                temp1 = self.next_temp()
                size = variable["size"]
                total_size += size
                self.ir["body_main"].extend(
                    [
                        f"   %t{temp1} = load {variable['llvm_type']}, ptr {variable['address']}, align {min(size, 8)}",
                    ]
                )
                parameters.append(f"{variable['llvm_type']} %t{temp1}")
            else:
                actual_type = param_entry["type"]
                if actual_type != expected_type:
                    self.logger.error(
                        f"Type mismatch for parameter {len(parameters) + 1}: expected {expected_type}, got {actual_type}",
                        location=instruction.token.location,
                    )
                    raise GlobalException

                size = self.type_map[expected_type][1]
                total_size += size
                temp1 = self.next_temp()
                self.ir["body_main"].extend(
                    [
                        f"   %t{temp1} = load ptr, ptr %stack_pointer, align 8",
                        f"   %t{temp1}.1 = getelementptr i8, ptr %t{temp1}, i32 -{total_size}",
                        f"   %t{temp1}.2 = load {parameter['llvm_type']}, ptr %t{temp1}.1, align {min(size, 8)}",
                    ]
                )
                parameters.append(f"{parameter['llvm_type']} %t{temp1}.2")

            self.ir["body_main"].extend(self.emit_pop())

        return_type = function_data["return_type"]["llvm_type"]
        return_type_kind = function_data["return_type"]["type_kind"]
        call = f"call {return_type} @{function_name}({', '.join(parameters)})"

        if return_type != "void":
            temp3 = self.next_temp()
            size = self.type_map[return_type_kind][1]
            self.ir["body_main"].extend(
                [
                    f"   %t{temp3} = {call}",
                    f"   %t{temp3}.1 = load ptr, ptr %stack_pointer, align 8",
                    f"   store {return_type} %t{temp3}, ptr %t{temp3}.1, align {min(size, 8)}",
                    f"   %t{temp3}.2 = getelementptr i8, ptr %t{temp3}.1, i32 {size}",
                    f"   store ptr %t{temp3}.2, ptr %stack_pointer, align 8",
                    f"   %t{temp3}.3 = load ptr, ptr %type_stack_pointer, align 8",
                    f"   store i32 {return_type_kind.value}, ptr %t{temp3}.3, align 4",
                    f"   %t{temp3}.4 = getelementptr i8, ptr %t{temp3}.3, i32 4",
                    f"   store ptr %t{temp3}.4, ptr %type_stack_pointer, align 8",
                ]
            )
            self.stack_types.append({"type": return_type_kind, "value": None})
        else:
            self.ir["body_main"].append(f"   {call}")

    def emit_function(self, instruction: Instruction) -> None:
        if (
            self.current <= 0
            or self.instructions[self.current - 1].kind != "push"
            or self.instructions[self.current - 1].content["type"] != TypeKind.VOID
        ):
            self.logger.error(
                "Function must be preceded by an identifier",
                location=instruction.token.location,
                location_message="expected function name before declaration",
            )
            raise GlobalException

        function_name = self.instructions[self.current - 1].content["value"]

        if (
            function_name in self.stack_symbols
            and self.stack_symbols[function_name].kind == "function"
        ):
            self.logger.error(
                f"Function '{function_name}' already defined",
                location=instruction.token.location,
                location_message="function identifiers are unique",
            )
            raise GlobalException

        function_data = instruction.content["value"]
        return_type_kind = (
            TypeKind[function_data["return_type"]["type"].lexeme.upper()]
            if function_data["return_type"]
            else TypeKind.VOID
        )
        return_type = (
            self.type_map[return_type_kind][0]
            if function_data["return_type"]
            else "void"
        )

        if return_type != "void":
            has_return = False
            for body_instruction in function_data["body"]:
                if body_instruction.kind == "return":
                    has_return = True
                    break

            if not has_return:
                self.logger.error(
                    f"Function '{function_name}' with non-void return type '{return_type}' must contain a return statement",
                    location=instruction.token.location,
                    location_message="add a return statement in the function body",
                )
                raise GlobalException

        parameters = []
        for parameter in function_data["parameters"]:
            parameters.append(
                {
                    "name": self.next_parameter(),
                    "llvm_type": self.type_map[
                        TypeKind[parameter["type"].lexeme.upper()]
                    ][0],
                    "type_kind": TypeKind[parameter["type"].lexeme.upper()],
                }
            )

        self.stack_symbols[function_name] = {
            "kind": "function",
            "parameters": parameters,
            "return_type": {
                "llvm_type": return_type,
                "type_kind": return_type_kind,
            },
        }

        ir = {
            "start_function": [
                f"define {return_type} @{function_name}({', '.join([f'{parameter["llvm_type"]} %{parameter["name"]}' for parameter in parameters])}) #0 {{",
                "entry:",
                "   %stack = alloca [1024 x i8], align 16",
                "   %stack_pointer = alloca ptr, align 8",
                "   %type_stack = alloca [1024 x i32], align 4",
                "   %type_stack_pointer = alloca ptr, align 8",
                "   store ptr %stack, ptr %stack_pointer, align 8",
                "   store ptr %type_stack, ptr %type_stack_pointer, align 8",
                "   %stack_end = getelementptr [1024 x i8], [1024 x i8]* %stack, i32 0, i32 1024",
            ],
            "body_function": [],
            "end_function": [
                "error:",
                "   call void @llvm.trap()",
                "   unreachable",
                "}",
            ],
        }

        temp_stack_types = self.stack_types
        temp_main_body = self.ir["body_main"]
        self.stack_types = []
        self.ir["body_main"] = []

        for parameter in parameters:
            size = self.type_map[parameter["type_kind"]][1]
            self.ir["body_main"].extend(
                self.emit_push(
                    parameter["llvm_type"],
                    f"%{parameter['name']}",
                    parameter["type_kind"],
                    size,
                )
            )

        for body_instruction in function_data["body"]:
            if body_instruction.kind == "return":
                if return_type != "void":
                    if not self.stack_types:
                        self.logger.error(
                            "Return statement with empty stack in function with non-void return type",
                            location=body_instruction.token.location,
                            location_message=f"expected a value of type {return_type_kind} on the stack",
                        )
                        raise GlobalException

                    top_entry = self.stack_types[-1]
                    top_type = top_entry["type"]
                    top_type_size = self.type_map[top_type][1]

                    if (
                        top_type == TypeKind.VOID
                        and top_entry["value"] in self.stack_symbols
                    ):
                        variable = self.stack_symbols[top_entry["value"]]
                        if variable["kind"] != "variable":
                            self.logger.error(
                                f"Identifier '{top_entry['value']}' is not a variable",
                                location=body_instruction.token.location,
                            )
                            raise GlobalException
                        if variable["type"] != return_type_kind:
                            self.logger.error(
                                f"Type mismatch in return statement: expected {return_type_kind}, got {variable['type']}",
                                location=body_instruction.token.location,
                            )
                            raise GlobalException

                        temp1 = self.next_temp()
                        self.ir["body_main"].extend(
                            [
                                f"   %t{temp1} = load {variable['llvm_type']}, ptr {variable['address']}, align {min(top_type_size, 8)}",
                                f"   ret {return_type} %t{temp1}",
                            ]
                        )
                    else:
                        if top_type != return_type_kind:
                            self.logger.error(
                                f"Type mismatch in return statement: expected {return_type_kind}, got {top_type}",
                                location=body_instruction.token.location,
                            )
                            raise GlobalException

                        temp1 = self.next_temp()
                        self.ir["body_main"].extend(
                            [
                                f"   %t{temp1} = load ptr, ptr %stack_pointer, align 8",
                                f"   %t{temp1}.1 = getelementptr i8, ptr %t{temp1}, i32 -{top_type_size}",
                                f"   %t{temp1}.2 = load {return_type}, ptr %t{temp1}.1, align {min(top_type_size, 8)}",
                                f"   ret {return_type} %t{temp1}.2",
                            ]
                        )
                else:
                    if self.stack_types:
                        self.logger.warning(
                            "Return statement with non-empty stack in void function",
                            location=body_instruction.token.location,
                            location_message="stack values will be ignored",
                        )
                    self.ir["body_main"].append("   ret void")
                continue

            self.translate_instruction(body_instruction)

        ir["body_function"].extend(self.ir["body_main"])
        self.stack_types = temp_stack_types
        self.ir["body_main"] = temp_main_body

        if return_type == "void" and not any(
            i.kind == "return" for i in function_data["body"]
        ):
            ir["end_function"] = ["   ret void"] + ir["end_function"]

        self.ir["functions"].extend(
            [line for section in ir.values() for line in section]
        )

    def emit_push(
        self,
        llvm_type: str,
        value: str,
        type_kind: TypeKind,
        size: int,
        push_value: any = None,
    ) -> list:
        temp1 = self.next_temp()
        temp2 = self.next_temp()
        temp3 = self.next_temp()
        temp4 = self.next_temp()
        label1 = self.next_label()
        label2 = self.next_label()
        self.stack_types.append({"type": type_kind, "value": push_value})
        return [
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
        size = self.type_map[self.stack_types.pop()["type"]][1]
        return [
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

    def emit_drop(self) -> list:
        self.stack_types.clear()
        return [
            "   store ptr %stack, ptr %stack_pointer, align 8",
            "   store ptr %type_stack, ptr %type_stack_pointer, align 8",
        ]

    def emit_dup(self) -> list:
        if not self.stack_types:
            self.logger.error(
                "Dup operation on empty stack",
                location=self.instructions[self.current - 1].token.location,
            )
            raise GlobalException

        type_kind = self.stack_types[-1]["type"]
        llvm_type, size = self.type_map[type_kind]
        value, _ = self.load_typed_value(0, type_kind)
        temp1 = self.next_temp()
        self.stack_types.append(self.stack_types[-1])
        return [
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

    def emit_swap(self) -> list:
        if len(self.stack_types) < 2:
            self.logger.error(
                "Swap operation requires at least two stack elements",
                location=self.instructions[self.current - 1].token.location,
            )
            raise GlobalException

        type_kind1 = self.stack_types[-1]["type"]
        size1 = self.type_map[type_kind1][1]
        type_kind2 = self.stack_types[-2]["type"]
        size2 = self.type_map[type_kind2][1]
        value1, llvm_type1 = self.load_typed_value(0, type_kind1)
        value2, llvm_type2 = self.load_typed_value(1, type_kind2)
        temp1 = self.next_temp()
        temp2 = self.next_temp()
        self.stack_types[-1], self.stack_types[-2] = (
            self.stack_types[-2],
            self.stack_types[-1],
        )
        return [
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

    def emit_over(self) -> list:
        if len(self.stack_types) < 2:
            self.logger.error(
                "Over operation requires at least two stack elements",
                location=self.instructions[self.current - 1].token.location,
            )
            raise GlobalException

        type_kind = self.stack_types[-2]["type"]
        size = self.type_map[type_kind][1]
        value, llvm_type = self.load_typed_value(1, type_kind)
        temp1 = self.next_temp()
        self.stack_types.append(self.stack_types[-2])
        return [
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

    def emit_rot(self) -> None:
        if len(self.stack_types) < 3:
            self.logger.error(
                "Rot operation requires at least three stack elements",
                location=self.instructions[self.current - 1].token.location,
            )
            raise GlobalException

        type_kind1 = self.stack_types[-1]["type"]
        size1 = self.type_map[type_kind1][1]
        type_kind2 = self.stack_types[-2]["type"]
        size2 = self.type_map[type_kind2][1]
        type_kind3 = self.stack_types[-3]["type"]
        size3 = self.type_map[type_kind3][1]
        value1, llvm_type1 = self.load_typed_value(0, type_kind1)
        value2, llvm_type2 = self.load_typed_value(1, type_kind2)
        value3, llvm_type3 = self.load_typed_value(2, type_kind3)
        temp1 = self.next_temp()
        temp2 = self.next_temp()
        temp3 = self.next_temp()
        self.stack_types[-3:] = [
            self.stack_types[-2],
            self.stack_types[-3],
            self.stack_types[-1],
        ]
        return [
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

    def emit_addition(self, instruction: Instruction) -> None:
        if len(self.stack_types) < 2:
            self.logger.error(
                "Addition requires at least two stack elements",
                location=instruction.token.location,
            )
            raise GlobalException

        type2, value2 = self.stack_types[-1], self.stack_types[-1]["value"]
        type1, value1 = self.stack_types[-2], self.stack_types[-2]["value"]

        if type1 == TypeKind.VOID and value1 in self.stack_symbols:
            variable = self.stack_symbols[value1]
            if variable["kind"] != "variable":
                self.logger.error(
                    f"Identifier '{value1}' is not a variable",
                    location=instruction.token.location,
                )
                raise GlobalException
            type1 = variable["type"]
            value1 = self.next_temp()
            self.ir["body_main"].extend(
                [
                    f"   %t{value1} = load {variable['llvm_type']}, ptr {variable['address']}, align {min(variable['size'], 8)}"
                ]
            )
        else:
            value1, llvm_type1 = self.load_typed_value(0, type1["type"])

        if type2 == TypeKind.VOID and value2 in self.stack_symbols:
            variable = self.stack_symbols[value2]
            if variable["kind"] != "variable":
                self.logger.error(
                    f"Identifier '{value2}' is not a variable",
                    location=instruction.token.location,
                )
                raise GlobalException
            type2 = variable["type"]
            value2 = self.next_temp()
            self.ir["body_main"].extend(
                [
                    f"   %t{value2} = load {variable['llvm_type']}, ptr {variable['address']}, align {min(variable['size'], 8)}"
                ]
            )
        else:
            value2, llvm_type2 = self.load_typed_value(0, type2["type"])

        if type1 != type2:
            self.logger.error(
                f"Type mismatch in addition: {type1} and {type2}",
                location=instruction.token.location,
            )
            raise GlobalException

        if type1 not in [
            TypeKind.I8,
            TypeKind.U8,
            TypeKind.I16,
            TypeKind.U16,
            TypeKind.I32,
            TypeKind.U32,
            TypeKind.I64,
            TypeKind.U64,
            TypeKind.I128,
            TypeKind.U128,
            TypeKind.F16,
            TypeKind.F32,
            TypeKind.F64,
            TypeKind.F128,
        ]:
            self.logger.error(
                f"Addition not supported for type {type1}",
                location=instruction.token.location,
            )
            raise GlobalException

        llvm_type, size = self.type_map[type1]
        temp1 = self.next_temp()
        operation = (
            "add"
            if type1
            in [
                TypeKind.I8,
                TypeKind.U8,
                TypeKind.I16,
                TypeKind.U16,
                TypeKind.I32,
                TypeKind.U32,
                TypeKind.I64,
                TypeKind.U64,
                TypeKind.I128,
                TypeKind.U128,
            ]
            else "fadd"
        )

        self.ir["body_main"].extend(
            [f"   %t{temp1} = {operation} {llvm_type} {value1}, {value2}"]
            + self.emit_push(llvm_type, f"%t{temp1}", type1, size)
        )
        self.ir["body_main"].extend(self.emit_pop())
        self.ir["body_main"].extend(self.emit_pop())

    def emit_subtraction(self, instruction: Instruction) -> None:
        if len(self.stack_types) < 2:
            self.logger.error(
                "Subtraction requires at least two stack elements",
                location=instruction.token.location,
            )
            raise GlobalException

        type2, value2 = self.stack_types[-1], self.stack_types[-1]["value"]
        type1, value1 = self.stack_types[-2], self.stack_types[-2]["value"]

        if type1 == TypeKind.VOID and value1 in self.stack_symbols:
            variable = self.stack_symbols[value1]
            if variable["kind"] != "variable":
                self.logger.error(
                    f"Identifier '{value1}' is not a variable",
                    location=instruction.token.location,
                )
                raise GlobalException
            type1 = variable["type"]
            value1 = self.next_temp()
            self.ir["body_main"].extend(
                [
                    f"   %t{value1} = load {variable['llvm_type']}, ptr {variable['address']}, align {min(variable['size'], 8)}"
                ]
            )
        else:
            value1, llvm_type1 = self.load_typed_value(0, type1["type"])

        if type2 == TypeKind.VOID and value2 in self.stack_symbols:
            variable = self.stack_symbols[value2]
            if variable["kind"] != "variable":
                self.logger.error(
                    f"Identifier '{value2}' is not a variable",
                    location=instruction.token.location,
                )
                raise GlobalException
            type2 = variable["type"]
            value2 = self.next_temp()
            self.ir["body_main"].extend(
                [
                    f"   %t{value2} = load {variable['llvm_type']}, ptr {variable['address']}, align {min(variable['size'], 8)}"
                ]
            )
        else:
            value2, llvm_type2 = self.load_typed_value(0, type2["type"])

        if type1 != type2:
            self.logger.error(
                f"Type mismatch in subtraction: {type1} and {type2}",
                location=instruction.token.location,
            )
            raise GlobalException

        if type1 not in [
            TypeKind.I8,
            TypeKind.U8,
            TypeKind.I16,
            TypeKind.U16,
            TypeKind.I32,
            TypeKind.U32,
            TypeKind.I64,
            TypeKind.U64,
            TypeKind.I128,
            TypeKind.U128,
            TypeKind.F16,
            TypeKind.F32,
            TypeKind.F64,
            TypeKind.F128,
        ]:
            self.logger.error(
                f"Subtraction not supported for type {type1}",
                location=instruction.token.location,
            )
            raise GlobalException

        llvm_type, size = self.type_map[type1]
        temp1 = self.next_temp()
        operation = (
            "sub"
            if type1
            in [
                TypeKind.I8,
                TypeKind.U8,
                TypeKind.I16,
                TypeKind.U16,
                TypeKind.I32,
                TypeKind.U32,
                TypeKind.I64,
                TypeKind.U64,
                TypeKind.I128,
                TypeKind.U128,
            ]
            else "fsub"
        )

        self.ir["body_main"].extend(
            [f"   %t{temp1} = {operation} {llvm_type} {value1}, {value2}"]
            + self.emit_push(llvm_type, f"%t{temp1}", type1, size)
        )
        self.ir["body_main"].extend(self.emit_pop())
        self.ir["body_main"].extend(self.emit_pop())

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
            size = self.type_map[self.stack_types[-(i + 1)]["type"]][1]
            total_offset += size

        type_kind = self.stack_types[-(offset + 1)]["type"]
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

        if expected_type and expected_type != type_kind:
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

    def next_parameter(self) -> str:
        parameter = f"pp{self.parameter_count}"
        self.parameter_count += 1
        return parameter

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
            except Exception as error:
                if not isinstance(error, GlobalException):
                    raise

    def sanitize_source(self, source: str) -> str:
        return source.replace("\t", "").replace("\r\n", "\n").replace("\r", "\n")


if __name__ == "__main__":
    cli = Cli()

    if len(sys.argv) == 2:
        cli.scan_file(sys.argv[1])
