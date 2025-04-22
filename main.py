from sys import argv, exit
from enum import Enum
from dataclasses import dataclass

SOURCE: str = ""
PATH: str = ""

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

    def print(log_type: LogType, message: str, options: dict, *arguments: any):
        color = Log.COLORS.get(log_type)
        informations = ""

        if "location" in options:
            location = options.get("location")
            line_content = f"{location.line}| {SOURCE.splitlines()[location.line - 1]}"
            pointer_line = " " * (2 + len(str(location.line))) + " " * location.start_offset + f"─" * max(1, location.current_offset - location.start_offset)
            informations += "\n| ".join(str(arg) for arg in [f"location: {PATH}:{location.line}:{location.start_offset}" if PATH != "" else "location:", f"{line_content}", f"{color}{pointer_line}{f" {options.get("location_message")}" if "location_message" in options else ""}{Color.GREY}"])
            if len(arguments) > 0: informations += "\n| "

        informations += "\n| ".join(str(argument) for argument in arguments)

        print(f"[{Color.BOLD}{color}{log_type.value}{Color.RESET}] {message}{Color.GREY}{"\n| " if informations != "" else ""}{informations}{Color.RESET}")

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
    L_SHIFT = "L_shift"
    R_SHIFT = "R_shift"
    IDENTIFIER = "Identifier"
    STRING = "String"
    INTEGER = "Integer"
    FLOAT = "Float"
    TYPE = "Type"
    AND = "And"
    CALL = "Call"
    CASE = "Case"
    CATCH = "Catch"
    CONST = "Const"
    DEFAULT = "Default"
    DEFER = "Defer"
    DISCARD = "Discard"
    DO = "Do"
    ELSE = "Else"
    END = "End"
    ENUM = "Enum"
    FUNCTION = "Function"
    IF = "If"
    MATCH = "Match"
    MUTABLE = "Mutable"
    NOT = "Not"
    OR = "Or"
    PRIVATE = "Private"
    PROTECTED = "Protected"
    PUBLIC = "Public"
    REQUIRE = "Require"
    RETURN = "Return"
    STRUCT = "Struct"
    TRY = "Try"
    UNION = "Union"
    WHILE = "While"
    WITH = "With"
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
    def __init__(self):
        self.start = 0
        self.start_offset = 0
        self.current = 0
        self.current_offset = 0
        self.line = 1
        self.tokens = []
        self.keywords = [
            Keyword("and", TokenType.AND),
            Keyword("call", TokenType.CALL),
            Keyword("case", TokenType.CASE),
            Keyword("catch", TokenType.TRY),
            Keyword("const", TokenType.CONST),
            Keyword("default", TokenType.DEFAULT),
            Keyword("defer", TokenType.DEFER),
            Keyword("discard", TokenType.DISCARD),
            Keyword("do", TokenType.DO),
            Keyword("drop", TokenType.DROP),
            Keyword("dup", TokenType.DUP),
            Keyword("else", TokenType.ELSE),
            Keyword("end", TokenType.END),
            Keyword("enum", TokenType.ENUM),
            Keyword("function", TokenType.FUNCTION),
            Keyword("if", TokenType.IF),
            Keyword("match", TokenType.MATCH),
            Keyword("mutable", TokenType.MUTABLE),
            Keyword("not", TokenType.NOT),
            Keyword("or", TokenType.OR),
            Keyword("over", TokenType.OVER),
            Keyword("private", TokenType.PRIVATE),
            Keyword("protected", TokenType.PROTECTED),
            Keyword("public", TokenType.PUBLIC),
            Keyword("require", TokenType.REQUIRE),
            Keyword("return", TokenType.RETURN),
            Keyword("rot", TokenType.ROT),
            Keyword("set", TokenType.SET),
            Keyword("struct", TokenType.STRUCT),
            Keyword("swap", TokenType.SWAP),
            Keyword("try", TokenType.TRY),
            Keyword("union", TokenType.UNION),
            Keyword("while", TokenType.WHILE),
            Keyword("with", TokenType.WITH),

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
        if self.eof() or self.peek() != expected: return False
        self.current += 1
        self.current_offset += 1
        return True

    def get_keyword(self, key: str) -> TokenType | None:
        for keyword in self.keywords:
            if keyword.name == key: return keyword.token
        return None

    def add_token(self, type: TokenType) -> None:
        self.tokens.append(Token(
            type=type,
            lexeme=SOURCE[self.start:self.current],
            location=Location(
                line=self.line,
                start=self.start,
                start_offset=self.start_offset,
                current=self.current,
                current_offset=self.current_offset,
            ),
        ))

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
                while not self.eof() and self.peek() != "\n": self.advance()

            case "\n" | "\r":
                self.line += 1
                self.current_offset = 0

            case " " | "\t":
                pass

            case "(": self.add_token(TokenType.L_PAREN)
            case ")": self.add_token(TokenType.R_PAREN)
            case ",": self.add_token(TokenType.COMMA)
            case ".": self.add_token(TokenType.DOT)
            case "%": self.add_token(TokenType.REMINDER)
            case "&": self.add_token(TokenType.BITWISE_AND)
            case "|": self.add_token(TokenType.BITWISE_OR)
            case "^": self.add_token(TokenType.BITWISE_XOR)

            case "*": self.add_token(TokenType.POWER if self.match("*") else TokenType.MULTIPLICATION)
            case "/": self.add_token(TokenType.EUCLIDIAN if self.match("/") else TokenType.DIVISION)

            case "<":
                if self.match("="): self.add_token(TokenType.LESS_EQUAL)
                elif self.match("<"): self.add_token(TokenType.L_SHIFT)
                else: self.add_token(TokenType.LESS)

            case ">":
                if self.match("="): self.add_token(TokenType.GREATER_EQUAL)
                elif self.match(">"): self.add_token(TokenType.R_SHIFT)
                else: self.add_token(TokenType.GREATER)

            case "=":
                if self.match("="): self.add_token(TokenType.EQUAL_EQUAL)

            case "!":
                if self.match("="): self.add_token(TokenType.NOT_EQUAL)

            case "\"":
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
                Log.print(LogType.WARNING, f"Invalid symbol {Color.GREY}'{Color.PURPLE}{char}{Color.GREY}'{Color.RESET} !", {
                    "location": self.get_location(),
                })

    def handle_numbers(self) -> None:
        while not self.eof() and self.peek().isdigit(): self.advance()

        if not self.eof() and self.peek() == "." and self.peek_next().isdigit():
            self.advance()
            while not self.eof() and self.peek().isdigit(): self.advance()
            self.add_token(TokenType.FLOAT)
        else:
            self.add_token(TokenType.INTEGER)

    def handle_strings(self) -> None:
        while not self.eof() and self.peek() != "\"":
            # if self.peek() == "\\":
            #     self.advance()

            if self.peek() == "\n":
                Log.print(LogType.ERROR, "Unterminated string literal !", {
                    "location": self.get_location(),
                    "location_message": "close the string with the corresponding quotes"
                })
                exit(1)

            self.advance()

        if self.eof():
            Log.print(LogType.ERROR, "Unterminated string literal !", {
                "location": self.get_location(),
                "location_message": "close the string with the corresponding quotes"
            })
            exit(1)

        self.advance()
        self.add_token(TokenType.STRING)

    def handle_identifiers(self) -> None:
        while not self.eof() and (self.peek().isalnum() or self.peek() == "_"): self.advance()
        text = SOURCE[self.start:self.current]
        token_type = self.get_keyword(text) or TokenType.IDENTIFIER
        self.add_token(token_type)

@dataclass
class Instruction:
    name: str
    value: any
    token: Token

class Parser:
    def __init__(self):
        self.instructions = []
        self.tokens = []
        self.current = 0

    def parse(self, tokens: list) -> list:
        self.instructions.clear()
        self.tokens = tokens
        self.current = 0

        while not self.eof():
            instruction = self.parse_instruction()
            if not instruction is None: self.instructions.append(instruction)

        return self.instructions

    def parse_instruction(self) -> Instruction | None:
        token = self.advance()

        match token.type:
            case TokenType.INTEGER: return Instruction("PushInt", int(token.lexeme), token)
            case TokenType.FLOAT: return Instruction("PushFloat", float(token.lexeme), token)
            case TokenType.STRING: return Instruction("PushString", token.lexeme[1:-1], token)

            case TokenType.PLUS: return Instruction("Plus", token.lexeme, token)
            case TokenType.MINUS: return Instruction("Minus", token.lexeme, token)
            case TokenType.MULTIPLICATION: return Instruction("Multiplication", token.lexeme, token)
            case TokenType.DIVISION: return Instruction("Division", token.lexeme, token)
            case TokenType.EUCLIDIAN: return Instruction("Euclidian", token.lexeme, token)
            case TokenType.REMINDER: return Instruction("Reminder", token.lexeme, token)
            case TokenType.POWER: return Instruction("Power", token.lexeme, token)

            case TokenType.DROP: return Instruction("Drop", token.lexeme, token)
            case TokenType.DUP: return Instruction("Dup", token.lexeme, token)
            case TokenType.OVER: return Instruction("Over", token.lexeme, token)
            case TokenType.ROT: return Instruction("Rot", token.lexeme, token)
            case TokenType.SWAP: return Instruction("Swap", token.lexeme, token)

            case TokenType.FUNCTION: return self.handle_function()

        return None

    def handle_function(self) -> Instruction:
        name = self.expect(TokenType.IDENTIFIER)
        if name is None:
            Log.print(LogType.ERROR, "Function without name or body !", {
                "location": self.tokens[self.current - 1].location,
                "location_message": "give it a name and open a body with `do ... end`"
            })
            exit(1)

        params = []
        while not self.eof() and not self.peek().type == TokenType.DO:
            param_type = self.expect(TokenType.TYPE)
            param_name = self.expect(TokenType.IDENTIFIER)

            if not self.peek().type in [TokenType.TYPE, TokenType.IDENTIFIER, TokenType.DO]:
                Log.print(LogType.ERROR, "Invalid parameter !", {
                    "location": self.peek().location,
                    "location_message": "a function parameter is composed of a type and a name"
                })
                exit(1)

            if param_type is None and param_name is None:
                self.advance()
                continue

            if param_type is None:
                Log.print(LogType.ERROR, "Parameter without type !", {
                    "location": param_name.location,
                    "location_message": "give it one of the available types"
                })
                exit(1)
            elif param_name is None:
                Log.print(LogType.ERROR, "Parameter without name !", {
                    "location": param_type.location,
                    "location_message": "give it a name"
                })
                exit(1)

            params.append({"type": param_type, "name": param_name})

        if self.eof() or self.peek().type != TokenType.DO:
            Log.print(LogType.ERROR, "Function without body !", {
                "location": name.location,
                "location_message": "open a function body with `do ... end`"
            })
            exit(1)

        self.advance()

        body = []
        closed = False
        while not self.eof():
            if self.peek().type == TokenType.END:
                self.advance()
                closed = True
                break

            instruction = self.parse_instruction()
            if not instruction is None: body.append(instruction)

        if not closed:
            Log.print(LogType.ERROR, "Function body not closed !", {
                "location": name.location,
                "location_message": "close a function body with `end`"
            })
            exit(1)

        return_type = None
        if not self.eof() and self.peek().type == TokenType.WITH:
            self.advance()
            result = self.expect(TokenType.TYPE)
            if result is None:
                Log.print(LogType.ERROR, "Invalid return type !", {
                    "location": self.tokens[self.current - 1].location,
                    "location_message": "should have a type after `with`"
                })
                Log.print(LogType.INFO, "If you don't want to specify a return type, don't put a `with`. It will return `void` by default !", {})
                exit(1)
            return_type = result

        return Instruction("Function", {"parameters": params, "body": body, "return_type": return_type}, name)

    def peek(self) -> Token:
        return self.tokens[self.current]

    def advance(self) -> Token:
        token = self.peek()
        self.current += 1
        return token

    def expect(self, expected_type: TokenType) -> Token | None:
        if not self.eof() and self.peek().type == expected_type: return self.advance()
        return None

    def eof(self) -> bool:
        return self.current >= len(self.tokens)

class Interpreter:
    def __init__(self):
        self.stack = []
        self.functions = []
        self.calls = []
        self.instructions = []

    def interpret(self, instructions: list) -> None:
        self.instructions = instructions

        for instruction in self.instructions:
            match instruction.name:
                case "PushInt": self.stack.append(instruction.value)
                case "PushFloat": self.stack.append(instruction.value)
                case "PushString": self.stack.append(instruction.value)

                case "Plus":
                    if len(self.stack) < 2:
                        # TODO: set error
                        exit(1)

                    b = self.stack.pop()
                    a = self.stack.pop()

                    if not isinstance(a, (int, float)) or not isinstance(b, (int, float)):
                        # TODO: set error
                        exit(1)

                    self.stack.append(a + b)

                case "Minus":
                    if len(self.stack) < 2:
                        # TODO: set error
                        exit(1)

                    b = self.stack.pop()
                    a = self.stack.pop()

                    if not isinstance(a, (int, float)) or not isinstance(b, (int, float)):
                        # TODO: set error
                        exit(1)

                    self.stack.append(a - b)

                case "Multiplication":
                    if len(self.stack) < 2:
                        # TODO: set error
                        exit(1)

                    b = self.stack.pop()
                    a = self.stack.pop()

                    if not isinstance(a, (int, float)) or not isinstance(b, (int, float)):
                        # TODO: set error
                        exit(1)

                    self.stack.append(a * b)

                case "Division":
                    if len(self.stack) < 2:
                        # TODO: set error
                        exit(1)

                    b = self.stack.pop()
                    a = self.stack.pop()

                    if not isinstance(a, (int, float)) or not isinstance(b, (int, float)):
                        # TODO: set error
                        exit(1)

                    self.stack.append(a / b)

                case "Euclidian":
                    if len(self.stack) < 2:
                        # TODO: set error
                        exit(1)

                    b = self.stack.pop()
                    a = self.stack.pop()

                    if not isinstance(a, (int, float)) or not isinstance(b, (int, float)):
                        # TODO: set error
                        exit(1)

                    self.stack.append(a // b)

                case "Reminder":
                    if len(self.stack) < 2:
                        # TODO: set error
                        exit(1)

                    b = self.stack.pop()
                    a = self.stack.pop()

                    if not isinstance(a, (int, float)) or not isinstance(b, (int, float)):
                        # TODO: set error
                        exit(1)

                    self.stack.append(a % b)

                case "Power":
                    if len(self.stack) < 2:
                        # TODO: set error
                        exit(1)

                    b = self.stack.pop()
                    a = self.stack.pop()

                    if not isinstance(a, (int, float)) or not isinstance(b, (int, float)):
                        # TODO: set error
                        exit(1)

                    self.stack.append(a ** b)

                case "Drop":
                    if len(self.stack) < 1:
                        # TODO: set error
                        exit(1)

                    self.stack.pop()

                case "Dup":
                    if len(self.stack) < 1:
                        # TODO: set error
                        exit(1)

                    self.stack.append(self.stack[-1])

                case "Over":
                    if len(self.stack) < 2:
                        # TODO: set error
                        exit(1)

                    self.stack.append(self.stack[-2])

                case "Rot":
                    if len(self.stack) < 3:
                        # TODO: set error
                        exit(1)

                    self.stack[-3], self.stack[-2], self.stack[-1] = self.stack[-2], self.stack[-1], self.stack[-3]

                case "Swap":
                    if len(self.stack) < 2:
                        # TODO: set error
                        exit(1)

                    self.stack[-1], self.stack[-2] = self.stack[-2], self.stack[-1]

                case _:
                    pass

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
                SOURCE = file.read().strip()
        except Exception as error:
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

        tokenizer = Tokenizer()
        parser = Parser()
        interpreter = Interpreter()

        while True:
            try:
                SOURCE = input("> ").strip()
            except Exception:
                Log.print(LogType.ERROR, "Failed to read user input !")
                exit(1)

            if SOURCE.lower() in ("quit", "exit"): break

            if SOURCE != "":
                instructions = parser.parse(tokenizer.tokenize())
                interpreter.interpret(instructions)
                print(instructions)
                print(interpreter.stack)

def main():
    cli = Cli()

    if len(argv) == 1: cli.scan_prompt()
    elif len(argv) == 2: cli.scan_file(argv[1])
    else:
        # TODO: print help or something
        exit(1)

if __name__ == "__main__":
    main()
