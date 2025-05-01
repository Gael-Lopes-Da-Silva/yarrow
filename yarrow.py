import sys

from enum import Enum
from dataclasses import dataclass

# GLOBALS
SOURCE: str = ""
PATH: str = ""


# ERRORS
class GlobalException(Exception):
    pass


# ENUMS
class TokenKind(Enum):
    LEFT_PAREN = "left_parenthesis"
    RIGHT_PAREN = "right_parenthesis"
    LEFT_CURLY = "left_curly_brace"
    RIGHT_CURLY = "right_curly_brace"
    LEFT_SQUARE = "left_square_bracket"
    RIGHT_SQUARE = "right_square_bracket"
    COMMA = "comma"
    DOT = "dot"

    PLUS = "plus"
    MINUS = "minus"
    MULTIPLICATION = "multiplication"
    DIVISION = "division"
    EUCLIDIAN = "euclidian_division"
    REMAINDER = "remainder"
    POWER = "power"

    EQUAL_EQUAL = "equal"
    NOT_EQUAL = "not_equal"
    GREATER = "greater"
    GREATER_EQUAL = "greater_equal"
    LESS = "less"
    LESS_EQUAL = "less_equal"

    BITWISE_AND = "bitwise_and"
    BITWISE_OR = "bitwise_or"
    BITWISE_XOR = "bitwise_xor"
    BITWISE_NOT = "bitwise_not"
    LEFT_SHIFT = "left_shift"
    RIGHT_SHIFT = "right_shift"

    IDENTIFIER = "identifier"
    STRING = "string"
    INTEGER = "integer"
    FLOAT = "float"
    BOOLEAN = "boolean"

    TYPE = "type"

    AND = "logical_and"
    OR = "logical_or"
    NOT = "logical_not"

    IF = "if"
    ELSE = "else"
    WHILE = "while"
    BREAK = "break"
    CONTINUE = "continue"
    MATCH = "match"
    CASE = "case"

    TRY = "try"
    CATCH = "catch"

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

    POP = "pop"
    DROP = "drop"
    DUP = "duplicate"
    OVER = "over"
    ROT = "rotate"
    SWAP = "swap"

    REQUIRE = "require"
    DEFER = "defer"
    END = "end"


class TypeKind(Enum):
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


# DATACLASSES
@dataclass
class Color:
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


@dataclass
class Location:
    line: int
    start: int
    end: int


@dataclass
class Token:
    kind: TokenKind
    lexeme: str
    location: Location


@dataclass
class Keyword:
    name: str
    token: TokenKind


@dataclass
class Instruction:
    kind: str
    content: any
    token: Token


# CLASSES
class Logger:
    pointer: str = "─"

    def error(message: str, *args: any, location: Location = None, location_message: str = None) -> None:
        output = f"[{Color.BOLD}{Color.RED}ERROR{Color.RESET}] {message}"

        if location:
            line = f"{location.line}│ {SOURCE.splitlines()[location.line - 1]}"
            pointer = " " * (len(str(location.line)) + 2 + location.start) + Logger.pointer
            output += f"{Color.GREY}\n| location: {PATH}:{location.line}:{location.start}\n|   {line}\n|   {Color.RED}{pointer * max(1, location.end - location.start)}"
            output += f" {location_message}{Color.RESET}" if location_message is not None else f"{Color.RESET}"

        for arg in args:
            output += f"{Color.GREY}\n| {arg}{Color.RESET}"

        print(f"{output}")

    def warning(message: str, *args: any, location: Location = None, location_message: str = None) -> None:
        output = f"[{Color.BOLD}{Color.YELLOW}WARNING{Color.RESET}] {message}"

        if location:
            line = f"{location.line}│ {SOURCE.splitlines()[location.line - 1]}"
            pointer = " " * (len(str(location.line)) + 2 + location.start) + Logger.pointer
            output += f"{Color.GREY}\n| location: {PATH}:{location.line}:{location.start}\n|   {line}\n|   {Color.YELLOW}{pointer * max(1, location.end - location.start)}"
            output += f" {location_message}{Color.RESET}" if location_message is not None else f"{Color.RESET}"

        for arg in args:
            output += f"{Color.GREY}\n| {arg}{Color.RESET}"

        print(f"{output}")

    def debug(message: str, *args: any) -> None:
        output = f"[{Color.BOLD}{Color.GREY}DEBUG{Color.RESET}] {message}"

        for arg in args:
            output += f"{Color.GREY}\n| {arg}{Color.RESET}"

        print(f"{output}")

    def info(message: str, *args: any) -> None:
        output = f"[{Color.BOLD}{Color.BLUE}INFO{Color.RESET}] {message}"

        for arg in args:
            output += f"{Color.GREY}\n| {arg}{Color.RESET}"

        print(f"{output}")


class Tokenizer:
    def __init__(self) -> None:
        self.start = 0
        self.start_offset = 0
        self.current = 0
        self.current_offset = 0
        self.line = 1
        self.tokens = []
        self.keywords = [
            Keyword("and", TokenKind.AND),
            Keyword("break", TokenKind.BREAK),
            Keyword("call", TokenKind.CALL),
            Keyword("case", TokenKind.CASE),
            Keyword("catch", TokenKind.TRY),
            Keyword("const", TokenKind.CONST),
            Keyword("continue", TokenKind.CONTINUE),
            Keyword("defer", TokenKind.DEFER),
            Keyword("do", TokenKind.DO),
            Keyword("drop", TokenKind.DROP),
            Keyword("dup", TokenKind.DUP),
            Keyword("else", TokenKind.ELSE),
            Keyword("end", TokenKind.END),
            Keyword("enum", TokenKind.ENUM),
            Keyword("false", TokenKind.BOOLEAN),
            Keyword("function", TokenKind.FUNCTION),
            Keyword("if", TokenKind.IF),
            Keyword("implement", TokenKind.IMPLEMENT),
            Keyword("match", TokenKind.MATCH),
            Keyword("mutable", TokenKind.MUTABLE),
            Keyword("not", TokenKind.NOT),
            Keyword("or", TokenKind.OR),
            Keyword("over", TokenKind.OVER),
            Keyword("pop", TokenKind.POP),
            Keyword("require", TokenKind.REQUIRE),
            Keyword("return", TokenKind.RETURN),
            Keyword("rot", TokenKind.ROT),
            Keyword("set", TokenKind.SET),
            Keyword("static", TokenKind.STATIC),
            Keyword("struct", TokenKind.STRUCT),
            Keyword("swap", TokenKind.SWAP),
            Keyword("true", TokenKind.BOOLEAN),
            Keyword("try", TokenKind.TRY),
            Keyword("union", TokenKind.UNION),
            Keyword("while", TokenKind.WHILE),
            Keyword("with", TokenKind.WITH),
        ]

        self.keywords.extend(
            Keyword(type_kind.value, TokenKind.TYPE) for type_kind in TypeKind
        )

    def tokenize(self) -> list:
        self.reset()

        while not self.eof():
            self.start = self.current
            self.start_offset = self.current_offset
            self.tokenize_lexeme()

        return self.tokens

    def tokenize_lexeme(self) -> None:
        lexeme = self.advance()

        match lexeme:
            case " " | "\t":
                pass

            case "#":
                while not self.eof() and self.peek() != "\n":
                    self.advance()

            case "\n":
                self.line += 1
                self.current_offset = 0

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
            case ",":
                self.add_token(TokenKind.COMMA)
            case ".":
                self.add_token(TokenKind.DOT)
            case "%":
                self.add_token(TokenKind.REMAINDER)
            case "&":
                self.add_token(TokenKind.BITWISE_AND)
            case "|":
                self.add_token(TokenKind.BITWISE_OR)
            case "^":
                self.add_token(TokenKind.BITWISE_XOR)
            case "~":
                self.add_token(TokenKind.BITWISE_NOT)

            case "*":
                self.add_token(
                    TokenKind.POWER if self.match("*") else TokenKind.MULTIPLICATION
                )
            case "/":
                self.add_token(
                    TokenKind.EUCLIDIAN if self.match("/") else TokenKind.DIVISION
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

            case "=":
                if self.match("="):
                    self.add_token(TokenKind.EQUAL_EQUAL)

            case "!":
                if self.match("="):
                    self.add_token(TokenKind.NOT_EQUAL)

            case '"':
                self.handle_strings()

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

            case _ if lexeme.isdigit():
                self.handle_numbers()

            case _ if lexeme.isalpha() or lexeme in ["_", "@"]:
                self.handle_identifiers()

            case _:
                Logger.warning(
                    f"Invalid symbol '{lexeme}'",
                    location=self.get_location(),
                )

    def handle_numbers(self) -> None:
        while not self.eof() and self.peek().isdigit():
            self.advance()

        if not self.eof() and self.peek() == "." and self.peek_next().isdigit():
            self.advance()

            while not self.eof() and self.peek().isdigit():
                self.advance()

            self.add_token(TokenKind.FLOAT)
        else:
            self.add_token(TokenKind.INTEGER)

    def handle_strings(self) -> None:
        while not self.eof() and self.peek() != '"':
            if self.peek() == "\n":
                Logger.error(
                    "Unterminated string literal !",
                    location=self.get_location(),
                    location_message="close the string with the corresponding quotes",
                )
                raise GlobalException

            if self.peek() == "\\":
                self.advance()
                if self.eof():
                    Logger.error(
                        "Incomplete escape sequence in string literal !",
                        location=self.get_location(),
                        location_message="expected character after backslash",
                    )
                    raise GlobalException

                escape_char = self.peek()
                if escape_char in {"n", "t", "r", "\\", '"'}:
                    self.advance()
                else:
                    Logger.error(
                        f"Invalid escape sequence '\\{escape_char}' in string literal !",
                        location=self.get_location(),
                        location_message="unknown escape sequence",
                    )
                    raise GlobalException
            else:
                self.advance()

        if self.eof():
            Logger.error(
                "Unterminated string literal !",
                location=self.get_location(),
                location_message="close the string with the corresponding quotes",
            )
            raise GlobalException

        self.advance()
        self.add_token(TokenKind.STRING)

    def handle_identifiers(self) -> None:
        while not self.eof() and (self.peek().isalnum() or self.peek() == "_"):
            self.advance()
        text = SOURCE[self.start : self.current]
        token_type = self.get_keyword(text.lower()) or TokenKind.IDENTIFIER
        self.add_token(token_type)

    def get_keyword(self, key: str) -> TokenKind | None:
        for keyword in self.keywords:
            if keyword.name == key:
                return keyword.token
        return None

    def add_token(self, type: TokenKind) -> None:
        self.tokens.append(
            Token(
                kind=type,
                lexeme=SOURCE[self.start : self.current],
                location=self.get_location(),
            )
        )

    def get_location(self) -> Location:
        return Location(
            line=self.line,
            start=self.start_offset,
            end=self.current_offset,
        )

    def eof(self) -> bool:
        return self.current >= len(SOURCE)

    def peek(self) -> str:
        return SOURCE[self.current]

    def peek_next(self) -> str:
        return SOURCE[self.current + 1]

    def advance(self) -> str:
        char = self.peek()
        self.current += 1
        self.current_offset += 1
        return char

    def match(self, expected: str) -> bool:
        if self.eof() or self.peek() != expected:
            return False
        self.current += 1
        self.current_offset += 1
        return True

    def reset(self) -> None:
        self.start = 0
        self.start_offset = 0
        self.current = 0
        self.current_offset = 0
        self.line = 1
        self.tokens = []


class Parser:
    def __init__(self) -> None:
        self.instructions = []
        self.tokens = []
        self.current = 0

    def parse(self, tokens: list) -> list:
        self.reset()
        self.tokens = tokens

        while not self.eof():
            instruction = self.parse_instruction()
            if instruction is not None:
                self.instructions.append(instruction)

        return self.instructions

    def parse_instruction(self) -> Instruction | None:
        token = self.advance()

        match token.kind:
            case TokenKind.INTEGER:
                return Instruction(
                    InstructionType.PUSH,
                    {"type": None, "value": int(token.lexeme)},
                    token,
                )
            case TokenKind.FLOAT:
                return Instruction(
                    InstructionType.PUSH,
                    {"type": Type.F64, "value": float(token.lexeme)},
                    token,
                )
            case TokenKind.STRING:
                return Instruction(
                    InstructionType.PUSH,
                    {"type": Type.STRING, "value": str(token.lexeme[1:-1])},
                    token,
                )
            case TokenKind.BOOLEAN:
                return Instruction(
                    InstructionType.PUSH,
                    {"type": Type.BOOL, "value": token.lexeme.lower() == "true"},
                    token,
                )
            case TokenKind.TYPE:
                return Instruction(
                    InstructionType.PUSH,
                    {"type": Type.TYPE, "value": Type[token.lexeme.upper()]},
                    token,
                )
            case TokenKind.IDENTIFIER:
                return Instruction(
                    InstructionType.PUSH,
                    {"type": Type.VOID, "value": token.lexeme},
                    token,
                )

            case TokenKind.PLUS:
                return Instruction(
                    InstructionType.PLUS,
                    {"type": Type.VOID, "value": token.lexeme},
                    token,
                )
            case TokenKind.MINUS:
                return Instruction(
                    InstructionType.MINUS,
                    {"type": Type.VOID, "value": token.lexeme},
                    token,
                )
            case TokenKind.MULTIPLICATION:
                return Instruction(
                    InstructionType.MULTIPLICATION,
                    {"type": Type.VOID, "value": token.lexeme},
                    token,
                )
            case TokenKind.DIVISION:
                return Instruction(
                    InstructionType.DIVISION,
                    {"type": Type.VOID, "value": token.lexeme},
                    token,
                )
            case TokenKind.EUCLIDIAN:
                return Instruction(
                    InstructionType.EUCLIDIAN,
                    {"type": Type.VOID, "value": token.lexeme},
                    token,
                )
            case TokenKind.REMINDER:
                return Instruction(
                    InstructionType.REMINDER,
                    {"type": Type.VOID, "value": token.lexeme},
                    token,
                )
            case TokenKind.POWER:
                return Instruction(
                    InstructionType.POWER,
                    {"type": Type.VOID, "value": token.lexeme},
                    token,
                )

            case TokenKind.AND:
                return Instruction(
                    InstructionType.AND,
                    {"type": Type.VOID, "value": token.lexeme},
                    token,
                )
            case TokenKind.OR:
                return Instruction(
                    InstructionType.OR,
                    {"type": Type.VOID, "value": token.lexeme},
                    token,
                )
            case TokenKind.NOT:
                return Instruction(
                    InstructionType.NOT,
                    {"type": Type.VOID, "value": token.lexeme},
                    token,
                )

            case TokenKind.EQUAL_EQUAL:
                return Instruction(
                    InstructionType.EQUAL_EQUAL,
                    {"type": Type.VOID, "value": token.lexeme},
                    token,
                )
            case TokenKind.NOT_EQUAL:
                return Instruction(
                    InstructionType.NOT_EQUAL,
                    {"type": Type.VOID, "value": token.lexeme},
                    token,
                )
            case TokenKind.GREATER:
                return Instruction(
                    InstructionType.GREATER,
                    {"type": Type.VOID, "value": token.lexeme},
                    token,
                )
            case TokenKind.GREATER_EQUAL:
                return Instruction(
                    InstructionType.GREATER_EQUAL,
                    {"type": Type.VOID, "value": token.lexeme},
                    token,
                )
            case TokenKind.LESS:
                return Instruction(
                    InstructionType.LESS,
                    {"type": Type.VOID, "value": token.lexeme},
                    token,
                )
            case TokenKind.LESS_EQUAL:
                return Instruction(
                    InstructionType.LESS_EQUAL,
                    {"type": Type.VOID, "value": token.lexeme},
                    token,
                )

            case TokenKind.POP:
                return Instruction(
                    InstructionType.POP,
                    {"type": Type.VOID, "value": token.lexeme},
                    token,
                )
            case TokenKind.DROP:
                return Instruction(
                    InstructionType.DROP,
                    {"type": Type.VOID, "value": token.lexeme},
                    token,
                )
            case TokenKind.DUP:
                return Instruction(
                    InstructionType.DUP,
                    {"type": Type.VOID, "value": token.lexeme},
                    token,
                )
            case TokenKind.OVER:
                return Instruction(
                    InstructionType.OVER,
                    {"type": Type.VOID, "value": token.lexeme},
                    token,
                )
            case TokenKind.ROT:
                return Instruction(
                    InstructionType.ROT,
                    {"type": Type.VOID, "value": token.lexeme},
                    token,
                )
            case TokenKind.SWAP:
                return Instruction(
                    InstructionType.SWAP,
                    {"type": Type.VOID, "value": token.lexeme},
                    token,
                )

            case TokenKind.RETURN:
                return Instruction(
                    InstructionType.RETURN,
                    {"type": Type.VOID, "value": token.lexeme},
                    token,
                )
            case TokenKind.CALL:
                return Instruction(
                    InstructionType.CALL,
                    {"type": Type.VOID, "value": token.lexeme},
                    token,
                )
            case TokenKind.BREAK:
                return Instruction(
                    InstructionType.BREAK,
                    {"type": Type.VOID, "value": token.lexeme},
                    token,
                )
            case TokenKind.CONTINUE:
                return Instruction(
                    InstructionType.CONTINUE,
                    {"type": Type.VOID, "value": token.lexeme},
                    token,
                )
            case TokenKind.DEFER:
                return Instruction(
                    InstructionType.DEFER,
                    {"type": Type.VOID, "value": token.lexeme},
                    token,
                )

            case TokenKind.MUTABLE | TokenKind.CONST | TokenKind.STATIC:
                return self.handle_variables(token)
            case TokenKind.SET:
                return self.handle_sets(token)
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
                return self.handle_implementations(token)
            case TokenKind.ENUM:
                return self.handle_enums(token)
            case TokenKind.UNION:
                return self.handle_unions(token)
            case TokenKind.REQUIRE:
                return self.handle_requires(token)
            case TokenKind.DOT:
                return self.handle_dots(token)

            case TokenKind.L_SQUARE:
                return self.handle_arrays(token)
            case TokenKind.L_CURLY:
                return self.handle_hashmaps(token)
            case TokenKind.L_PAREN:
                return self.handle_lists(token)

        return None

    def handle_lists(self, token: Token) -> Instruction:
        body = []
        while not self.eof() and self.peek().kind != TokenKind.R_PAREN:
            body.append(self.advance())

        if self.eof() or self.expect(TokenKind.R_PAREN) is None:
            Log.print(
                LogType.ERROR,
                "List not closed !",
                {
                    "location": token.location,
                    "location_message": "you need to close a list with `)`",
                },
            )
            raise ParserError

        return Instruction(
            InstructionType.LIST_DEF,
            {"body": body},
            token,
        )

    def handle_arrays(self, token: Token) -> Instruction:
        body = []
        while not self.eof() and self.peek().kind != TokenKind.R_SQUARE:
            body.append(self.advance())

        if self.eof() or self.expect(TokenKind.R_SQUARE) is None:
            Log.print(
                LogType.ERROR,
                "Array not closed !",
                {
                    "location": token.location,
                    "location_message": "you need to close an array with `]`",
                },
            )
            raise ParserError

        return Instruction(
            InstructionType.ARRAY_DEF,
            {"body": body},
            token,
        )

    def handle_hashmaps(self, token: Token) -> Instruction:
        body = []
        while not self.eof() and self.peek().kind != TokenKind.R_CURLY:
            hashmap_value = self.advance()
            hashmap_key = self.advance()

            body.append()

        if self.eof() or self.expect(TokenKind.R_CURLY) is None:
            Log.print(
                LogType.ERROR,
                "Hashmap not closed !",
                {
                    "location": token.location,
                    "location_message": "you need to close an hashmap with `}`",
                },
            )
            raise ParserError

        return Instruction(
            InstructionType.ARRAY_DEF,
            {"body": body},
            token,
        )

    def handle_variables(self, token: Token) -> Instruction:
        variable_type = self.expect(TokenKind.TYPE)
        if variable_type is None:
            Log.print(
                LogType.ERROR,
                "Invalid variable syntax !",
                {
                    "location": self.peek_previous().location,
                    "location_message": "there should be a type after this",
                },
            )
            raise ParserError

        body = []
        while not self.eof() and self.peek().kind != TokenKind.END:
            instruction = self.parse_instruction()
            if instruction is not None:
                body.append(instruction)

        if self.eof() or self.expect(TokenKind.END) is None:
            Log.print(
                LogType.ERROR,
                "Variable not closed !",
                {
                    "location": token.location,
                    "location_message": "you need to close a variable initialization with `end`",
                },
            )
            raise ParserError

        return Instruction(
            InstructionType.VARIABLE,
            {"type": Type.VOID, "value": {"type": variable_type, "body": body}},
            token,
        )

    def handle_sets(self, token: Token) -> Instruction:
        body = []
        while not self.eof() and self.peek().kind != TokenKind.END:
            instruction = self.parse_instruction()
            if instruction is not None:
                body.append(instruction)

        if not body:
            Log.print(
                LogType.ERROR,
                "Invalid assignation syntax !",
                {
                    "location": token.location,
                    "location_message": "there should be a value or an expression after this",
                },
            )
            raise ParserError

        if self.eof() or self.expect(TokenKind.END) is None:
            Log.print(
                LogType.ERROR,
                "Assignation not closed !",
                {
                    "location": token.location,
                    "location_message": "you need to close a variable assignation with `end`",
                },
            )
            raise ParserError

        return Instruction(
            InstructionType.SET,
            {
                "type": Type.VOID,
                "value": {"body": body},
            },
            token,
        )

    def handle_functions(self, token: Token) -> Instruction:
        parameters = []
        while not self.eof() and self.peek().kind != TokenKind.DO:
            if self.peek().type not in [
                TokenKind.TYPE,
                TokenKind.IDENTIFIER,
                TokenKind.DO,
            ]:
                Log.print(
                    LogType.ERROR,
                    "Invalid function syntax !",
                    {
                        "location": self.peek().location,
                        "location_message": "parameters are composed of a type followed by an identifier",
                    },
                )
                raise ParserError

            parameter_type = self.expect(TokenKind.TYPE)
            parameter_name = self.expect(TokenKind.IDENTIFIER)

            if parameter_type is None:
                Log.print(
                    LogType.ERROR,
                    "Invalid parameter syntax !",
                    {
                        "location": parameter_name.location,
                        "location_message": "there should be a type before this",
                    },
                )
                raise ParserError
            elif parameter_name is None:
                Log.print(
                    LogType.ERROR,
                    "Invalid parameter syntax !",
                    {
                        "location": parameter_type.location,
                        "location_message": "there should be an identifier after this",
                    },
                )
                raise ParserError

            parameters.append({"type": parameter_type, "name": parameter_name})

        if self.eof() or self.expect(TokenKind.DO) is None:
            Log.print(
                LogType.ERROR,
                "Invalid function syntax !",
                {
                    "location": self.peek_previous().location,
                    "location_message": "there should be a function body after this",
                },
            )
            Log.print(
                LogType.INFO,
                "Open a function body with a `do` and close it with a `end` !",
                {},
            )
            raise ParserError

        body = []
        while not self.eof() and self.peek().type != TokenKind.END:
            instruction = self.parse_instruction()
            if instruction is not None:
                body.append(instruction)

        if self.eof() or self.expect(TokenKind.END) is None:
            Log.print(
                LogType.ERROR,
                "Function not closed !",
                {
                    "location": token.location,
                    "location_message": "you need to close a function with `end`",
                },
            )
            raise ParserError

        return_type = None
        if not self.eof() and self.expect(TokenKind.WITH) is not None:
            return_type = self.expect(TokenKind.TYPE)
            if return_type is None:
                Log.print(
                    LogType.ERROR,
                    "Invalid function syntax !",
                    {
                        "location": self.peek_previous().location,
                        "location_message": "there should be a type after this",
                    },
                )
                Log.print(
                    LogType.INFO,
                    "If you don't want to specify a return type, don't put a `with`. It will return `void` by default !",
                    {},
                )
                raise ParserError

        return Instruction(
            InstructionType.FUNCTION,
            {
                "type": Type.VOID,
                "value": {
                    "parameters": parameters,
                    "body": body,
                    "return_type": return_type,
                },
            },
            token,
        )

    def handle_if_elses(self, token: Token) -> Instruction:
        if_body = []
        while not self.eof() and self.peek().type not in [
            TokenKind.ELSE,
            TokenKind.END,
        ]:
            instruction = self.parse_instruction()
            if instruction is not None:
                if_body.append(instruction)

        else_body = []
        else_token = self.expect(TokenKind.ELSE)
        if not self.eof() and else_token is not None:
            while not self.eof() and self.peek().type != TokenKind.END:
                instruction = self.parse_instruction()
                if instruction is not None:
                    else_body.append(instruction)

        if self.eof() or self.expect(TokenKind.END) is None:
            Log.print(
                LogType.ERROR,
                "If statement not closed !",
                {
                    "location": else_token.location
                    if else_token is not None
                    else token.location,
                    "location_message": "you need to close an if statement with `end`",
                },
            )
            raise ParserError

        return Instruction(
            InstructionType.IF,
            {
                "type": Type.VOID,
                "value": {"if": if_body, "else": else_body},
            },
            token,
        )

    def handle_matchs(self, token: Token) -> Instruction:
        cases = []
        else_body = []
        while not self.eof() and self.peek().type != TokenKind.END:
            else_token = self.expect(TokenKind.ELSE)
            if else_token is not None:
                while not self.eof and self.peek().type != TokenKind.END:
                    instruction = self.parse_instruction()
                    if instruction is not None:
                        else_body.append(instruction)

                if self.eof() or self.expect(TokenKind.END) is None:
                    Log.print(
                        LogType.ERROR,
                        "Case not closed !",
                        {
                            "location": else_token.location,
                            "location_message": "you need to close a case with `end`",
                        },
                    )
                    raise ParserError
                break

            case_condition = []
            while not self.eof() and self.peek().type != TokenKind.CASE:
                instruction = self.parse_instruction()
                if instruction is not None:
                    case_condition.append(instruction)

            case_token = self.expect(TokenKind.CASE)
            if case_condition and case_token is None:
                Log.print(
                    LogType.ERROR,
                    "Invalid match syntax !",
                    {
                        "location": case_condition[-1].token.location,
                        "location_message": "there should be a case body after this",
                    },
                )
                raise ParserError
            elif not case_condition and case_token is not None:
                Log.print(
                    LogType.ERROR,
                    "Invalid match syntax !",
                    {
                        "location": self.peek_previous().location,
                        "location_message": "there should be a value or a condition before this",
                    },
                )
                raise ParserError

            if not case_condition and case_token is None:
                break

            case_body = []
            while not self.eof and self.peek().type != TokenKind.END:
                instruction = self.parse_instruction()
                if instruction is not None:
                    case_body.append(instruction)

            if self.eof() or self.expect(TokenKind.END) is None:
                Log.print(
                    LogType.ERROR,
                    "Case not closed !",
                    {
                        "location": case_token.location,
                        "location_message": "you need to close a case with `end`",
                    },
                )
                raise ParserError

            cases.append({"condition": case_condition, "body": case_body})

        if self.eof() or self.expect(TokenKind.END) is None:
            Log.print(
                LogType.ERROR,
                "Match statement not closed !",
                {
                    "location": token.location,
                    "location_message": "you need to close a match statement with `end`",
                },
            )
            raise ParserError

        return Instruction(
            InstructionType.MATCH,
            {
                "type": Type.VOID,
                "value": {"cases": cases, "else": else_body},
            },
            token,
        )

    def handle_whiles(self, token: Token) -> Instruction:
        body = []
        while not self.eof() and self.peek().type != TokenKind.END:
            instruction = self.parse_instruction()
            if instruction is not None:
                body.append(instruction)

        if self.eof() or self.expect(TokenKind.END) is None:
            Log.print(
                LogType.ERROR,
                "While statement not closed !",
                {
                    "location": token.location,
                    "location_message": "you need to close a while statement with `end`",
                },
            )
            raise ParserError

        return Instruction(
            InstructionType.WHILE,
            {
                "type": Type.VOID,
                "value": {"body": body},
            },
            token,
        )

    def handle_structs(self, token: Token) -> Instruction:
        body = []
        while not self.eof() and self.peek().type != TokenKind.END:
            if self.peek().type not in [
                TokenKind.TYPE,
                TokenKind.IDENTIFIER,
            ]:
                Log.print(
                    LogType.ERROR,
                    "Invalid struct syntax !",
                    {
                        "location": self.peek().location,
                        "location_message": "struct fields are composed of a type followed by an identifier",
                    },
                )
                raise ParserError

            struct_variable_type = self.expect(TokenKind.TYPE)
            struct_variable_name = self.expect(TokenKind.IDENTIFIER)

            if struct_variable_type is None:
                Log.print(
                    LogType.ERROR,
                    "Invalid struct syntax !",
                    {
                        "location": struct_variable_name.location,
                        "location_message": "there should be a type before this",
                    },
                )
                raise ParserError
            elif struct_variable_name is None:
                Log.print(
                    LogType.ERROR,
                    "Invalid struct syntax !",
                    {
                        "location": struct_variable_type.location,
                        "location_message": "there should be an identifier after this",
                    },
                )
                raise ParserError

            body.append({"type": struct_variable_type, "name": struct_variable_name})

        if self.eof() or self.expect(TokenKind.END) is None:
            Log.print(
                LogType.ERROR,
                "Struct statement not closed !",
                {
                    "location": token.location,
                    "location_message": "you need to close a struct statement with `end`",
                },
            )
            raise ParserError

        return Instruction(
            InstructionType.STRUCT,
            {
                "type": Type.VOID,
                "value": {"body": body},
            },
            token,
        )

    def handle_implementations(self, token: Token) -> Instruction:
        body = []
        while not self.eof() and self.peek().type != TokenKind.END:
            if self.peek().type not in [
                TokenKind.FUNCTION,
                TokenKind.IDENTIFIER,
            ]:
                Log.print(
                    LogType.ERROR,
                    "Invalid implement syntax !",
                    {
                        "location": self.peek().location,
                        "location_message": "implement are composed of functions only",
                    },
                )
                raise ParserError

            implement_identifier = self.expect(TokenKind.IDENTIFIER)
            if implement_identifier is not None and (
                self.eof() or self.peek().type != TokenKind.FUNCTION
            ):
                Log.print(
                    LogType.ERROR,
                    "Invalid implement syntax !",
                    {
                        "location": implement_identifier.location,
                        "location_message": "there should be a function after this",
                    },
                )
                raise ParserError

            instruction = self.parse_instruction()
            if instruction is not None:
                body.append(instruction)

        if self.eof() or self.expect(TokenKind.END) is None:
            Log.print(
                LogType.ERROR,
                "Implement statement not closed !",
                {
                    "location": token.location,
                    "location_message": "you need to close an implement statement with `end`",
                },
            )
            raise ParserError

        return Instruction(
            InstructionType.IMPLEMENT,
            {
                "type": Type.VOID,
                "value": {"body": body},
            },
            token,
        )

    def handle_enums(self, token: Token) -> Instruction:
        body = []
        while not self.eof() and self.peek().type != TokenKind.END:
            identifier = self.expect(TokenKind.IDENTIFIER)
            if identifier is None:
                Log.print(
                    LogType.ERROR,
                    "Invalid enum syntax !",
                    {
                        "location": self.peek().location,
                        "location_message": "there should be an identifier here",
                    },
                )
                Log.print(
                    LogType.INFO,
                    "After an identifier, you can give an integer or a float to start the enum from !",
                    {},
                )
                raise ParserError

            value = self.expect(TokenKind.INTEGER) or self.expect(TokenKind.FLOAT)
            body.append({"identifier": identifier, "value": value})

        if self.eof() or self.expect(TokenKind.END) is None:
            Log.print(
                LogType.ERROR,
                "Enum statement not closed !",
                {
                    "location": token.location,
                    "location_message": "you need to close an enum statement with `end`",
                },
            )
            raise ParserError

        return Instruction(
            InstructionType.ENUM,
            {
                "type": Type.VOID,
                "value": {"body": body},
            },
            token,
        )

    def handle_unions(self, token: Token) -> Instruction:
        body = []
        while not self.eof() and self.peek().type != TokenKind.END:
            union_type = self.expect(TokenKind.TYPE)
            if union_type is None:
                Log.print(
                    LogType.ERROR,
                    "Invalid union syntax !",
                    {
                        "location": self.peek().location,
                        "location_message": "there should be a type here",
                    },
                )
                raise ParserError

            body.append(union_type)

        if self.eof() or self.expect(TokenKind.END) is None:
            Log.print(
                LogType.ERROR,
                "Union statement not closed !",
                {
                    "location": token.location,
                    "location_message": "you need to close an union statement with `end`",
                },
            )
            raise ParserError

        return Instruction(
            InstructionType.UNION,
            {
                "type": Type.VOID,
                "value": {"body": body},
            },
            token,
        )

    def handle_requires(self, token: Token) -> Instruction:
        identifiers = []
        while not self.eof() and self.peek().type != TokenKind.END:
            identifier = self.expect(TokenKind.IDENTIFIER)
            if identifier is None:
                Log.print(
                    LogType.ERROR,
                    "Invalid require syntax !",
                    {
                        "location": self.peek().location,
                        "location_message": "there should be an identifier here",
                    },
                )
                raise ParserError

            identifiers.append(identifier)

        if identifiers and len(identifiers) > 1:
            Log.print(
                LogType.ERROR,
                "Invalid require syntax !",
                {
                    "location": token.location,
                    "location_message": "there can only be on identifier per `require`",
                },
            )
            raise ParserError

        if self.eof() or self.expect(TokenKind.END) is None:
            Log.print(
                LogType.ERROR,
                "Require statement not closed !",
                {
                    "location": token.location,
                    "location_message": "you need to close a require statement with `end`",
                },
            )
            raise ParserError

        return Instruction(
            InstructionType.REQUIRE,
            {
                "type": Type.VOID,
                "value": {"scope": identifiers[0]},
            },
            token,
        )

    def handle_dots(self, token: Token) -> Instruction:
        identifier = self.expect(TokenKind.IDENTIFIER)
        if identifier is None:
            Log.print(
                LogType.ERROR,
                "Invalid dot syntax !",
                {
                    "location": token.location,
                    "location_message": "there should be an identifier after this",
                },
            )
            raise ParserError

        return Instruction(
            InstructionType.DOT,
            {
                "type": Type.VOID,
                "value": {"identifier": identifier},
            },
            token,
        )

    def peek_previous(self) -> Token:
        return self.tokens[self.current - 1]

    def peek(self) -> Token:
        return self.tokens[self.current]

    def advance(self) -> Token:
        token = self.peek()
        self.current += 1
        return token

    def expect(self, expected_type: TokenKind) -> Token | None:
        if not self.eof() and self.peek().type == expected_type:
            return self.advance()
        return None

    def eof(self) -> bool:
        return self.current >= len(self.tokens)

    def reset(self) -> None:
        self.instructions = []
        self.tokens = []
        self.current = 0


class Compiler:
    def __init__(self):
        pass

    def compile(self, instructions: list) -> None:
        pass


class Cli:
    def scan_file(self, path: str) -> None:
        global SOURCE
        global PATH

        PATH = path

        try:
            with open(PATH, "r", encoding="utf-8") as file:
                SOURCE = self.sanitize_source(file.read().strip())
        except Exception:
            Logger.error(
                "No sush file or directory !",
                f"path: {PATH}",
            )
            exit(1)

        tokenizer = Tokenizer()
        parser = Parser()

        if SOURCE != "":
            try:
                parser.parse(tokenizer.tokenize())
                print(parser.instructions)
            except Exception as error:
                if not isinstance(error, GlobalException):
                    raise

    def sanitize_source(self, source: str) -> str:
        return source.replace("\t", "").replace("\r\n", "\n").replace("\r", "\n")


def main():
    cli = Cli()

    if len(sys.argv) == 2:
        cli.scan_file(sys.argv[1])
    else:
        # TODO: print help or something
        exit(1)


if __name__ == "__main__":
    main()
