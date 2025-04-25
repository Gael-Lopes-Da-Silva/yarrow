from sys import argv, exit
from enum import Enum
from dataclasses import dataclass

SOURCE: str = ""
PATH: str = ""
REPL: bool = False


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


class LogType(Enum):
    ERROR = "ERROR"
    WARNING = "WARNING"
    INFO = "INFO"
    DEBUG = "DEBUG"


class Log:
    COLORS = {
        LogType.ERROR: Color.RED,
        LogType.WARNING: Color.YELLOW,
        LogType.INFO: Color.CYAN,
        LogType.DEBUG: Color.GREY,
    }

    def print(log_type: LogType, message: str, options: dict, *arguments: any) -> None:
        color = Log.COLORS.get(log_type)
        informations = ""

        if "location" in options:
            location = options.get("location")
            line_content = f"{location.line}| {SOURCE.splitlines()[location.line - 1]}"
            pointer_line = (
                " " * (2 + len(str(location.line)))
                + " " * location.start_offset
                + "─" * max(1, location.current_offset - location.start_offset)
            )
            informations += "\n| ".join(
                str(arg)
                for arg in [
                    f"location: {PATH}:{location.line}:{location.start_offset}"
                    if PATH != ""
                    else "location:",
                    f"{line_content}",
                    f"{color}{pointer_line}{f' {options.get("location_message")}' if 'location_message' in options else ''}{Color.GREY}",
                ]
            )
            if len(arguments) > 0:
                informations += "\n| "

        informations += "\n| ".join(str(argument) for argument in arguments)

        print(
            f"[{Color.BOLD}{color}{log_type.value}{Color.RESET}] {message}{Color.GREY}{'\n| ' if informations != '' else ''}{informations}{Color.RESET}"
        )


class TokenType(Enum):
    L_PAREN = "L_paren"
    R_PAREN = "R_paren"
    COMMA = "Comma"
    DOT = "Dot"

    PLUS = "Plus"
    MINUS = "Minus"
    MULTIPLICATION = "Multiplication"
    DIVISION = "Division"
    EUCLIDIAN = "Euclidian"
    REMINDER = "Reminder"
    POWER = "Power"

    EQUAL_EQUAL = "Equal_equal"
    NOT_EQUAL = "Not_equal"
    GREATER = "Greater"
    GREATER_EQUAL = "Greater_equal"
    LESS = "Less"
    LESS_EQUAL = "Less_equal"

    BITWISE_AND = "Bitwise_and"
    BITWISE_OR = "Bitwise_or"
    BITWISE_XOR = "Bitwise_xor"
    BITWISE_NOT = "Bitwise_not"
    L_SHIFT = "L_shift"
    R_SHIFT = "R_shift"

    IDENTIFIER = "Identifier"
    STRING = "String"
    INTEGER = "Integer"
    FLOAT = "Float"
    BOOLEAN = "Boolean"

    AND = "And"
    NOT = "Not"
    OR = "Or"

    TYPE = "Type"

    END = "End"

    CONST = "Const"
    STATIC = "Static"
    MUTABLE = "Mutable"

    IF = "If"
    ELSE = "Else"

    TRY = "Try"
    CATCH = "Catch"

    MATCH = "Match"
    CASE = "Case"

    WHILE = "While"
    IN = "In"
    BREAK = "Break"
    CONTINUE = "Continue"

    FUNCTION = "Function"
    DO = "Do"
    WITH = "With"
    RETURN = "Return"
    CALL = "Call"

    STRUCT = "Struct"
    ENUM = "Enum"
    UNION = "Union"

    REQUIRE = "Require"
    DEFER = "Defer"
    DISCARD = "Discard"

    DROP = "Drop"
    DUP = "Dup"
    OVER = "Over"
    ROT = "Rot"
    SET = "Set"
    SWAP = "Swap"


@dataclass
class Location:
    line: int
    start: int
    start_offset: int
    current: int
    current_offset: int


@dataclass
class Token:
    type: TokenType
    lexeme: str
    location: Location


@dataclass
class Keyword:
    name: str
    token: TokenType


class Tokenizer:
    def __init__(self) -> None:
        self.start = 0
        self.start_offset = 0
        self.current = 0
        self.current_offset = 0
        self.line = 1
        self.tokens = []
        self.keywords = [
            Keyword("and", TokenType.AND),
            Keyword("not", TokenType.NOT),
            Keyword("or", TokenType.OR),
            Keyword("case", TokenType.CASE),
            Keyword("with", TokenType.WITH),
            Keyword("call", TokenType.CALL),
            Keyword("catch", TokenType.TRY),
            Keyword("const", TokenType.CONST),
            Keyword("break", TokenType.BREAK),
            Keyword("continue", TokenType.CONTINUE),
            Keyword("while", TokenType.WHILE),
            Keyword("do", TokenType.DO),
            Keyword("else", TokenType.ELSE),
            Keyword("end", TokenType.END),
            Keyword("function", TokenType.FUNCTION),
            Keyword("if", TokenType.IF),
            Keyword("in", TokenType.IN),
            Keyword("match", TokenType.MATCH),
            Keyword("mutable", TokenType.MUTABLE),
            Keyword("require", TokenType.REQUIRE),
            Keyword("return", TokenType.RETURN),
            Keyword("set", TokenType.SET),
            Keyword("static", TokenType.STATIC),
            Keyword("struct", TokenType.STRUCT),
            Keyword("try", TokenType.TRY),
            Keyword("union", TokenType.UNION),
            Keyword("enum", TokenType.ENUM),
            Keyword("defer", TokenType.DEFER),
            Keyword("discard", TokenType.DISCARD),
            Keyword("drop", TokenType.DROP),
            Keyword("dup", TokenType.DUP),
            Keyword("over", TokenType.OVER),
            Keyword("rot", TokenType.ROT),
            Keyword("swap", TokenType.SWAP),
            Keyword("true", TokenType.BOOLEAN),
            Keyword("false", TokenType.BOOLEAN),
            Keyword("i8", TokenType.TYPE),
            Keyword("i16", TokenType.TYPE),
            Keyword("i32", TokenType.TYPE),
            Keyword("i64", TokenType.TYPE),
            Keyword("i128", TokenType.TYPE),
            Keyword("u8", TokenType.TYPE),
            Keyword("u16", TokenType.TYPE),
            Keyword("u32", TokenType.TYPE),
            Keyword("u64", TokenType.TYPE),
            Keyword("u128", TokenType.TYPE),
            Keyword("f16", TokenType.TYPE),
            Keyword("f32", TokenType.TYPE),
            Keyword("f64", TokenType.TYPE),
            Keyword("f128", TokenType.TYPE),
            Keyword("bool", TokenType.TYPE),
            Keyword("void", TokenType.TYPE),
            Keyword("string", TokenType.TYPE),
            Keyword("array", TokenType.TYPE),
            Keyword("vector", TokenType.TYPE),
            Keyword("hashmap", TokenType.TYPE),
            Keyword("stack", TokenType.TYPE),
            Keyword("queue", TokenType.TYPE),
            Keyword("ptr", TokenType.TYPE),
            Keyword("usize", TokenType.TYPE),
            Keyword("isize", TokenType.TYPE),
            Keyword("c_char", TokenType.TYPE),
            Keyword("c_short", TokenType.TYPE),
            Keyword("c_ushort", TokenType.TYPE),
            Keyword("c_int", TokenType.TYPE),
            Keyword("c_uint", TokenType.TYPE),
            Keyword("c_long", TokenType.TYPE),
            Keyword("c_ulong", TokenType.TYPE),
            Keyword("c_longlong", TokenType.TYPE),
            Keyword("c_ulonglong", TokenType.TYPE),
            Keyword("c_double", TokenType.TYPE),
            Keyword("c_longdouble", TokenType.TYPE),
        ]

    def tokenize(self) -> list:
        self.start = 0
        self.start_offset = 0
        self.current = 0
        self.current_offset = 0
        self.line = 1
        self.tokens.clear()

        while not self.eof():
            self.start = self.current
            self.start_offset = self.current_offset
            self.scan_token()

        return self.tokens

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

    def get_keyword(self, key: str) -> TokenType | None:
        for keyword in self.keywords:
            if keyword.name == key:
                return keyword.token
        return None

    def add_token(self, type: TokenType) -> None:
        self.tokens.append(
            Token(
                type=type,
                lexeme=SOURCE[self.start : self.current],
                location=Location(
                    line=self.line,
                    start=self.start,
                    start_offset=self.start_offset,
                    current=self.current,
                    current_offset=self.current_offset,
                ),
            )
        )

    def get_location(self) -> Location:
        return Location(
            line=self.line,
            start=self.start,
            start_offset=self.start_offset,
            current=self.current,
            current_offset=self.current_offset,
        )

    def scan_token(self) -> None:
        char = self.advance()
        match char:
            case "#":
                while not self.eof() and self.peek() != "\n":
                    self.advance()

            case "\n" | "\r":
                self.line += 1
                self.current_offset = 0

            case " " | "\t":
                pass

            case "(":
                self.add_token(TokenType.L_PAREN)
            case ")":
                self.add_token(TokenType.R_PAREN)
            case ",":
                self.add_token(TokenType.COMMA)
            case ".":
                self.add_token(TokenType.DOT)
            case "%":
                self.add_token(TokenType.REMINDER)
            case "&":
                self.add_token(TokenType.BITWISE_AND)
            case "|":
                self.add_token(TokenType.BITWISE_OR)
            case "^":
                self.add_token(TokenType.BITWISE_XOR)
            case "~":
                self.add_token(TokenType.BITWISE_NOT)

            case "*":
                self.add_token(
                    TokenType.POWER if self.match("*") else TokenType.MULTIPLICATION
                )
            case "/":
                self.add_token(
                    TokenType.EUCLIDIAN if self.match("/") else TokenType.DIVISION
                )

            case "<":
                if self.match("="):
                    self.add_token(TokenType.LESS_EQUAL)
                elif self.match("<"):
                    self.add_token(TokenType.L_SHIFT)
                else:
                    self.add_token(TokenType.LESS)

            case ">":
                if self.match("="):
                    self.add_token(TokenType.GREATER_EQUAL)
                elif self.match(">"):
                    self.add_token(TokenType.R_SHIFT)
                else:
                    self.add_token(TokenType.GREATER)

            case "=":
                if self.match("="):
                    self.add_token(TokenType.EQUAL_EQUAL)

            case "!":
                if self.match("="):
                    self.add_token(TokenType.NOT_EQUAL)

            case '"':
                self.handle_strings()

            case "-":
                if not self.eof() and self.peek().isdigit():
                    self.handle_numbers()
                else:
                    self.add_token(TokenType.MINUS)

            case "+":
                if not self.eof() and self.peek().isdigit():
                    self.handle_numbers()
                else:
                    self.add_token(TokenType.PLUS)

            case _ if char.isdigit():
                self.handle_numbers()

            case _ if char.isalpha() or char == "_":
                self.handle_identifiers()

            case _:
                Log.print(
                    LogType.WARNING,
                    f"Invalid symbol {Color.GREY}'{Color.PURPLE}{char}{Color.GREY}'{Color.RESET} !",
                    {
                        "location": self.get_location(),
                    },
                )

    def handle_numbers(self) -> None:
        while not self.eof() and self.peek().isdigit():
            self.advance()

        if not self.eof() and self.peek() == "." and self.peek_next().isdigit():
            self.advance()
            while not self.eof() and self.peek().isdigit():
                self.advance()
            self.add_token(TokenType.FLOAT)
        else:
            self.add_token(TokenType.INTEGER)

    def handle_strings(self) -> None:
        while not self.eof() and self.peek() != '"':
            # if self.peek() == "\\":
            #     self.advance()

            if self.peek() == "\n":
                Log.print(
                    LogType.ERROR,
                    "Unterminated string literal !",
                    {
                        "location": self.get_location(),
                        "location_message": "close the string with the corresponding quotes",
                    },
                )
                if not REPL:
                    exit(1)
                return

            self.advance()

        if self.eof():
            Log.print(
                LogType.ERROR,
                "Unterminated string literal !",
                {
                    "location": self.get_location(),
                    "location_message": "close the string with the corresponding quotes",
                },
            )
            if not REPL:
                exit(1)
            return

        self.advance()
        self.add_token(TokenType.STRING)

    def handle_identifiers(self) -> None:
        while not self.eof() and (self.peek().isalnum() or self.peek() == "_"):
            self.advance()
        text = SOURCE[self.start : self.current]
        token_type = self.get_keyword(text.lower()) or TokenType.IDENTIFIER
        self.add_token(token_type)


@dataclass
class Instruction:
    name: str
    value: any
    token: Token


class Parser:
    def __init__(self) -> None:
        self.instructions = []
        self.tokens = []
        self.current = 0

    def parse(self, tokens: list) -> list:
        self.current = 0
        self.tokens = tokens
        self.instructions.clear()

        while not self.eof():
            instruction = self.parse_instruction()
            if instruction is not None:
                self.instructions.append(instruction)

        return self.instructions

    def parse_instruction(self) -> Instruction | None:
        token = self.advance()

        match token.type:
            case TokenType.INTEGER:
                return Instruction("Push", int(token.lexeme), token)
            case TokenType.FLOAT:
                return Instruction("Push", float(token.lexeme), token)
            case TokenType.STRING:
                return Instruction("Push", token.lexeme[1:-1], token)
            case TokenType.BOOLEAN:
                return Instruction("Push", token.lexeme.lower() == "true", token)

            case TokenType.IDENTIFIER:
                return Instruction("Identifier", token.lexeme, token)

            case TokenType.PLUS:
                return Instruction("Plus", token.lexeme, token)
            case TokenType.MINUS:
                return Instruction("Minus", token.lexeme, token)
            case TokenType.MULTIPLICATION:
                return Instruction("Multiplication", token.lexeme, token)
            case TokenType.DIVISION:
                return Instruction("Division", token.lexeme, token)
            case TokenType.EUCLIDIAN:
                return Instruction("Euclidian", token.lexeme, token)
            case TokenType.REMINDER:
                return Instruction("Reminder", token.lexeme, token)
            case TokenType.POWER:
                return Instruction("Power", token.lexeme, token)

            case TokenType.AND:
                return Instruction("And", token.lexeme, token)
            case TokenType.OR:
                return Instruction("Or", token.lexeme, token)
            case TokenType.NOT:
                return Instruction("Not", token.lexeme, token)

            case TokenType.EQUAL_EQUAL:
                return Instruction("EqualEqual", token.lexeme, token)
            case TokenType.NOT_EQUAL:
                return Instruction("NotEqual", token.lexeme, token)
            case TokenType.GREATER:
                return Instruction("Greater", token.lexeme, token)
            case TokenType.GREATER_EQUAL:
                return Instruction("GreaterEqual", token.lexeme, token)
            case TokenType.LESS:
                return Instruction("Less", token.lexeme, token)
            case TokenType.LESS_EQUAL:
                return Instruction("LessEquale", token.lexeme, token)

            case TokenType.DROP:
                return Instruction("Drop", token.lexeme, token)
            case TokenType.DUP:
                return Instruction("Dup", token.lexeme, token)
            case TokenType.OVER:
                return Instruction("Over", token.lexeme, token)
            case TokenType.ROT:
                return Instruction("Rot", token.lexeme, token)
            case TokenType.SWAP:
                return Instruction("Swap", token.lexeme, token)

            case TokenType.MUTABLE | TokenType.CONST | TokenType.STATIC:
                return self.handle_variables(token)
            case TokenType.SET:
                return self.handle_assignations(token)
            case TokenType.FUNCTION:
                return self.handle_functions(token)
            case TokenType.IF:
                return self.handle_if_elses(token)
            case TokenType.MATCH:
                return self.handle_matchs(token)
            case TokenType.WHILE:
                return self.handle_whiles(token)
            case TokenType.STRUCT:
                return self.handle_structs(token)
            case TokenType.ENUM:
                return self.handle_enums(token)
            case TokenType.UNION:
                return self.handle_unions(token)
            case TokenType.REQUIRE:
                return self.handle_requires(token)

        return None

    def handle_variables(self, token: Token) -> Instruction:
        variable_type = self.expect(TokenType.TYPE)
        if variable_type is None:
            Log.print(
                LogType.ERROR,
                "Invalid variable syntax !",
                {
                    "location": self.tokens[self.current - 1].location,
                    "location_message": "put a type after the modifier",
                },
            )
            if not REPL:
                exit(1)
            return

        expression = []
        while not self.eof() and self.peek().type != TokenType.END:
            instruction = self.parse_instruction()
            if instruction is not None:
                expression.append(instruction)

        if self.eof() or self.expect(TokenType.END) is None:
            Log.print(
                LogType.ERROR,
                "Variable not closed !",
                {
                    "location": token.location,
                    "location_message": "close a variable with `end`",
                },
            )
            if not REPL:
                exit(1)
            return

        return Instruction(
            "Variable",
            {"type": variable_type, "expression": expression},
            token,
        )

    def handle_assignations(self, token: Token) -> Instruction:
        expression = []
        while not self.eof() and self.peek().type != TokenType.END:
            instruction = self.parse_instruction()
            if instruction is not None:
                expression.append(instruction)

        if self.eof() or self.expect(TokenType.END) is None:
            Log.print(
                LogType.ERROR,
                "Invalid assignation !",
                {
                    "location": token.location,
                    "location_message": "close a variable assignation with `end`",
                },
            )
            if not REPL:
                exit(1)
            return

        if not expression:
            Log.print(
                LogType.ERROR,
                "Invalid assignation !",
                {
                    "location": token.location,
                    "location_message": "you need to provide a value or expression",
                },
            )
            if not REPL:
                exit(1)
            return

        return Instruction(
            "Assignation",
            {"expression": expression},
            token,
        )

    def handle_functions(self, token: Token) -> Instruction:
        params = []
        while not self.eof() and self.peek().type != TokenType.DO:
            if self.peek().type not in [
                TokenType.TYPE,
                TokenType.IDENTIFIER,
                TokenType.DO,
            ]:
                Log.print(
                    LogType.ERROR,
                    "Invalid parameter !",
                    {
                        "location": self.peek().location,
                        "location_message": "a function parameter is composed of a type and a name",
                    },
                )
                if not REPL:
                    exit(1)
                return

            param_type = self.expect(TokenType.TYPE)
            param_name = self.expect(TokenType.IDENTIFIER)

            if param_type is None and param_name is None:
                self.advance()
                continue
            elif param_type is None:
                Log.print(
                    LogType.ERROR,
                    "Parameter without type !",
                    {
                        "location": param_name.location,
                        "location_message": "give it one of the available types",
                    },
                )
                if not REPL:
                    exit(1)
                return
            elif param_name is None:
                Log.print(
                    LogType.ERROR,
                    "Parameter without name !",
                    {
                        "location": param_type.location,
                        "location_message": "give it a name",
                    },
                )
                if not REPL:
                    exit(1)
                return

            params.append({"type": param_type, "name": param_name})

        if self.eof() or self.expect(TokenType.DO) is None:
            Log.print(
                LogType.ERROR,
                "Function without body !",
                {
                    "location": token.location,
                    "location_message": "open a function body with `do ... end`",
                },
            )
            if not REPL:
                exit(1)
            return

        body = []
        while not self.eof() and self.peek().type != TokenType.END:
            instruction = self.parse_instruction()
            if instruction is not None:
                body.append(instruction)

        if self.eof() or self.expect(TokenType.END) is None:
            Log.print(
                LogType.ERROR,
                "Function body not closed !",
                {
                    "location": token.location,
                    "location_message": "close a function body with `end`",
                },
            )
            if not REPL:
                exit(1)
            return

        return_type = None
        if not self.eof() and self.peek().type == TokenType.WITH:
            self.advance()
            result = self.expect(TokenType.TYPE)
            if result is None:
                Log.print(
                    LogType.ERROR,
                    "Invalid return type !",
                    {
                        "location": self.tokens[self.current - 1].location,
                        "location_message": "should have a type after `with`",
                    },
                )
                Log.print(
                    LogType.INFO,
                    "If you don't want to specify a return type, don't put a `with`. It will return `void` by default !",
                    {},
                )
                if not REPL:
                    exit(1)
                return
            return_type = result

        return Instruction(
            "Function",
            {"parameters": params, "body": body, "return_type": return_type},
            token,
        )

    def handle_if_elses(self, token: Token) -> Instruction:
        if_body = []
        while not self.eof() and self.peek().type not in [
            TokenType.ELSE,
            TokenType.END,
        ]:
            instruction = self.parse_instruction()
            if instruction is not None:
                if_body.append(instruction)

        else_body = []
        else_token = self.expect(TokenType.ELSE)
        if not self.eof() and else_token is not None:
            while not self.eof() and self.peek().type != TokenType.END:
                instruction = self.parse_instruction()
                if instruction is not None:
                    else_body.append(instruction)

        if self.eof() or self.expect(TokenType.END) is None:
            Log.print(
                LogType.ERROR,
                "If else statement not closed !",
                {
                    "location": else_token.location
                    if else_token is not None
                    else token.location,
                    "location_message": "close an if else statement with `end`",
                },
            )
            if not REPL:
                exit(1)
            return

        return Instruction(
            "If",
            {
                "body": if_body,
                "else": else_body,
            },
            token,
        )

    def handle_matchs(self, token: Token) -> Instruction:
        cases = []
        else_body = []
        while not self.eof() and self.peek().type != TokenType.END:
            if self.expect(TokenType.ELSE) is not None:
                while not self.eof and self.peek().type != TokenType.END:
                    instruction = self.parse_instruction()
                    if instruction is not None:
                        else_body.append(instruction)

                if self.eof() or self.expect(TokenType.END) is None:
                    Log.print(
                        LogType.ERROR,
                        "Match statement case not closed !",
                        {
                            "location": token.location,
                            "location_message": "close all cases in a match statement with `end`",
                        },
                    )
                    if not REPL:
                        exit(1)
                    return
                break

            case_condition = []
            while not self.eof() and self.peek().type != TokenType.CASE:
                instruction = self.parse_instruction()
                if instruction is not None:
                    case_condition.append(instruction)

            case_token = self.expect(TokenType.CASE)
            if case_condition and case_token is None:
                Log.print(
                    LogType.ERROR,
                    "Match statement invalid syntax !",
                    {
                        "location": case_condition[-1].token.location,
                        "location_message": "add a `case ... end` after a condition in a match statement",
                    },
                )
                if not REPL:
                    exit(1)
                return
            elif not case_condition and case_token is not None:
                Log.print(
                    LogType.ERROR,
                    "Match statement invalid syntax !",
                    {
                        "location": self.tokens[self.current - 1].location,
                        "location_message": "add a condition to match before a `case`",
                    },
                )
                if not REPL:
                    exit(1)
                return

            if not case_condition and case_token is None:
                break

            case_body = []
            while not self.eof and self.peek().type != TokenType.END:
                instruction = self.parse_instruction()
                if instruction is not None:
                    case_body.append(instruction)

            if self.eof() or self.expect(TokenType.END) is None:
                Log.print(
                    LogType.ERROR,
                    "Match statement case not closed !",
                    {
                        "location": case_token.location,
                        "location_message": "close all cases in a match statement with `end`",
                    },
                )
                if not REPL:
                    exit(1)
                return

            cases.append({"condition": case_condition, "body": case_body})

        if self.eof() or self.expect(TokenType.END) is None:
            Log.print(
                LogType.ERROR,
                "Match statement not closed !",
                {
                    "location": token.location,
                    "location_message": "close a match statement with `end`",
                },
            )
            if not REPL:
                exit(1)
            return

        return Instruction(
            "Match",
            {
                "cases": cases,
                "else": else_body,
            },
            token,
        )

    def handle_whiles(self, token: Token) -> Instruction:
        while_body = []
        while not self.eof() and self.peek().type != TokenType.END:
            instruction = self.parse_instruction()
            if instruction is not None:
                while_body.append(instruction)

        if self.eof() or self.expect(TokenType.END) is None:
            Log.print(
                LogType.ERROR,
                "While statement not closed !",
                {
                    "location": token.location,
                    "location_message": "close a while statement with `end`",
                },
            )
            if not REPL:
                exit(1)
            return

        return Instruction(
            "While",
            {
                "body": while_body,
            },
            token,
        )

    def handle_structs(self, token: Token) -> Instruction:
        pass

    def handle_enums(self, token: Token) -> Instruction:
        body = []
        while not self.eof() and self.peek().type != TokenType.END:
            identifier = self.expect(TokenType.IDENTIFIER)
            if identifier is None:
                Log.print(
                    LogType.ERROR,
                    "Invalid enum syntax !",
                    {
                        "location": self.tokens[self.current].location,
                        "location_message": "enum should be composed of identifiers",
                    },
                )
                Log.print(
                    LogType.INFO,
                    "After an identifier you can also give a number to start from",
                    {},
                )
                if not REPL:
                    exit(1)
                return

            value = self.expect(TokenType.INTEGER) or self.expect(TokenType.FLOAT)
            body.append({"identifier": identifier, "value": value})

        if self.eof() or self.expect(TokenType.END) is None:
            Log.print(
                LogType.ERROR,
                "Enum statement not closed !",
                {
                    "location": token.location,
                    "location_message": "close a enum statement with `end`",
                },
            )
            if not REPL:
                exit(1)
            return

        return Instruction(
            "Enum",
            {"body": body},
            token,
        )

    def handle_unions(self, token: Token) -> Instruction:
        body = []
        while not self.eof() and self.peek().type != TokenType.END:
            union_type = self.expect(TokenType.TYPE)
            if union_type is None:
                Log.print(
                    LogType.ERROR,
                    "Invalid union syntax !",
                    {
                        "location": self.tokens[self.current].location,
                        "location_message": "only types are allowed in unions",
                    },
                )
                if not REPL:
                    exit(1)
                return

            body.append(union_type)

        if self.eof() or self.expect(TokenType.END) is None:
            Log.print(
                LogType.ERROR,
                "Union statement not closed !",
                {
                    "location": token.location,
                    "location_message": "close a union statement with `end`",
                },
            )
            if not REPL:
                exit(1)
            return

        return Instruction(
            "Union",
            {"body": body},
            token,
        )

    def handle_requires(self, token: Token) -> Instruction:
        scope = self.expect(TokenType.IDENTIFIER)

        if self.eof() or self.expect(TokenType.END) is None:
            Log.print(
                LogType.ERROR,
                "Require statement not closed !",
                {
                    "location": token.location,
                    "location_message": "close a require statement with `end`",
                },
            )
            if not REPL:
                exit(1)
            return

        return Instruction(
            "Require",
            {"scope": scope},
            token,
        )

    def peek(self) -> Token:
        return self.tokens[self.current]

    def advance(self) -> Token:
        token = self.peek()
        self.current += 1
        return token

    def expect(self, expected_type: TokenType) -> Token | None:
        if not self.eof() and self.peek().type == expected_type:
            return self.advance()
        return None

    def eof(self) -> bool:
        return self.current >= len(self.tokens)


class Interpreter:
    def __init__(self):
        self.stack = []
        self.instructions = []

    def binary_operator(
        self,
        instruction: Instruction,
        operator: any,
        expected_type: any,
        type_name: str,
        extra_check: any = None,
    ) -> None:
        if len(self.stack) < 2:
            Log.print(
                LogType.ERROR,
                "Stack underflow !",
                {
                    "location": instruction.token.location,
                    "location_message": "there must be at least two elements on the stack",
                },
            )
            if not REPL:
                exit(1)
            return

        b = self.stack.pop()
        a = self.stack.pop()

        if not isinstance(a, expected_type) or not isinstance(b, expected_type):
            Log.print(
                LogType.ERROR,
                "Type mismatch !",
                {
                    "location": instruction.token.location,
                    "location_message": f"must be of type {type_name}",
                },
            )
            if not REPL:
                exit(1)
            return

        if extra_check and (extra_check(a) or extra_check(b)):
            Log.print(
                LogType.ERROR,
                "Type mismatch !",
                {
                    "location": instruction.token.location,
                    "location_message": f"must be of type {type_name}",
                },
            )
            if not REPL:
                exit(1)
            return

        self.stack.append(operator(a, b))

    def unary_operator(
        self,
        instruction: Instruction,
        operator: any,
        expected_type: any,
        type_name: str,
    ) -> None:
        if len(self.stack) < 1:
            Log.print(
                LogType.ERROR,
                "Stack underflow !",
                {
                    "location": instruction.token.location,
                    "location_message": "there must be at least one element on the stack",
                },
            )
            if not REPL:
                exit(1)
            return

        a = self.stack.pop()

        if not isinstance(a, expected_type):
            Log.print(
                LogType.ERROR,
                "Type mismatch !",
                {
                    "location": instruction.token.location,
                    "location_message": f"must be of type {type_name}",
                },
            )
            if not REPL:
                exit(1)
            return

        self.stack.append(operator(a))

    def stack_operator(self, size: int, instruction: Instruction) -> bool:
        if len(self.stack) < size:
            Log.print(
                LogType.ERROR,
                "Stack underflow !",
                {
                    "location": instruction.token.location,
                    "location_message": f"there must be at least {'one' if size == 1 else ('two' if size == 2 else 'three')} element{'s' if size > 1 else ''} on the stack",
                },
            )
            if not REPL:
                exit(1)
            return False
        return True

    def interpret(self, instructions: list) -> None:
        self.instructions = instructions

        for instruction in self.instructions:
            match instruction.name:
                case "Push":
                    self.stack.append(instruction.value)

                case "Plus":
                    self.binary_operator(
                        instruction,
                        (lambda a, b: a + b),
                        (int, float),
                        "int or float",
                        (lambda x: isinstance(x, bool)),
                    )
                case "Minus":
                    self.binary_operator(
                        instruction,
                        (lambda a, b: a - b),
                        (int, float),
                        "int or float",
                        (lambda x: isinstance(x, bool)),
                    )
                case "Multiplication":
                    self.binary_operator(
                        instruction,
                        (lambda a, b: a * b),
                        (int, float),
                        "int or float",
                        (lambda x: isinstance(x, bool)),
                    )
                case "Division":
                    self.binary_operator(
                        instruction,
                        (lambda a, b: a / b),
                        (int, float),
                        "int or float",
                        (lambda x: isinstance(x, bool)),
                    )
                case "Euclidian":
                    self.binary_operator(
                        instruction,
                        (lambda a, b: a // b),
                        (int, float),
                        "int or float",
                        (lambda x: isinstance(x, bool)),
                    )
                case "Reminder":
                    self.binary_operator(
                        instruction,
                        (lambda a, b: a % b),
                        (int, float),
                        "int or float",
                        (lambda x: isinstance(x, bool)),
                    )
                case "Power":
                    self.binary_operator(
                        instruction,
                        (lambda a, b: a**b),
                        (int, float),
                        "int or float",
                        (lambda x: isinstance(x, bool)),
                    )

                case "And":
                    self.binary_operator(
                        instruction, (lambda a, b: a and b), bool, "boolean"
                    )
                case "Or":
                    self.binary_operator(
                        instruction, (lambda a, b: a or b), bool, "boolean"
                    )
                case "Not":
                    self.unary_operator(instruction, (lambda a: not a), bool, "boolean")

                case "Drop":
                    if self.stack_operator(1, instruction):
                        self.stack.pop()
                case "Dup":
                    if self.stack_operator(1, instruction):
                        self.stack.append(self.stack[-1])
                case "Over":
                    if self.stack_operator(2, instruction):
                        self.stack.append(self.stack[-2])
                case "Rot":
                    if self.stack_operator(3, instruction):
                        self.stack[-3], self.stack[-2], self.stack[-1] = (
                            self.stack[-2],
                            self.stack[-1],
                            self.stack[-3],
                        )
                case "Swap":
                    if self.stack_operator(2, instruction):
                        self.stack[-1], self.stack[-2] = self.stack[-2], self.stack[-1]


class Compiler:
    def __init__(self):
        pass

    def compile(self, instructions: list) -> None:
        pass


class Cli:
    def scan_file(self, path: str) -> None:
        global SOURCE
        global PATH
        global REPL

        PATH = path

        try:
            with open(PATH, "r", encoding="utf-8") as file:
                SOURCE = file.read().strip()
        except Exception:
            Log.print(LogType.ERROR, "No such file or directory !", f"path: {PATH}")
            exit(1)

        tokenizer = Tokenizer()
        parser = Parser()
        interpreter = Interpreter()

        if SOURCE != "":
            instructions = parser.parse(tokenizer.tokenize())
            interpreter.interpret(instructions)
            print(instructions)
            print(interpreter.stack)

    def scan_prompt(self) -> None:
        global SOURCE
        global PATH
        global REPL

        REPL = True

        tokenizer = Tokenizer()
        parser = Parser()
        interpreter = Interpreter()

        while True:
            try:
                SOURCE = input("> ").strip()
            except Exception:
                Log.print(LogType.ERROR, "Failed to read user input !")
                exit(1)

            if SOURCE.lower() in ("quit", "exit"):
                break

            if SOURCE != "":
                instructions = parser.parse(tokenizer.tokenize())
                interpreter.interpret(instructions)
                print(instructions)
                print(interpreter.stack)


def main():
    cli = Cli()

    if len(argv) == 1:
        cli.scan_prompt()
    elif len(argv) == 2:
        cli.scan_file(argv[1])
    else:
        # TODO: print help or something
        exit(1)


if __name__ == "__main__":
    main()
