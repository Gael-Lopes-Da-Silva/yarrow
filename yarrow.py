import os
import sys
import enum
import math


# ERRORS
class GlobalException(Exception):
    pass


# ENUMS
class TokenKind(enum.Enum):
    LEFT_PAREN = "left_parenthesis"
    RIGHT_PAREN = "right_parenthesis"
    LEFT_CURLY = "left_curly_brace"
    RIGHT_CURLY = "right_curly_brace"
    LEFT_SQUARE = "left_square_bracket"
    RIGHT_SQUARE = "right_square_bracket"

    PLUS = "plus"
    MINUS = "minus"
    STAR = "star"
    SLASH = "slash"
    SLASH_SLASH = "slash_slash"
    PERCENT = "percent"
    CARET = "caret"
    DOT = "dot"
    QUESTION = "question"

    EXCLAMATION = "exclamation"
    AMPERSAND = "ampersand"
    BAR = "bar"
    COLON = "colon"
    SEMI_COLON = "semi_colon"
    COMMA = "comma"

    EQUAL = "equal"
    EQUAL_EQUAL = "equal_equal"
    NOT_EQUAL = "not_equal"
    GREATER = "greater"
    GREATER_EQUAL = "greater_equal"
    LESS = "less"
    LESS_EQUAL = "less_equal"

    IDENTIFIER = "identifier"
    STRING = "string"
    RUNE = "rune"
    INTEGER = "integer"
    FLOAT = "float"
    BOOLEAN = "boolean"

    TYPE = "type"

    AND = "logical_and"
    OR = "logical_or"
    XOR = "logical_xor"
    NOT = "logical_not"
    LEFT_SHIFT = "left_shift"
    RIGHT_SHIFT = "right_shift"

    IF = "if"
    ELSE = "else"
    WHILE = "while"
    BREAK = "break"
    CONTINUE = "continue"
    MATCH = "match"
    CASE = "case"

    UNWRAP = "unwrap"
    HANDLE = "handle"

    FUNCTION = "function"
    RETURN = "return"
    CALL = "call"
    DO = "do"
    WITH = "with"

    CONST = "const"
    STATIC = "static"
    MUTABLE = "mutable"
    SET = "set"

    STRUCT = "struct"
    IMPLEMENT = "implement"
    ENUM = "enum"
    UNION = "union"
    NEW = "new"

    POP = "pop"
    DROP = "drop"
    DUP = "duplicate"
    OVER = "over"
    ROT = "rotate"
    SWAP = "swap"

    REQUIRE = "require"
    DEFER = "defer"
    END = "end"


class TypeKind(enum.Enum):
    I8 = "i8"
    I16 = "i16"
    I32 = "i32"
    I64 = "i64"
    I128 = "i128"
    U8 = "u8"
    U16 = "u16"
    U32 = "u32"
    U64 = "u64"
    U128 = "u128"
    F16 = "f16"
    F32 = "f32"
    F64 = "f64"
    F128 = "f128"
    BOOL = "bool"
    VOID = "void"
    ERROR = "error"
    TYPE = "type"
    STRING = "string"
    RUNE = "rune"
    ARRAY = "array"
    LIST = "list"
    HASHMAP = "hashmap"
    STACK = "stack"
    QUEUE = "queue"
    POINTER = "pointer"
    USIZE = "usize"
    ISIZE = "isize"
    C_CHAR = "c_char"
    C_SHORT = "c_short"
    C_USHORT = "c_ushort"
    C_INT = "c_int"
    C_UINT = "c_uint"
    C_LONG = "c_long"
    C_ULONG = "c_ulong"
    C_LONGLONG = "c_longlong"
    C_ULONGLONG = "c_ulonglong"
    C_DOUBLE = "c_double"
    C_LONGDOUBLE = "c_longdouble"


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
        ]

        self.keywords.extend(
            Keyword(type_kind.value, TokenKind.TYPE) for type_kind in TypeKind
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
                elif self.match("<"):
                    self.add_token(TokenKind.LEFT_SHIFT)
                else:
                    self.add_token(TokenKind.LESS)

            case ">":
                if self.match("="):
                    self.add_token(TokenKind.GREATER_EQUAL)
                elif self.match(">"):
                    self.add_token(TokenKind.RIGHT_SHIFT)
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
        self.tokens = tokens

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
                    {"type": TypeKind.TYPE, "value": TypeKind[token.lexeme.upper()]},
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

            variable_type = self.parse_type(True)
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
            return_type = self.parse_type(True)
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
                return_error = self.parse_type(True)
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

            variable_type = self.parse_type(True)
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
            union_type = self.parse_type(True)
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
        new_type = self.parse_type(True)
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

    def parse_type(self, no_error: bool = False) -> dict | None:
        variable_type = self.expect(TokenKind.TYPE) or self.expect(TokenKind.IDENTIFIER)
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
            value_type = self.parse_type(True)

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
        self.ir = []

    def __repr__(self) -> str:
        return f"{self.ir}"

    def compile(self, instructions: list) -> None:
        self.instructions = instructions
        self.output_path = os.path.splitext(self.path)[0] + ".ll"

        self.emit_preamble()
        self.emit_runtime_declaration()
        self.emit_start_main_function()

        while not self.eof():
            instruction = self.advance()
            self.translate_instruction(instruction)

        self.emit_end_main_function()

        try:
            with open(self.output_path, "w", encoding="utf-8") as file:
                for line in self.ir:
                    file.write(line + "\n")
        except Exception:
            self.logger.error(
                f"Failed to write LLVM IR to `{self.output_path}` !",
            )
            raise GlobalException

    def translate_instruction(self, instruction: Instruction) -> None:
        match instruction.kind:
            case "push":
                pass

            case _:
                self.logger.warning(
                    f"Unsupported instruction '{instruction.kind}'",
                    location=instruction.token.location,
                )

    def emit_preamble(self) -> None:
        self.ir.extend(
            [
                "; ModuleID = 'yarrow'",
                f"source_filename = '{self.path}'",
                "target datalayout = 'e-m:e-p270:32:32-p271:32:32-p272:64:64-i64:64-f80:128-n8:16:32:64-S128'",
                "target triple = 'x86_64-pc-linux-gnu'",
            ]
        )

    def emit_runtime_declaration(self) -> None:
        self.ir.extend([])

    def emit_start_main_function(self) -> None:
        self.ir.extend(
            [
                "define i32 @main() #0 {",
                "entry:",
                "   %stack = alloca [1024 * i8], align 16",
                "   %stack_pointer = alloca i8*, align 8",
                "   store i8* %stack, i8** %stack_pointer, align 8",
            ]
        )

    def emit_end_main_function(self) -> None:
        self.ir.extend(
            [
                "   ret i32 0",
                "}",
                "attributes #0 = { noinline nounwind optnone uwtable }",
            ]
        )

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

                compiler.compile(parser.parse(tokenizer.tokenize()))

                print(compiler)
            except Exception as error:
                if not isinstance(error, GlobalException):
                    raise

    def sanitize_source(self, source: str) -> str:
        return source.replace("\t", "").replace("\r\n", "\n").replace("\r", "\n")


if __name__ == "__main__":
    cli = Cli()

    if len(sys.argv) == 2:
        cli.scan_file(sys.argv[1])
